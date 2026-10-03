//! Candidate discovery: configured repositories (source dirs and `.pwcmod` files), the store, and
//! explicit `path`/`file` entries (docs/spec/filesystem.md, "Repositories").

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pwc_manifest::{ModManifest, PackageHash, PackageId, Version};
use pwc_package::Package;

use crate::{Config, InstanceError};

/// Where an available package can be obtained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AvailableSource {
    /// A source directory inside a repository.
    RepositoryDir(PathBuf),
    /// A `.pwcmod` inside a repository.
    RepositoryFile(PathBuf),
    /// An explicit local source tree.
    Path(PathBuf),
    /// An explicit `.pwcmod`.
    File(PathBuf),
    /// Installed in the store.
    Store,
}

impl AvailableSource {
    /// Whether this is a mutable development tree (an explicit `path` entry).
    pub fn is_mutable(&self) -> bool {
        matches!(self, AvailableSource::Path(_))
    }

    /// The file or directory, if not the store.
    pub fn path(&self) -> Option<&Path> {
        match self {
            AvailableSource::RepositoryDir(p)
            | AvailableSource::RepositoryFile(p)
            | AvailableSource::Path(p)
            | AvailableSource::File(p) => Some(p),
            AvailableSource::Store => None,
        }
    }

    /// Read the package from this source. `Ok(None)` for the store (it is already installed).
    pub fn read_package(&self) -> Result<Option<Package>, InstanceError> {
        fn at(path: &Path) -> impl FnOnce(pwc_package::PackageError) -> InstanceError + '_ {
            move |source| InstanceError::PackageAt {
                path: path.to_path_buf(),
                source,
            }
        }
        match self {
            AvailableSource::RepositoryDir(p) | AvailableSource::Path(p) => Package::from_dir(p)
                .map(|(package, _ignored)| Some(package))
                .map_err(at(p)),
            AvailableSource::RepositoryFile(p) | AvailableSource::File(p) => {
                Package::from_archive_file(p).map(Some).map_err(at(p))
            }
            AvailableSource::Store => Ok(None),
        }
    }
}

impl std::fmt::Display for AvailableSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.path() {
            Some(path) => write!(f, "{}", path.display()),
            None => f.write_str("the store"),
        }
    }
}

/// One available package version.
#[derive(Clone, Debug)]
pub struct AvailablePackage {
    /// Manifest.
    pub manifest: ModManifest,
    /// Hash (computed by packaging the source).
    pub hash: PackageHash,
    /// Where it is.
    pub source: AvailableSource,
}

/// Everything discoverable.
#[derive(Clone, Debug, Default)]
pub struct Available {
    /// Packages (duplicates with the same hash merged, preferring non-store sources).
    pub packages: Vec<AvailablePackage>,
    /// Problems with individual entries (invalid packages are skipped, not fatal).
    pub warnings: Vec<String>,
}

impl Available {
    /// Scan every configured repository and the store.
    ///
    /// Repositories are scanned in configuration order, each one's children sorted by name: a
    /// directory containing `mod.toml` is packaged on the fly, a `*.pwcmod` file is read and
    /// verified, anything else (hidden entries, READMEs, directories without `mod.toml`) is
    /// ignored. Store entries come last. Invalid packages and missing repositories become
    /// warnings. The same hash found twice is kept once (the first, so a repository wins over the
    /// store); the same id and version with *different* contents is kept once too (the first),
    /// with a warning naming both, since published versions are immutable.
    pub fn scan(config: &Config, store: &pwc_store::Store) -> Result<Self, InstanceError> {
        let mut scan = Scan::default();
        for repo in &config.repositories {
            let children = match std::fs::read_dir(&repo.path) {
                Ok(items) => {
                    let mut children: Vec<PathBuf> =
                        items.filter_map(|i| i.ok().map(|i| i.path())).collect();
                    children.sort();
                    children
                }
                Err(e) => {
                    scan.available.warnings.push(format!(
                        "repository `{}` ({}): {e}",
                        repo.name,
                        repo.path.display()
                    ));
                    continue;
                }
            };
            for child in children {
                if child
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_none_or(|n| n.starts_with('.'))
                {
                    continue;
                }
                let source = if child.is_dir() && child.join("mod.toml").is_file() {
                    AvailableSource::RepositoryDir(child)
                } else if child.is_file() && child.extension().is_some_and(|e| e == "pwcmod") {
                    AvailableSource::RepositoryFile(child)
                } else {
                    continue;
                };
                match source.read_package() {
                    Ok(Some(package)) => scan.offer(AvailablePackage {
                        manifest: package.manifest().clone(),
                        hash: package.hash().clone(),
                        source,
                    }),
                    Ok(None) => {}
                    Err(e) => scan.available.warnings.push(format!("skipping {e}")),
                }
            }
        }
        let (entries, broken) = store.list_lenient()?;
        for entry in entries {
            scan.offer(AvailablePackage {
                manifest: entry.manifest,
                hash: entry.hash,
                source: AvailableSource::Store,
            });
        }
        for (hash, e) in broken {
            scan.available.warnings.push(format!(
                "skipping store entry {hash}: {e} (see `pwc store verify`)"
            ));
        }
        Ok(scan.available)
    }

    /// Add one explicit local source tree or `.pwcmod`, returning its index in `packages`.
    ///
    /// Always appended (even when the same hash is already known) so the caller can pin exactly
    /// this source. `RepositoryDir`/`RepositoryFile` are read like `Path`/`File`; `Store` is
    /// rejected.
    pub fn add_explicit(&mut self, source: AvailableSource) -> Result<usize, InstanceError> {
        let Some(package) = source.read_package()? else {
            return Err(InstanceError::Config(
                "a store entry cannot be added as an explicit source".into(),
            ));
        };
        self.packages.push(AvailablePackage {
            manifest: package.manifest().clone(),
            hash: package.hash().clone(),
            source,
        });
        Ok(self.packages.len() - 1)
    }

    /// Every package with this id, sorted by version (ascending).
    pub fn versions_of(&self, id: &PackageId) -> Vec<&AvailablePackage> {
        let mut found: Vec<&AvailablePackage> = self
            .packages
            .iter()
            .filter(|p| p.manifest.id() == id)
            .collect();
        found.sort_by(|a, b| a.manifest.version().cmp(b.manifest.version()));
        found
    }

    /// Every known id with its packages sorted by version (ascending).
    pub fn by_id(&self) -> BTreeMap<&PackageId, Vec<&AvailablePackage>> {
        let mut map: BTreeMap<&PackageId, Vec<&AvailablePackage>> = BTreeMap::new();
        for p in &self.packages {
            map.entry(p.manifest.id()).or_default().push(p);
        }
        for list in map.values_mut() {
            list.sort_by(|a, b| a.manifest.version().cmp(b.manifest.version()));
        }
        map
    }
}

/// Deduplication state of one scan.
#[derive(Default)]
struct Scan {
    available: Available,
    by_hash: BTreeMap<PackageHash, usize>,
    by_version: BTreeMap<(PackageId, Version), usize>,
}

impl Scan {
    fn offer(&mut self, package: AvailablePackage) {
        if self.by_hash.contains_key(&package.hash) {
            return;
        }
        let key = (
            package.manifest.id().clone(),
            package.manifest.version().clone(),
        );
        if let Some(&first) = self.by_version.get(&key) {
            let kept = &self.available.packages[first];
            self.available.warnings.push(format!(
                "{} {} is offered with different contents by {} ({}) and {} ({}); using the first \
                 (published versions are immutable: bump the version when the contents change)",
                key.0, key.1, kept.source, kept.hash, package.source, package.hash
            ));
            return;
        }
        let index = self.available.packages.len();
        self.by_hash.insert(package.hash.clone(), index);
        self.by_version.insert(key, index);
        self.available.packages.push(package);
    }
}
