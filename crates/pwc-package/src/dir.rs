//! Collecting a package from a source directory.
//!
//! Only the package contents are read: `mod.toml`, the files the manifest names (readme, licence
//! files), the optional top-level files and the `src/`, `assets/` and `data/` trees. Everything
//! else (`target/`, `dist/`, `.git/`, a development `Cargo.toml`, editor files, …) is reported as
//! ignored without being descended into. Inside the included trees, symlinks, hidden entries,
//! `build.rs`, `Cargo.toml`, `Cargo.lock` and special files are errors. Symlinks are never
//! followed.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Component, Path};

use walkdir::WalkDir;

use crate::contents::{self, MANIFEST, TREES, forbidden, forbidden_reason, named_files, tree_of};
use crate::{MAX_FILE_BYTES, MAX_FILES, PackageError, PackageFile};

/// The package files of `dir` (sorted by path) and the ignored paths (sorted; directories end in `/`).
pub(crate) fn collect(dir: &Path) -> Result<(Vec<PackageFile>, Vec<String>), PackageError> {
    let meta = fs::metadata(dir).map_err(|source| io_error(dir, source))?;
    if !meta.is_dir() {
        return Err(io_error(
            dir,
            io::Error::new(io::ErrorKind::NotADirectory, "not a directory"),
        ));
    }

    // The manifest first: it names the readme and licence files.
    let manifest_path = dir.join(MANIFEST);
    let manifest_meta = match fs::symlink_metadata(&manifest_path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Err(PackageError::MissingFile(MANIFEST.into()));
        }
        Err(err) => return Err(io_error(&manifest_path, err)),
    };
    check_file_type(MANIFEST, &manifest_meta)?;
    contents::check_file_size(MANIFEST, manifest_meta.len())?;
    let manifest = contents::parse_manifest(
        &fs::read(&manifest_path).map_err(|source| io_error(&manifest_path, source))?,
    )?;
    let named = named_files(&manifest);
    // Directories leading to a named file outside the trees (`licenses` for `licenses/MIT.txt`).
    let named_dirs: BTreeSet<&str> = named
        .iter()
        .flat_map(|path| path.match_indices('/').map(|(i, _)| &path[..i]))
        .collect();

    let mut files = BTreeMap::new();
    let mut ignored = Vec::new();
    let mut total = 0u64;
    let mut walker = WalkDir::new(dir)
        .min_depth(1)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter();
    while let Some(entry) = walker.next() {
        let entry = entry.map_err(|err| walk_error(dir, err))?;
        let file_type = entry.file_type();
        let relative = entry.path().strip_prefix(dir).unwrap_or(entry.path());
        let Some(path) = utf8_path(relative) else {
            // Not UTF-8: an error inside a tree, otherwise simply not part of the package.
            let lossy = relative.to_string_lossy().replace('\\', "/");
            if TREES.iter().any(|tree| relative.starts_with(tree)) {
                return Err(forbidden(&lossy, "file names must be valid UTF-8"));
            }
            ignored.push(if file_type.is_dir() {
                format!("{lossy}/")
            } else {
                lossy
            });
            if file_type.is_dir() {
                walker.skip_current_dir();
            }
            continue;
        };

        if TREES.contains(&path.as_str()) {
            // `src`, `assets`, `data` themselves.
            if file_type.is_symlink() {
                return Err(forbidden(&path, "symlinks are not allowed in a package"));
            }
            if !file_type.is_dir() {
                return Err(forbidden(&path, "must be a directory"));
            }
            if path == "src" && !manifest.kind().has_code() {
                return Err(forbidden(
                    &path,
                    "a bundle contains no code (no src/ directory)",
                ));
            }
        } else if tree_of(&path).is_some() {
            if let Some(reason) = forbidden_reason(&path) {
                return Err(forbidden(&path, reason));
            }
            if file_type.is_symlink() {
                return Err(forbidden(&path, "symlinks are not allowed in a package"));
            }
            if file_type.is_dir() {
                continue;
            }
            if !file_type.is_file() {
                return Err(forbidden(&path, "not a regular file"));
            }
            add_file(&mut files, &mut total, entry.path(), path)?;
        } else if named.contains(path.as_str()) {
            // A named file outside the trees; a forbidden name (`.README`) is reported by the
            // contents check like any other.
            check_file_type(
                &path,
                &entry.metadata().map_err(|err| walk_error(dir, err))?,
            )?;
            add_file(&mut files, &mut total, entry.path(), path)?;
        } else if named_dirs.contains(path.as_str()) {
            if file_type.is_symlink() {
                return Err(forbidden(&path, "symlinks are not allowed in a package"));
            }
            if !file_type.is_dir() {
                ignored.push(path);
            }
        } else if file_type.is_dir() {
            ignored.push(format!("{path}/"));
            walker.skip_current_dir();
        } else {
            ignored.push(path);
        }
    }
    ignored.sort_unstable();
    let files = files
        .into_iter()
        .map(|(path, bytes)| PackageFile { path, bytes })
        .collect();
    Ok((files, ignored))
}

/// Read one package file into `files`, enforcing the size and count limits before reading.
fn add_file(
    files: &mut BTreeMap<String, Vec<u8>>,
    total: &mut u64,
    full: &Path,
    path: String,
) -> Result<(), PackageError> {
    if files.len() == MAX_FILES {
        return Err(contents::too_many_files(MAX_FILES + 1));
    }
    let len = fs::symlink_metadata(full)
        .map_err(|source| io_error(full, source))?
        .len();
    contents::check_file_size(&path, len)?;
    contents::check_total_size(*total + len)?;
    let bytes = read_limited(full, &path)?;
    *total += bytes.len() as u64;
    contents::check_total_size(*total)?;
    files.insert(path, bytes);
    Ok(())
}

/// A package file must be a regular file: not a symlink, directory or special file.
fn check_file_type(path: &str, meta: &fs::Metadata) -> Result<(), PackageError> {
    let file_type = meta.file_type();
    if file_type.is_symlink() {
        Err(forbidden(path, "symlinks are not allowed in a package"))
    } else if file_type.is_dir() {
        Err(forbidden(path, "must be a file, not a directory"))
    } else if !file_type.is_file() {
        Err(forbidden(path, "not a regular file"))
    } else {
        Ok(())
    }
}

/// Read a file, refusing to read more than the per-file limit even if it grew since `stat`.
fn read_limited(full: &Path, path: &str) -> Result<Vec<u8>, PackageError> {
    use io::Read;
    let file = fs::File::open(full).map_err(|source| io_error(full, source))?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| io_error(full, source))?;
    contents::check_file_size(path, bytes.len() as u64)?;
    Ok(bytes)
}

/// `/`-joined UTF-8 relative path, or `None` if a component is not UTF-8.
fn utf8_path(relative: &Path) -> Option<String> {
    let mut out = String::new();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return None;
        };
        if !out.is_empty() {
            out.push('/');
        }
        out.push_str(name.to_str()?);
    }
    Some(out)
}

fn io_error(path: &Path, source: io::Error) -> PackageError {
    PackageError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn walk_error(dir: &Path, err: walkdir::Error) -> PackageError {
    let path = err.path().unwrap_or(dir).to_path_buf();
    let source = err
        .into_io_error()
        .unwrap_or_else(|| io::Error::other("filesystem loop"));
    PackageError::Io { path, source }
}
