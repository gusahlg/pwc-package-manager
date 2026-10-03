//! Instance directories and the locking operation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pwc_manifest::{
    InstanceManifest, InstanceName, LockedPackage, Lockfile, ManifestError, ModManifest, ModSource,
    PackageHash, PackageId, PackageSource, Version,
};
use pwc_resolver::{Candidate, Problem, Requirement};
use pwc_store::Store;

use crate::fsutil::{io, tmp_sibling, write_atomic};
use crate::{Available, AvailableSource, Config, Dirs, InstanceError, PwcSource};

/// An instance on disk.
#[derive(Clone, Debug)]
pub struct Instance {
    /// Its directory.
    pub dir: PathBuf,
    /// Its `instance.toml`.
    pub manifest: InstanceManifest,
}

const MANIFEST_FILE: &str = "instance.toml";

impl Instance {
    /// Create `<dirs.instances>/<name>/instance.toml` (error if it exists).
    ///
    /// Also creates the instance's `game/` directory.
    pub fn create(dirs: &Dirs, manifest: InstanceManifest) -> Result<Self, InstanceError> {
        let instances = dirs.instances();
        std::fs::create_dir_all(&instances).map_err(|e| io(&instances, e))?;
        let dir = instances.join(manifest.name.as_str());
        match std::fs::create_dir(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if dir.join(MANIFEST_FILE).exists() {
                    return Err(InstanceError::InstanceExists(manifest.name.to_string()));
                }
                // A leftover directory without instance.toml: take it over.
            }
            Err(e) => return Err(io(&dir, e)),
        }
        let instance = Self { dir, manifest };
        let game = instance.game_dir();
        std::fs::create_dir_all(&game).map_err(|e| io(&game, e))?;
        instance.save()?;
        Ok(instance)
    }

    /// Open by name.
    pub fn open(dirs: &Dirs, name: &InstanceName) -> Result<Self, InstanceError> {
        let dir = dirs.instances().join(name.as_str());
        let path = dir.join(MANIFEST_FILE);
        if !path.is_file() {
            return Err(InstanceError::NoInstance(name.to_string()));
        }
        let manifest = InstanceManifest::from_file(&path).map_err(|e| file_error(&path, e))?;
        if &manifest.name != name {
            return Err(InstanceError::Manifest(ManifestError::invalid(
                "name",
                manifest.name.as_str(),
                format!(
                    "does not match the instance directory `{}` ({})",
                    name,
                    dir.display()
                ),
            )));
        }
        Ok(Self { dir, manifest })
    }

    /// Every instance, sorted by name.
    ///
    /// Directories whose name is not a valid instance name, or without `instance.toml`, are
    /// skipped; an unreadable `instance.toml` is an error.
    pub fn list(dirs: &Dirs) -> Result<Vec<Self>, InstanceError> {
        let root = dirs.instances();
        let items = match std::fs::read_dir(&root) {
            Ok(items) => items,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io(&root, e)),
        };
        let mut names = Vec::new();
        for item in items {
            let item = item.map_err(|e| io(&root, e))?;
            let Some(Ok(name)) = item.file_name().to_str().map(InstanceName::parse) else {
                continue;
            };
            if item.path().join(MANIFEST_FILE).is_file() {
                names.push(name);
            }
        }
        names.sort();
        names.iter().map(|name| Self::open(dirs, name)).collect()
    }

    /// Delete the instance directory.
    ///
    /// The directory is first renamed to a hidden name, so a half-deleted instance is never listed.
    pub fn remove(self) -> Result<(), InstanceError> {
        let doomed = tmp_sibling(&self.dir);
        std::fs::rename(&self.dir, &doomed).map_err(|e| io(&self.dir, e))?;
        std::fs::remove_dir_all(&doomed).map_err(|e| io(&doomed, e))
    }

    /// Write `instance.toml`.
    pub fn save(&self) -> Result<(), InstanceError> {
        write_atomic(
            &self.manifest_path(),
            self.manifest.to_toml_string().as_bytes(),
        )
    }

    /// `instance.toml` path.
    pub fn manifest_path(&self) -> PathBuf {
        self.dir.join(MANIFEST_FILE)
    }

    /// `pwc.lock` path.
    pub fn lock_path(&self) -> PathBuf {
        self.dir.join("pwc.lock")
    }

    /// The game data directory (`game/`).
    pub fn game_dir(&self) -> PathBuf {
        self.dir.join("game")
    }

    /// The current lock, if any.
    pub fn lockfile(&self) -> Result<Option<Lockfile>, InstanceError> {
        let path = self.lock_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(Some(
                Lockfile::parse(&text).map_err(|e| file_error(&path, e))?,
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io(&path, e)),
        }
    }

    /// Resolve a `path` entry relative to the instance directory.
    pub fn resolve_path(&self, p: &Path) -> PathBuf {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.dir.join(p)
        }
    }

    /// [`Instance::resolve_path`], canonicalised when it exists: the form recorded in `pwc.lock`.
    pub fn source_path(&self, p: &Path) -> PathBuf {
        let resolved = self.resolve_path(p);
        std::fs::canonicalize(&resolved).unwrap_or(resolved)
    }

    /// Why `lock` no longer describes this instance built against `pwc`, or `None` if it does.
    ///
    /// Stale when: the PWC source differs from `[pwc]` (directory, version, API, revision or
    /// dirty state); a `[mods]` entry is missing from the lock, no longer matches its locked
    /// version, or changed source kind; a `path`/`file` entry's contents changed (re-packaged and
    /// re-hashed now); or the lock holds packages nothing requires any more.
    pub fn stale_reason(
        &self,
        lock: &Lockfile,
        pwc: &PwcSource,
    ) -> Result<Option<String>, InstanceError> {
        let locked = &lock.pwc;
        if locked.source != pwc.dir || locked.version != pwc.version || locked.api != pwc.api {
            return Ok(Some(format!(
                "the PWC source changed ({} {} api {} → {} {} api {})",
                locked.source.display(),
                locked.version,
                locked.api,
                pwc.dir.display(),
                pwc.version,
                pwc.api
            )));
        }
        if locked.revision != pwc.revision || locked.dirty != pwc.dirty {
            return Ok(Some(format!(
                "the PWC source moved ({}{} → {}{})",
                locked.revision,
                if locked.dirty { " dirty" } else { "" },
                pwc.revision,
                if pwc.dirty { " dirty" } else { "" }
            )));
        }
        for (id, req) in &self.manifest.mods {
            let Some(package) = lock.package(id) else {
                return Ok(Some(format!("{id} is not locked yet")));
            };
            if !req.version.matches(&package.version) {
                return Ok(Some(format!(
                    "{id} {} no longer matches `{}`",
                    package.version, req.version
                )));
            }
            match (&req.source, &package.source) {
                (ModSource::Registry, PackageSource::Repository(_) | PackageSource::Store) => {}
                (ModSource::Path(p), PackageSource::Path(locked_path))
                    if self.source_path(p) == *locked_path =>
                {
                    let now = AvailableSource::Path(locked_path.clone()).read_package()?;
                    if now.is_some_and(|now| now.hash() != &package.hash) {
                        return Ok(Some(format!("{id} changed in {}", locked_path.display())));
                    }
                }
                (ModSource::File(p), PackageSource::File(locked_path))
                    if self.source_path(p) == *locked_path =>
                {
                    let now = AvailableSource::File(locked_path.clone()).read_package()?;
                    if now.is_some_and(|now| now.hash() != &package.hash) {
                        return Ok(Some(format!("{id} changed in {}", locked_path.display())));
                    }
                }
                _ => return Ok(Some(format!("{id} has a new source"))),
            }
        }
        // Everything locked must still be needed.
        let mut needed: BTreeSet<&PackageId> = BTreeSet::new();
        let mut queue: Vec<&PackageId> = self.manifest.mods.keys().collect();
        while let Some(id) = queue.pop() {
            if needed.insert(id)
                && let Some(package) = lock.package(id)
            {
                queue.extend(&package.dependencies);
            }
        }
        if let Some(extra) = lock.packages.iter().find(|p| !needed.contains(&p.id)) {
            return Ok(Some(format!("{} is no longer required", extra.id)));
        }
        Ok(None)
    }
}

/// A manifest error with the file it came from (I/O errors already name it).
fn file_error(path: &Path, e: ManifestError) -> InstanceError {
    match e {
        ManifestError::Io { .. } => InstanceError::Manifest(e),
        source => InstanceError::ManifestAt {
            path: path.to_path_buf(),
            source,
        },
    }
}

/// How to lock.
#[derive(Clone, Debug, Default)]
pub struct LockOptions {
    /// Ids to update to their newest compatible version.
    pub update: BTreeSet<PackageId>,
    /// Update everything.
    pub update_all: bool,
}

/// What locking did.
#[derive(Clone, Debug)]
pub struct LockReport {
    /// The written lockfile.
    pub lock: Lockfile,
    /// `(id, old version, new version)` for every change (`None` = added/removed). A package
    /// whose contents changed at the same version (a `path` tree) appears with equal versions.
    pub changes: Vec<(PackageId, Option<String>, Option<String>)>,
    /// Non-fatal discovery warnings.
    pub warnings: Vec<String>,
}

/// Resolve `instance` against the configured repositories and the store, insert every selected
/// package into the store, and write `pwc.lock`.
///
/// Steps: check the instance's `pwc` requirement against the PWC version; scan the repositories
/// and the store; add every `path`/`file` entry as a pinned candidate (its id and any asserted
/// version must match); resolve with the previous lock as preference (unless updating); refuse a
/// published package (not a `path` tree) whose contents changed since the previous lock without
/// a version bump; insert every selected package into the store (re-reading its source and
/// checking the hash); write `pwc.lock` atomically.
pub fn lock_instance(
    dirs: &Dirs,
    config: &Config,
    instance: &Instance,
    pwc: &PwcSource,
    options: &LockOptions,
) -> Result<LockReport, InstanceError> {
    let manifest = &instance.manifest;
    if !manifest.pwc.matches(&pwc.version) {
        return Err(InstanceError::PwcSource(format!(
            "instance `{}` requires PWC `{}`, but {} is version {}",
            manifest.name,
            manifest.pwc,
            pwc.dir.display(),
            pwc.version
        )));
    }
    let store = Store::open(dirs.store())?;
    let mut available = Available::scan(config, &store)?;
    let mut warnings = std::mem::take(&mut available.warnings);
    let previous = match instance.lockfile() {
        Ok(lock) => lock,
        Err(e) => {
            warnings.push(format!("ignoring the existing pwc.lock: {e}"));
            None
        }
    };

    let mut roots = BTreeMap::new();
    for (id, req) in &manifest.mods {
        let source = match &req.source {
            ModSource::Registry => {
                roots.insert(id.clone(), Requirement::Version(req.version.clone()));
                continue;
            }
            ModSource::Path(p) => AvailableSource::Path(instance.source_path(p)),
            ModSource::File(p) => AvailableSource::File(instance.source_path(p)),
        };
        let index = available.add_explicit(source)?;
        let found = &available.packages[index];
        let where_ = found.source.to_string();
        if found.manifest.id() != id {
            return Err(InstanceError::Manifest(ManifestError::invalid(
                format!("mods.{id}"),
                where_,
                format!("the package there is `{}`, not `{id}`", found.manifest.id()),
            )));
        }
        if !req.version.matches(found.manifest.version()) {
            return Err(InstanceError::Manifest(ManifestError::invalid(
                format!("mods.{id}.version"),
                req.version.to_string(),
                format!(
                    "the package at {where_} is version {}",
                    found.manifest.version()
                ),
            )));
        }
        roots.insert(id.clone(), Requirement::Pin(index));
    }

    let candidates = available
        .packages
        .iter()
        .enumerate()
        .map(|(source, p)| Candidate {
            id: p.manifest.id().clone(),
            version: p.manifest.version().clone(),
            kind: p.manifest.kind(),
            hash: p.hash.clone(),
            pwc_api: p.manifest.package.pwc_api.clone(),
            dependencies: p.manifest.dependencies.clone(),
            conflicts: p.manifest.conflicts.clone(),
            source,
        })
        .collect();
    let problem = Problem {
        api: Some(pwc.api.clone()),
        roots,
        candidates,
        previous: previous
            .iter()
            .flat_map(|l| &l.packages)
            .map(|p| (p.id.clone(), p.version.clone()))
            .collect(),
        update: options.update.clone(),
        update_all: options.update_all,
    };
    let resolution = pwc_resolver::resolve(&problem)?;

    // Published versions are immutable.
    for selected in &resolution.packages {
        let offered = &available.packages[selected.candidate.source];
        if offered.source.is_mutable() {
            continue;
        }
        if let Some(old) = previous
            .as_ref()
            .and_then(|l| l.package(offered.manifest.id()))
            && &old.version == offered.manifest.version()
            && old.hash != offered.hash
            && !matches!(old.source, PackageSource::Path(_))
        {
            return Err(InstanceError::ContentsChanged {
                id: old.id.clone(),
                version: old.version.clone(),
                locked: old.hash.clone(),
                found: offered.hash.clone(),
            });
        }
    }

    // Fill the store.
    for selected in &resolution.packages {
        let offered = &available.packages[selected.candidate.source];
        if store.contains(&offered.hash) {
            continue;
        }
        let Some(package) = offered.source.read_package()? else {
            return Err(InstanceError::NotInstalled {
                id: offered.manifest.id().clone(),
                hash: offered.hash.clone(),
            });
        };
        if package.hash() != &offered.hash {
            return Err(InstanceError::PackageAt {
                path: offered
                    .source
                    .path()
                    .map(Path::to_path_buf)
                    .unwrap_or_default(),
                source: pwc_package::PackageError::HashMismatch {
                    expected: offered.hash.clone(),
                    found: package.hash().clone(),
                },
            });
        }
        store.insert(&package)?;
    }

    let packages = resolution
        .packages
        .iter()
        .map(|selected| {
            let offered = &available.packages[selected.candidate.source];
            LockedPackage {
                id: selected.candidate.id.clone(),
                version: selected.candidate.version.clone(),
                kind: selected.candidate.kind,
                hash: selected.candidate.hash.clone(),
                source: match &offered.source {
                    AvailableSource::RepositoryDir(p) | AvailableSource::RepositoryFile(p) => {
                        PackageSource::Repository(p.clone())
                    }
                    AvailableSource::Path(p) => PackageSource::Path(p.clone()),
                    AvailableSource::File(p) => PackageSource::File(p.clone()),
                    AvailableSource::Store => PackageSource::Store,
                },
                dependencies: selected.dependencies.clone(),
            }
        })
        .collect();
    let lock = Lockfile::new(pwc.locked(), packages);
    write_atomic(&instance.lock_path(), lock.to_toml_string().as_bytes())?;

    Ok(LockReport {
        changes: changes(previous.as_ref(), &lock),
        lock,
        warnings,
    })
}

/// `(id, old, new)` versions for every package added, removed, or changed (version or hash).
fn changes(
    previous: Option<&Lockfile>,
    lock: &Lockfile,
) -> Vec<(PackageId, Option<String>, Option<String>)> {
    let old: BTreeMap<&PackageId, (&Version, &PackageHash)> = previous
        .iter()
        .flat_map(|l| &l.packages)
        .map(|p| (&p.id, (&p.version, &p.hash)))
        .collect();
    let new: BTreeMap<&PackageId, (&Version, &PackageHash)> = lock
        .packages
        .iter()
        .map(|p| (&p.id, (&p.version, &p.hash)))
        .collect();
    let ids: BTreeSet<&PackageId> = old.keys().chain(new.keys()).copied().collect();
    ids.into_iter()
        .filter_map(|id| {
            let (o, n) = (old.get(id), new.get(id));
            (o != n).then(|| {
                (
                    (*id).clone(),
                    o.map(|(v, _)| v.to_string()),
                    n.map(|(v, _)| v.to_string()),
                )
            })
        })
        .collect()
}

/// Every hash referenced by any instance's `pwc.lock` (what `pwc store gc` keeps).
///
/// Looks at every directory under `instances/` that has a `pwc.lock`, whether or not its
/// `instance.toml` is readable. An unreadable lock is an error: collecting garbage without knowing
/// what it references could delete packages still in use.
pub fn referenced_hashes(dirs: &Dirs) -> Result<BTreeSet<PackageHash>, InstanceError> {
    let root = dirs.instances();
    let items = match std::fs::read_dir(&root) {
        Ok(items) => items,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(e) => return Err(io(&root, e)),
    };
    let mut hashes = BTreeSet::new();
    for item in items {
        let item = item.map_err(|e| io(&root, e))?;
        let lock = item.path().join("pwc.lock");
        if lock.is_file() {
            let parsed = Lockfile::from_file(&lock).map_err(|e| file_error(&lock, e))?;
            hashes.extend(parsed.packages.into_iter().map(|p| p.hash));
        }
    }
    Ok(hashes)
}

/// The store directory and manifest of every package in `lock` (what the builder needs).
pub fn installed_packages(
    store: &Store,
    lock: &Lockfile,
) -> Result<BTreeMap<PackageId, (PathBuf, ModManifest)>, InstanceError> {
    let mut out = BTreeMap::new();
    for package in &lock.packages {
        let entry = store
            .get(&package.hash)?
            .ok_or_else(|| InstanceError::NotInstalled {
                id: package.id.clone(),
                hash: package.hash.clone(),
            })?;
        out.insert(package.id.clone(), (entry.dir, entry.manifest));
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
