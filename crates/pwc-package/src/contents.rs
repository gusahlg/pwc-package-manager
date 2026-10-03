//! The package contents rules (docs/spec/mod-manifest.md, "Package contents"): which paths form a
//! package, which are forbidden, which are required, and the size limits.

use std::collections::BTreeSet;

use pwc_manifest::{ManifestError, ModManifest, check_package_path};

use crate::{MAX_FILE_BYTES, MAX_FILES, MAX_TOTAL_BYTES, PackageError, PackageFile};

/// Top-level directories whose whole tree is part of a package.
pub(crate) const TREES: [&str; 3] = ["src", "assets", "data"];

/// Optional top-level files.
pub(crate) const OPTIONAL_FILES: [&str; 2] = ["CHANGELOG.md", "NOTICE"];

/// The manifest's path.
pub(crate) const MANIFEST: &str = "mod.toml";

/// The tree a path belongs to (`src` for `src/lib.rs`), if any.
pub(crate) fn tree_of(path: &str) -> Option<&'static str> {
    TREES.into_iter().find(|tree| {
        path.strip_prefix(tree)
            .is_some_and(|rest| rest.starts_with('/'))
    })
}

/// The individually named files of a package: `mod.toml`, the readme, the licence files and the
/// optional top-level files.
pub(crate) fn named_files(manifest: &ModManifest) -> BTreeSet<&str> {
    let mut named: BTreeSet<&str> = [MANIFEST, manifest.package.readme.as_str()].into();
    named.extend(manifest.package.license_files.iter().map(String::as_str));
    named.extend(OPTIONAL_FILES);
    named
}

/// Whether `path` is part of the package contents.
pub(crate) fn is_package_path(path: &str, named: &BTreeSet<&str>) -> bool {
    named.contains(path) || tree_of(path).is_some()
}

/// Why `path` may never be in a package (checked on every component), if it may not.
pub(crate) fn forbidden_reason(path: &str) -> Option<&'static str> {
    path.split('/').find_map(|component| {
        if component.starts_with('.') {
            Some("hidden files and directories are not allowed in a package")
        } else {
            match component {
                "build.rs" => Some("build scripts are not allowed (compiling a package runs no code from it)"),
                "Cargo.toml" | "Cargo.lock" => Some("packages do not ship Cargo manifests (the builder generates them from mod.toml)"),
                _ => None,
            }
        }
    })
}

/// Parse `mod.toml` bytes.
pub(crate) fn parse_manifest(bytes: &[u8]) -> Result<ModManifest, PackageError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ManifestError::Toml("mod.toml is not valid UTF-8".into()))?;
    Ok(ModManifest::parse(text)?)
}

/// Sort `files` by path and apply every contents rule; returns the manifest.
pub(crate) fn validate(files: &mut [PackageFile]) -> Result<ModManifest, PackageError> {
    check_sizes(files)?;
    files.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    for pair in files.windows(2) {
        if pair[0].path == pair[1].path {
            return Err(forbidden(&pair[1].path, "the path appears more than once"));
        }
    }
    for file in files.iter() {
        check_package_path(&file.path).map_err(|reason| forbidden(&file.path, reason))?;
        if let Some(reason) = forbidden_reason(&file.path) {
            return Err(forbidden(&file.path, reason));
        }
    }
    // A path cannot be both a file and a directory (`src/a` and `src/a/b.rs`).
    let mut dirs = BTreeSet::new();
    for file in files.iter() {
        let mut path = file.path.as_str();
        while let Some((parent, _)) = path.rsplit_once('/') {
            if !dirs.insert(parent) {
                break;
            }
            path = parent;
        }
    }
    if let Some(file) = files.iter().find(|f| dirs.contains(f.path.as_str())) {
        return Err(forbidden(
            &file.path,
            "the path is both a file and a directory",
        ));
    }

    let manifest_file = files
        .iter()
        .find(|f| f.path == MANIFEST)
        .ok_or_else(|| PackageError::MissingFile(MANIFEST.into()))?;
    let manifest = parse_manifest(&manifest_file.bytes)?;
    let named = named_files(&manifest);
    let kind = manifest.kind();
    for file in files.iter() {
        if !is_package_path(&file.path, &named) {
            return Err(forbidden(
                &file.path,
                "not part of the package contents (mod.toml, the readme, the licence files, CHANGELOG.md, NOTICE, src/, assets/ and data/)",
            ));
        }
        if !kind.has_code() && tree_of(&file.path) == Some("src") {
            return Err(forbidden(
                &file.path,
                "a bundle contains no code (no src/ directory)",
            ));
        }
    }

    let present = |path: &str| {
        files
            .binary_search_by(|f| f.path.as_str().cmp(path))
            .is_ok()
    };
    let p = &manifest.package;
    for required in [p.readme.as_str()]
        .into_iter()
        .chain(p.license_files.iter().map(String::as_str))
    {
        if !present(required) {
            return Err(PackageError::MissingFile(required.into()));
        }
    }
    if kind.has_code() && !present("src/lib.rs") {
        return Err(PackageError::MissingFile("src/lib.rs".into()));
    }
    if let Some(icon) = &p.icon
        && !present(icon)
    {
        return Err(PackageError::MissingFile(icon.clone()));
    }
    Ok(manifest)
}

/// The file count, per-file and total size limits.
pub(crate) fn check_sizes(files: &[PackageFile]) -> Result<(), PackageError> {
    if files.len() > MAX_FILES {
        return Err(too_many_files(files.len()));
    }
    let mut total = 0u64;
    for file in files {
        let len = file.bytes.len() as u64;
        check_file_size(&file.path, len)?;
        total += len;
        check_total_size(total)?;
    }
    Ok(())
}

pub(crate) fn too_many_files(count: usize) -> PackageError {
    PackageError::TooLarge(format!("{count} files; a package has at most {MAX_FILES}"))
}

pub(crate) fn check_file_size(path: &str, len: u64) -> Result<(), PackageError> {
    if len > MAX_FILE_BYTES {
        return Err(PackageError::TooLarge(format!(
            "`{path}` is {len} bytes; a file is at most {MAX_FILE_BYTES} bytes (32 MiB)"
        )));
    }
    Ok(())
}

pub(crate) fn check_total_size(total: u64) -> Result<(), PackageError> {
    if total > MAX_TOTAL_BYTES {
        return Err(PackageError::TooLarge(format!(
            "more than {MAX_TOTAL_BYTES} bytes (256 MiB) in total"
        )));
    }
    Ok(())
}

pub(crate) fn forbidden(path: &str, reason: &str) -> PackageError {
    PackageError::Forbidden {
        path: path.to_string(),
        reason: reason.to_string(),
    }
}
