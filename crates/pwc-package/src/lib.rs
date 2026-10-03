//! The `.pwcmod` package format (docs/spec/package-format.md): which files form a package, the
//! canonical (deterministic) tar, Zstandard compression, the package hash and verification.
//!
//! A package can be read from a **source directory** (an author's tree or a repository entry) or
//! from a **`.pwcmod` archive**; both give the same [`Package`] and the same hash.
//!
//! Every constructor validates completely (manifest, licence policy, contents rules, limits), so a
//! [`Package`] value is always a valid package. Reading an archive never panics, whatever the
//! input, and never allocates much beyond the package size limits.

mod contents;
mod dir;
#[cfg(test)]
mod tests;
mod ustar;
mod zstd;

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use pwc_manifest::{ManifestError, ModManifest, PackageHash};
use sha2::{Digest, Sha256};

/// Largest file in a package (docs/spec/mod-manifest.md, "Package contents").
pub const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
/// Most files in one package.
pub const MAX_FILES: usize = 4096;
/// Most bytes in one package.
pub const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

/// Largest canonical tar a valid package can have: the file data plus, per file, a header, a PAX
/// header and record for a long path, and padding; plus the end-of-archive blocks. Decompression
/// stops beyond this.
const MAX_TAR_BYTES: u64 = MAX_TOTAL_BYTES + MAX_FILES as u64 * 4096 + 1024;

/// Largest `.pwcmod` file read: Zstandard's worst-case expansion of the largest tar is far below
/// this.
const MAX_ARCHIVE_BYTES: u64 = MAX_TAR_BYTES + MAX_TAR_BYTES / 64 + 1024 * 1024;

/// One file of a package.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PackageFile {
    /// Package-relative path with `/` separators.
    pub path: String,
    /// Contents.
    pub bytes: Vec<u8>,
}

/// A validated package held in memory: manifest, files (sorted by path) and hash.
#[derive(Clone)]
pub struct Package {
    manifest: ModManifest,
    files: Vec<PackageFile>,
    hash: PackageHash,
}

/// A summary (id, version, hash, file paths and sizes) rather than every byte of every file.
impl std::fmt::Debug for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        struct Files<'a>(&'a [PackageFile]);
        impl std::fmt::Debug for Files<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_map()
                    .entries(self.0.iter().map(|file| (&file.path, file.bytes.len())))
                    .finish()
            }
        }
        f.debug_struct("Package")
            .field("id", &self.manifest.package.id)
            .field("version", &self.manifest.package.version)
            .field("hash", &self.hash)
            .field("files", &Files(&self.files))
            .finish()
    }
}

/// What was left out when collecting a source directory (for `--verbose` output).
#[derive(Clone, Debug, Default)]
pub struct Collected {
    /// The package.
    pub package: Option<Package>,
    /// Paths present in the directory but not part of the package.
    pub ignored: Vec<String>,
}

impl Package {
    /// Collect the package contents of a source directory, validate everything (manifest, licence
    /// policy, contents rules) and compute the hash. Ignored paths are reported, not errors;
    /// forbidden ones (build.rs, symlinks, …) are errors.
    ///
    /// The package is `mod.toml`, the readme and licence files it names, `CHANGELOG.md`, `NOTICE`
    /// and the `src/`, `assets/` and `data/` trees. Everything else in the directory (`target/`,
    /// `dist/`, `.git/`, a development `Cargo.toml`, …) is returned in the second value, sorted,
    /// directories with a trailing `/` and not descended into. Symlinks are never followed.
    pub fn from_dir(dir: &Path) -> Result<(Self, Vec<String>), PackageError> {
        let (files, ignored) = dir::collect(dir)?;
        Ok((Self::from_files(files)?, ignored))
    }

    /// Read and fully verify a `.pwcmod` file.
    pub fn from_archive_file(path: &Path) -> Result<Self, PackageError> {
        let io_error = |source| PackageError::Io {
            path: path.to_path_buf(),
            source,
        };
        let file = fs::File::open(path).map_err(io_error)?;
        let mut bytes = Vec::new();
        file.take(MAX_ARCHIVE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
            return Err(PackageError::TooLarge(format!(
                "{} is larger than any valid package",
                path.display()
            )));
        }
        Self::from_archive_bytes(&bytes)
    }

    /// Read and fully verify `.pwcmod` bytes (docs/spec/package-format.md, "Reading and
    /// verification"): decompress exactly one Zstandard frame; parse the tar, rejecting anything
    /// but sorted, unique regular files with canonical headers (the parsed files must re-serialise
    /// to exactly the decompressed bytes); validate the manifest and the contents rules; hash.
    ///
    /// Compare the result with an expected hash using [`Package::verify_hash`].
    pub fn from_archive_bytes(bytes: &[u8]) -> Result<Self, PackageError> {
        let tar = zstd::decompress(bytes, MAX_TAR_BYTES as usize).map_err(PackageError::Archive)?;
        let mut files = ustar::read(&tar, MAX_FILES).map_err(|err| match err {
            ustar::ReadError::TooManyFiles(_) => contents::too_many_files(MAX_FILES + 1),
            ustar::ReadError::Malformed(message) => PackageError::Archive(message),
        })?;
        for pair in files.windows(2) {
            if pair[0].path == pair[1].path {
                return Err(PackageError::Archive(format!(
                    "duplicate entry `{}`",
                    pair[1].path
                )));
            }
            if pair[0].path > pair[1].path {
                return Err(PackageError::Archive(format!(
                    "entries are not sorted by path (`{}` after `{}`)",
                    pair[1].path, pair[0].path
                )));
            }
        }
        if !ustar::is_canonical(&files, &tar) {
            return Err(PackageError::Archive(
                "not a canonical archive (it was not written by the canonical writer)".into(),
            ));
        }
        let manifest = contents::validate(&mut files)?;
        let hash = PackageHash::from_digest(Sha256::digest(&tar).into());
        Ok(Self {
            manifest,
            files,
            hash,
        })
    }

    /// Validate an in-memory file set (used by the two constructors above and by tests). The
    /// files may come in any order; they are sorted by path.
    pub fn from_files(mut files: Vec<PackageFile>) -> Result<Self, PackageError> {
        let manifest = contents::validate(&mut files)?;
        let mut hasher = Sha256::new();
        ustar::emit(&files, |chunk| hasher.update(chunk));
        let hash = PackageHash::from_digest(hasher.finalize().into());
        Ok(Self {
            manifest,
            files,
            hash,
        })
    }

    /// Check the package against a hash the caller expected (from a lockfile or a registry).
    pub fn verify_hash(&self, expected: &PackageHash) -> Result<(), PackageError> {
        if &self.hash == expected {
            Ok(())
        } else {
            Err(PackageError::HashMismatch {
                expected: expected.clone(),
                found: self.hash.clone(),
            })
        }
    }

    /// The manifest.
    pub fn manifest(&self) -> &ModManifest {
        &self.manifest
    }

    /// The files, sorted by path.
    pub fn files(&self) -> &[PackageFile] {
        &self.files
    }

    /// The package hash.
    pub fn hash(&self) -> &PackageHash {
        &self.hash
    }

    /// `<id>-<version>.pwcmod`.
    pub fn file_name(&self) -> String {
        format!(
            "{}-{}.pwcmod",
            self.manifest.package.id, self.manifest.package.version
        )
    }

    /// The canonical tar bytes (what the hash is computed over).
    pub fn canonical_tar(&self) -> Vec<u8> {
        ustar::to_vec(&self.files)
    }

    /// The `.pwcmod` bytes (Zstandard-compressed canonical tar).
    pub fn to_archive_bytes(&self) -> Vec<u8> {
        zstd::compress(&self.canonical_tar())
    }

    /// Write `<out_dir>/<file_name>` (creating `out_dir` if needed) and return its path. The file is
    /// written under a temporary name and renamed into place, so it is never seen half-written.
    pub fn write_archive(&self, out_dir: &Path) -> Result<PathBuf, PackageError> {
        fs::create_dir_all(out_dir).map_err(|source| PackageError::Io {
            path: out_dir.to_path_buf(),
            source,
        })?;
        let path = out_dir.join(self.file_name());
        let tmp = out_dir.join(format!(".{}.tmp-{}", self.file_name(), std::process::id()));
        if let Err(source) = fs::write(&tmp, self.to_archive_bytes()) {
            let _ = fs::remove_file(&tmp);
            return Err(PackageError::Io { path: tmp, source });
        }
        fs::rename(&tmp, &path).map_err(|source| {
            let _ = fs::remove_file(&tmp);
            PackageError::Io {
                path: path.clone(),
                source,
            }
        })?;
        Ok(path)
    }

    /// Write the files under `dir` (which must not exist yet or be empty), creating directories as
    /// needed. Refuses a non-empty directory rather than mixing contents.
    pub fn unpack_to(&self, dir: &Path) -> Result<(), PackageError> {
        let io_error = |path: &Path, source| PackageError::Io {
            path: path.to_path_buf(),
            source,
        };
        match fs::read_dir(dir) {
            Ok(mut entries) => {
                if entries.next().is_some() {
                    return Err(io_error(
                        dir,
                        io::Error::new(
                            io::ErrorKind::DirectoryNotEmpty,
                            "refusing to unpack a package into a non-empty directory",
                        ),
                    ));
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                fs::create_dir_all(dir).map_err(|source| io_error(dir, source))?
            }
            Err(err) => return Err(io_error(dir, err)),
        }
        for file in &self.files {
            // Paths are validated: relative, `/`-separated, no `.`/`..` components.
            let target = dir.join(&file.path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
            }
            let mut out = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|source| io_error(&target, source))?;
            out.write_all(&file.bytes)
                .map_err(|source| io_error(&target, source))?;
        }
        Ok(())
    }
}

/// Why a package was rejected.
#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    /// The manifest is invalid (includes licence policy failures).
    #[error("mod.toml: {0}")]
    Manifest(#[from] ManifestError),
    /// A required file is missing (`mod.toml`, README, licence file, `src/lib.rs`).
    #[error("missing required file `{0}`")]
    MissingFile(String),
    /// A forbidden path (build.rs, Cargo.toml, hidden file, symlink, src/ in a bundle, …).
    #[error("forbidden path `{path}`: {reason}")]
    Forbidden {
        /// The path.
        path: String,
        /// Why.
        reason: String,
    },
    /// A size limit was exceeded.
    #[error("package too large: {0}")]
    TooLarge(String),
    /// The archive is malformed or not canonical.
    #[error("malformed .pwcmod: {0}")]
    Archive(String),
    /// The contents do not match an expected hash.
    #[error("hash mismatch: expected {expected}, found {found}")]
    HashMismatch {
        /// Expected.
        expected: PackageHash,
        /// Found.
        found: PackageHash,
    },
    /// I/O.
    #[error("{path}: {source}")]
    Io {
        /// Path.
        path: PathBuf,
        /// Error.
        source: std::io::Error,
    },
}
