//! The content-addressed package store (docs/spec/filesystem.md, "The store"): immutable
//! unpacked packages under `<root>/<hex>/`, inserted atomically, verified by hash, garbage
//! collected against the hashes instances still reference.
//!
//! Layout and rules:
//!
//! - `<root>/<hex>/` holds exactly the files of one package, `<hex>` being its hash without the
//!   `sha256:` prefix.
//! - Insertion unpacks into `<root>/.tmp-<random>/`, re-collects and re-hashes that copy, makes it
//!   read-only (files `0444`, directories `0555`) and renames it into place. If the entry appeared
//!   meanwhile the new copy is discarded: the same hash means the same contents.
//! - Removal (gc) first renames the entry to a `.tmp-<random>` name, so a half-deleted entry is
//!   never visible under its hash, then restores write permission and deletes it.
//! - Names starting with `.` are never entries; stale `.tmp-*` directories left by an interrupted
//!   process are deleted by [`Store::gc`].

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pwc_manifest::{ModManifest, PackageHash};
use pwc_package::{Package, PackageError};

/// One installed package.
#[derive(Clone, Debug)]
pub struct StoreEntry {
    /// The hash (directory name).
    pub hash: PackageHash,
    /// Its directory.
    pub dir: PathBuf,
    /// Its manifest.
    pub manifest: ModManifest,
}

/// What [`Store::gc_report`] removed (or, in a dry run, would remove).
#[derive(Clone, Debug, Default)]
pub struct GcReport {
    /// Unreferenced entries, sorted by (id, version).
    pub removed: Vec<StoreEntry>,
    /// Unreferenced entries whose manifest could not be read (corrupt), sorted.
    pub removed_unreadable: Vec<PackageHash>,
    /// Leftover temporary directories of interrupted operations, sorted.
    pub temporaries: Vec<PathBuf>,
}

/// A store rooted at a directory (normally `$XDG_DATA_HOME/pwc/store`).
#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
}

/// Prefix of temporary directories inside the store root.
const TMP_PREFIX: &str = ".tmp-";

/// [`Store::gc`] only deletes temporaries at least this old: younger ones may belong to an insertion
/// still running in another process.
pub const TEMPORARY_MIN_AGE: std::time::Duration = std::time::Duration::from_secs(3600);

impl Store {
    /// Open (creating the root if needed).
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(|e| io(&root, e))?;
        Ok(Self { root })
    }

    /// The root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Insert a validated package (atomic; a no-op returning the existing entry if present).
    pub fn insert(&self, package: &Package) -> Result<StoreEntry, StoreError> {
        let hash = package.hash().clone();
        let dir = self.dir_of(&hash);
        let entry = StoreEntry {
            hash: hash.clone(),
            dir: dir.clone(),
            manifest: package.manifest().clone(),
        };
        if dir.is_dir() {
            return Ok(entry);
        }
        // Stage in `.tmp-<random>/package/` (the package directory itself has an ordinary name).
        let tmp = self.new_tmp_dir()?;
        let result = Self::stage_and_publish(package, &tmp.join("package"), &dir);
        // Empty on success; a failed or redundant copy otherwise.
        let cleanup = remove_tree(&tmp);
        result?;
        cleanup?;
        Ok(entry)
    }

    /// Unpack into `staged`, verify the copy, freeze it and rename it to `dir`.
    fn stage_and_publish(package: &Package, staged: &Path, dir: &Path) -> Result<(), StoreError> {
        package.unpack_to(staged)?;
        let (copy, ignored) = Package::from_dir(staged)?;
        if copy.hash() != package.hash() {
            return Err(StoreError::Package(PackageError::HashMismatch {
                expected: package.hash().clone(),
                found: copy.hash().clone(),
            }));
        }
        if !ignored.is_empty() {
            return Err(StoreError::Corrupt(package.hash().clone()));
        }
        // Everything below the top directory now; the top one after the move (moving a directory
        // to another parent needs write permission on it).
        set_read_only_below(staged)?;
        match std::fs::rename(staged, dir) {
            Ok(()) => set_mode(dir, true, false),
            // Someone else inserted it first: theirs is identical.
            Err(_) if dir.is_dir() => Ok(()),
            Err(e) => Err(io(dir, e)),
        }
    }

    /// A fresh, empty `.tmp-<random>` directory in the root.
    fn new_tmp_dir(&self) -> Result<PathBuf, StoreError> {
        loop {
            let tmp = self.tmp_name();
            match std::fs::create_dir(&tmp) {
                Ok(()) => return Ok(tmp),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(io(&tmp, e)),
            }
        }
    }

    /// An unused-looking `.tmp-<random>` path in the root.
    fn tmp_name(&self) -> PathBuf {
        use std::hash::{BuildHasher, Hasher};
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        // RandomState is seeded randomly per process; the counter separates calls within one.
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
        hasher.write_u32(std::process::id());
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        hasher.write_u128(nanos);
        self.root
            .join(format!("{TMP_PREFIX}{:016x}", hasher.finish()))
    }

    /// Whether `hash` is installed.
    pub fn contains(&self, hash: &PackageHash) -> bool {
        self.dir_of(hash).is_dir()
    }

    /// The directory an entry lives in (whether or not it exists).
    pub fn dir_of(&self, hash: &PackageHash) -> PathBuf {
        self.root.join(hash.hex())
    }

    /// Load one entry's manifest.
    pub fn get(&self, hash: &PackageHash) -> Result<Option<StoreEntry>, StoreError> {
        let dir = self.dir_of(hash);
        if !dir.is_dir() {
            return Ok(None);
        }
        let manifest =
            ModManifest::from_file(&dir.join("mod.toml")).map_err(PackageError::Manifest)?;
        Ok(Some(StoreEntry {
            hash: hash.clone(),
            dir,
            manifest,
        }))
    }

    /// The hashes of every entry (whatever their state), sorted.
    pub fn hashes(&self) -> Result<Vec<PackageHash>, StoreError> {
        let mut hashes = Vec::new();
        for item in std::fs::read_dir(&self.root).map_err(|e| io(&self.root, e))? {
            let item = item.map_err(|e| io(&self.root, e))?;
            let Some(name) = item.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(hash) = PackageHash::parse(&format!("sha256:{name}")) else {
                continue;
            };
            if item.file_type().map_err(|e| io(&item.path(), e))?.is_dir() {
                hashes.push(hash);
            }
        }
        hashes.sort();
        Ok(hashes)
    }

    /// Every entry, sorted by (id, version). Fails on the first entry whose manifest cannot be
    /// read; see [`Store::list_lenient`].
    pub fn list(&self) -> Result<Vec<StoreEntry>, StoreError> {
        let (entries, mut broken) = self.list_lenient()?;
        match broken.drain(..).next() {
            Some((_, e)) => Err(e),
            None => Ok(entries),
        }
    }

    /// Every readable entry, sorted by (id, version, hash), plus the entries whose manifest could
    /// not be read (sorted by hash).
    #[allow(clippy::type_complexity)]
    pub fn list_lenient(
        &self,
    ) -> Result<(Vec<StoreEntry>, Vec<(PackageHash, StoreError)>), StoreError> {
        let mut entries = Vec::new();
        let mut broken = Vec::new();
        for hash in self.hashes()? {
            match self.get(&hash) {
                Ok(Some(entry)) => entries.push(entry),
                Ok(None) => {} // removed concurrently
                Err(e) => broken.push((hash, e)),
            }
        }
        sort_entries(&mut entries);
        Ok((entries, broken))
    }

    /// Re-hash one entry; `Ok(false)` if its contents no longer match.
    ///
    /// An entry that no longer forms a valid package, or holds files beyond the package's, is
    /// corrupt too. A missing entry is an I/O error (`NotFound`).
    pub fn verify(&self, hash: &PackageHash) -> Result<bool, StoreError> {
        let dir = self.dir_of(hash);
        if !dir.is_dir() {
            return Err(io(
                &dir,
                std::io::Error::new(std::io::ErrorKind::NotFound, "no such store entry"),
            ));
        }
        match Package::from_dir(&dir) {
            Ok((package, ignored)) => Ok(package.hash() == hash && ignored.is_empty()),
            Err(PackageError::Io { path, source }) => Err(StoreError::Io { path, source }),
            Err(_) => Ok(false),
        }
    }

    /// Re-hash every entry: the hashes of the corrupt ones, sorted.
    pub fn verify_all(&self) -> Result<Vec<PackageHash>, StoreError> {
        let mut corrupt = Vec::new();
        for hash in self.hashes()? {
            if !self.verify(&hash)? {
                corrupt.push(hash);
            }
        }
        Ok(corrupt)
    }

    /// Delete every entry not in `keep`; returns what was (or, with `dry_run`, would be) removed.
    ///
    /// Unreadable unreferenced entries and stale temporaries are removed too; [`Store::gc_report`]
    /// lists them.
    pub fn gc(
        &self,
        keep: &BTreeSet<PackageHash>,
        dry_run: bool,
    ) -> Result<Vec<StoreEntry>, StoreError> {
        Ok(self.gc_report(keep, dry_run)?.removed)
    }

    /// [`Store::gc`] with the full report.
    pub fn gc_report(
        &self,
        keep: &BTreeSet<PackageHash>,
        dry_run: bool,
    ) -> Result<GcReport, StoreError> {
        let mut report = GcReport::default();
        for hash in self.hashes()? {
            if keep.contains(&hash) {
                continue;
            }
            match self.get(&hash) {
                Ok(Some(entry)) => report.removed.push(entry),
                Ok(None) => continue,
                Err(_) => report.removed_unreadable.push(hash),
            }
        }
        let now = std::time::SystemTime::now();
        for item in std::fs::read_dir(&self.root).map_err(|e| io(&self.root, e))? {
            let item = item.map_err(|e| io(&self.root, e))?;
            let is_tmp = item
                .file_name()
                .to_str()
                .is_some_and(|n| n.starts_with(TMP_PREFIX));
            // Unknown age counts as old.
            let young = item
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| now.duration_since(t).ok())
                .is_some_and(|age| age < TEMPORARY_MIN_AGE);
            if is_tmp && !young {
                report.temporaries.push(item.path());
            }
        }
        report.temporaries.sort();
        sort_entries(&mut report.removed);
        if !dry_run {
            for hash in report
                .removed
                .iter()
                .map(|e| &e.hash)
                .chain(&report.removed_unreadable)
            {
                self.remove(hash)?;
            }
            for tmp in &report.temporaries {
                remove_tree(tmp)?;
            }
        }
        Ok(report)
    }

    /// Delete one entry; `Ok(false)` if it was not installed.
    pub fn remove(&self, hash: &PackageHash) -> Result<bool, StoreError> {
        let dir = self.dir_of(hash);
        if !dir.is_dir() {
            return Ok(false);
        }
        // Move it out of the way first: a half-deleted tree must never look like an entry.
        let doomed = self.tmp_name();
        match std::fs::rename(&dir, &doomed) {
            Ok(()) => {}
            // Removed concurrently.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(io(&dir, e)),
        }
        remove_tree(&doomed)?;
        Ok(true)
    }
}

fn sort_entries(entries: &mut [StoreEntry]) {
    entries.sort_by(|a, b| {
        (a.manifest.id(), a.manifest.version(), &a.hash).cmp(&(
            b.manifest.id(),
            b.manifest.version(),
            &b.hash,
        ))
    });
}

fn io(path: &Path, source: std::io::Error) -> StoreError {
    StoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Make everything below `dir` read-only: files `0444`, directories `0555`.
fn set_read_only_below(dir: &Path) -> Result<(), StoreError> {
    for item in std::fs::read_dir(dir).map_err(|e| io(dir, e))? {
        let item = item.map_err(|e| io(dir, e))?;
        let path = item.path();
        let kind = item.file_type().map_err(|e| io(&path, e))?;
        if kind.is_dir() {
            set_read_only_below(&path)?;
            set_mode(&path, true, false)?;
        } else if kind.is_file() {
            set_mode(&path, false, false)?;
        }
    }
    Ok(())
}

/// Delete a tree that may be read-only: restore write permission on every directory first.
/// Something already gone is fine; a plain file is removed too.
fn remove_tree(dir: &Path) -> Result<(), StoreError> {
    let gone = |e: &std::io::Error| e.kind() == std::io::ErrorKind::NotFound;
    let result = match std::fs::symlink_metadata(dir) {
        Ok(meta) if meta.is_dir() => {
            make_writable(dir)?;
            std::fs::remove_dir_all(dir)
        }
        Ok(_) => std::fs::remove_file(dir),
        Err(e) => Err(e),
    };
    match result {
        Err(e) if !gone(&e) => Err(io(dir, e)),
        _ => Ok(()),
    }
}

fn make_writable(dir: &Path) -> Result<(), StoreError> {
    let meta = match std::fs::symlink_metadata(dir) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(io(dir, e)),
    };
    if !meta.is_dir() {
        return Ok(());
    }
    set_mode(dir, true, true)?;
    for item in std::fs::read_dir(dir).map_err(|e| io(dir, e))? {
        let item = item.map_err(|e| io(dir, e))?;
        let path = item.path();
        let kind = item.file_type().map_err(|e| io(&path, e))?;
        if kind.is_dir() {
            make_writable(&path)?;
        } else if kind.is_file() {
            set_mode(&path, false, true)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, is_dir: bool, writable: bool) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = match (is_dir, writable) {
        (true, false) => 0o555,
        (true, true) => 0o755,
        (false, false) => 0o444,
        (false, true) => 0o644,
    };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).map_err(|e| io(path, e))
}

#[cfg(not(unix))]
fn set_mode(path: &Path, _is_dir: bool, writable: bool) -> Result<(), StoreError> {
    let mut perms = std::fs::metadata(path)
        .map_err(|e| io(path, e))?
        .permissions();
    perms.set_readonly(!writable);
    std::fs::set_permissions(path, perms).map_err(|e| io(path, e))
}

/// Store failures.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// A package failed validation.
    #[error(transparent)]
    Package(#[from] PackageError),
    /// An entry's directory name does not match its contents.
    #[error("store entry {0} is corrupt (contents do not match its hash)")]
    Corrupt(PackageHash),
    /// I/O.
    #[error("{path}: {source}")]
    Io {
        /// Path.
        path: PathBuf,
        /// Error.
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests;
