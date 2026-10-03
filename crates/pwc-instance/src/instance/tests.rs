//! Instance and locking tests on temporary directories: a fake PWC source, a repository of small
//! valid packages, and the full lock → store → lockfile path.

use std::path::{Path, PathBuf};

use pwc_manifest::{ModKind, ModRequirement, VersionReq};
use pwc_package::Package;

use super::*;
use crate::RepositoryConfig;
use crate::testutil::{Pkg, fake_pwc};

fn pid(s: &str) -> PackageId {
    PackageId::parse(s).unwrap()
}

fn req(s: &str) -> VersionReq {
    VersionReq::parse(s).unwrap()
}

fn name(s: &str) -> InstanceName {
    InstanceName::parse(s).unwrap()
}

struct Env {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    dirs: Dirs,
    config: Config,
    pwc: PwcSource,
    repo: PathBuf,
}

impl Drop for Env {
    /// Store entries are read-only; give write permission back so the temp dir can be deleted.
    fn drop(&mut self) {
        crate::testutil::make_writable(&self.root);
    }
}

fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let dirs = Dirs::under(&root.join("home"));
    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let pwc = PwcSource::open(&fake_pwc(&root, "2.1.0", "1.0.0")).unwrap();
    let config = Config {
        pwc_source: Some(pwc.dir.clone()),
        repositories: vec![RepositoryConfig {
            name: "test".into(),
            path: repo.clone(),
        }],
        ..Config::default()
    };
    Env {
        _tmp: tmp,
        root,
        dirs,
        config,
        pwc,
        repo,
    }
}

impl Env {
    fn manifest(&self, instance: &str, mods: &[(&str, &str)]) -> InstanceManifest {
        let mut m = InstanceManifest::new(name(instance), req("^2.1"));
        for (id, r) in mods {
            m.mods.insert(
                pid(id),
                ModRequirement {
                    version: req(r),
                    source: ModSource::Registry,
                },
            );
        }
        m
    }

    fn instance(&self, instance: &str, mods: &[(&str, &str)]) -> Instance {
        Instance::create(&self.dirs, self.manifest(instance, mods)).unwrap()
    }

    fn lock_with(
        &self,
        instance: &Instance,
        options: &LockOptions,
    ) -> Result<LockReport, InstanceError> {
        lock_instance(&self.dirs, &self.config, instance, &self.pwc, options)
    }

    fn lock(&self, instance: &Instance) -> LockReport {
        self.lock_with(instance, &LockOptions::default()).unwrap()
    }

    fn store(&self) -> Store {
        Store::open(self.dirs.store()).unwrap()
    }
}

/// `id version` of every locked package.
fn locked(lock: &Lockfile) -> Vec<String> {
    lock.packages
        .iter()
        .map(|p| format!("{} {}", p.id, p.version))
        .collect()
}

fn change(
    id: &str,
    old: Option<&str>,
    new: Option<&str>,
) -> (PackageId, Option<String>, Option<String>) {
    (pid(id), old.map(String::from), new.map(String::from))
}

// ---------------------------------------------------------------------------------------------
// Instance directories.

#[test]
fn create_open_list_save_remove() {
    let e = env();
    assert!(
        Instance::list(&e.dirs).unwrap().is_empty(),
        "no instances directory yet"
    );

    let survival = e.instance("survival", &[]);
    assert_eq!(survival.dir, e.dirs.instances().join("survival"));
    assert!(survival.manifest_path().is_file());
    assert!(survival.game_dir().is_dir());
    assert_eq!(survival.lock_path(), survival.dir.join("pwc.lock"));
    assert!(survival.lockfile().unwrap().is_none());

    match Instance::create(&e.dirs, e.manifest("survival", &[])) {
        Err(InstanceError::InstanceExists(n)) => assert_eq!(n, "survival"),
        other => panic!("{other:?}"),
    }
    match Instance::open(&e.dirs, &name("nope")) {
        Err(InstanceError::NoInstance(n)) => assert_eq!(n, "nope"),
        other => panic!("{other:?}"),
    }

    let mut opened = Instance::open(&e.dirs, &name("survival")).unwrap();
    assert_eq!(opened.manifest, survival.manifest);
    opened.manifest.mods.insert(
        pid("t.hotbar"),
        ModRequirement {
            version: req("^1.0"),
            source: ModSource::Registry,
        },
    );
    opened.manifest.description = Some("with a hotbar".into());
    opened.save().unwrap();
    assert_eq!(
        Instance::open(&e.dirs, &name("survival")).unwrap().manifest,
        opened.manifest
    );

    e.instance("creative", &[]);
    // Not instances: a bad name, a hidden directory, a directory without instance.toml, a file.
    std::fs::create_dir_all(e.dirs.instances().join("Bad Name")).unwrap();
    std::fs::create_dir_all(e.dirs.instances().join(".hidden")).unwrap();
    std::fs::create_dir_all(e.dirs.instances().join("empty")).unwrap();
    std::fs::write(e.dirs.instances().join("file"), "").unwrap();
    let names: Vec<String> = Instance::list(&e.dirs)
        .unwrap()
        .iter()
        .map(|i| i.manifest.name.to_string())
        .collect();
    assert_eq!(names, ["creative", "survival"]);

    // A leftover directory without instance.toml can be taken over.
    let empty = Instance::create(&e.dirs, e.manifest("empty", &[])).unwrap();
    assert!(empty.manifest_path().is_file());

    Instance::open(&e.dirs, &name("survival"))
        .unwrap()
        .remove()
        .unwrap();
    assert!(!e.dirs.instances().join("survival").exists());
    let names: Vec<String> = Instance::list(&e.dirs)
        .unwrap()
        .iter()
        .map(|i| i.manifest.name.to_string())
        .collect();
    assert_eq!(names, ["creative", "empty"]);
    let leftovers: Vec<String> = std::fs::read_dir(e.dirs.instances())
        .unwrap()
        .map(|i| i.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("survival"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn open_rejects_a_mismatched_name_and_bad_files() {
    let e = env();
    let a = e.instance("alpha", &[]);
    let copy = e.dirs.instances().join("beta");
    std::fs::create_dir_all(&copy).unwrap();
    std::fs::copy(a.manifest_path(), copy.join("instance.toml")).unwrap();
    let err = Instance::open(&e.dirs, &name("beta"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("does not match"), "{err}");

    std::fs::write(a.manifest_path(), "format = 1\nthis is = not valid").unwrap();
    let err = Instance::open(&e.dirs, &name("alpha"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("instance.toml"), "{err}");

    std::fs::write(a.lock_path(), "garbage").unwrap();
    let a = Instance {
        dir: a.dir.clone(),
        manifest: e.manifest("alpha", &[]),
    };
    let err = a.lockfile().unwrap_err().to_string();
    assert!(err.contains("pwc.lock"), "{err}");
}

#[test]
fn resolve_path_is_relative_to_the_instance() {
    let e = env();
    let i = e.instance("dev", &[]);
    assert_eq!(i.resolve_path(Path::new("../x")), i.dir.join("../x"));
    assert_eq!(i.resolve_path(Path::new("/abs/x")), Path::new("/abs/x"));
}

// ---------------------------------------------------------------------------------------------
// Discovery.

#[test]
fn scan_reads_repositories_and_the_store() {
    let e = env();
    Pkg::new("t.hotbar", "1.0.0").write_into(&e.repo);
    Pkg::new("t.inventory", "1.0.0")
        .deps(&[("t.hotbar", "^1.0")])
        .write_into(&e.repo);
    // An archive.
    let (pkg, _) =
        Package::from_dir(&Pkg::new("t.archived", "0.1.0").write(&e.root.join("src/archived")))
            .unwrap();
    let archive = pkg.write_archive(&e.repo).unwrap();
    // Silently ignored: files, hidden entries, directories without mod.toml.
    std::fs::write(e.repo.join("README.md"), "# repo").unwrap();
    std::fs::create_dir_all(e.repo.join("docs")).unwrap();
    Pkg::new("t.hidden", "1.0.0").write(&e.repo.join(".hidden"));
    // Invalid: warned about and skipped.
    let broken = e.repo.join("broken");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(
        broken.join("mod.toml"),
        "format = 1\n[package]\nid = \"t.broken\"\n",
    )
    .unwrap();
    std::fs::write(e.repo.join("bad.pwcmod"), "not zstd").unwrap();

    let store = e.store();
    let a = Available::scan(&e.config, &store).unwrap();
    let ids: Vec<String> = a
        .packages
        .iter()
        .map(|p| format!("{} {}", p.manifest.id(), p.manifest.version()))
        .collect();
    assert_eq!(
        ids,
        ["t.archived 0.1.0", "t.hotbar 1.0.0", "t.inventory 1.0.0"]
    );
    assert_eq!(
        a.packages[0].source,
        AvailableSource::RepositoryFile(archive)
    );
    assert_eq!(
        a.packages[1].source,
        AvailableSource::RepositoryDir(e.repo.join("t.hotbar-1.0.0"))
    );
    assert_eq!(a.packages[0].hash, *pkg.hash());
    assert_eq!(a.warnings.len(), 2, "{:?}", a.warnings);
    assert!(a.warnings.iter().any(|w| w.contains("broken")));
    assert!(a.warnings.iter().any(|w| w.contains("bad.pwcmod")));
    assert_eq!(a.versions_of(&pid("t.hotbar")).len(), 1);
    assert_eq!(a.by_id().len(), 3);

    // Store entries: a duplicate of a repository package merges into the repository one.
    store.insert(&pkg).unwrap();
    let (other, _) =
        Package::from_dir(&Pkg::new("t.only-in-store", "2.0.0").write(&e.root.join("src/s")))
            .unwrap();
    store.insert(&other).unwrap();
    let a = Available::scan(&e.config, &store).unwrap();
    assert_eq!(a.packages.len(), 4);
    let archived: Vec<_> = a
        .packages
        .iter()
        .filter(|p| p.hash == *pkg.hash())
        .collect();
    assert_eq!(archived.len(), 1);
    assert!(matches!(
        archived[0].source,
        AvailableSource::RepositoryFile(_)
    ));
    let only = a.packages.iter().find(|p| p.hash == *other.hash()).unwrap();
    assert_eq!(only.source, AvailableSource::Store);
}

#[test]
fn scan_warns_about_missing_repositories_and_changed_versions() {
    let mut e = env();
    let second = e.root.join("second");
    Pkg::new("t.hotbar", "1.0.0").write_into(&e.repo);
    Pkg::new("t.hotbar", "1.0.0").write_into(&second); // identical: merged silently
    Pkg::new("t.inventory", "1.0.0").write_into(&e.repo);
    Pkg::new("t.inventory", "1.0.0")
        .code("// different\n")
        .write_into(&second); // same version, other contents
    e.config.repositories.push(RepositoryConfig {
        name: "second".into(),
        path: second.clone(),
    });
    e.config.repositories.push(RepositoryConfig {
        name: "gone".into(),
        path: e.root.join("gone"),
    });

    let a = Available::scan(&e.config, &e.store()).unwrap();
    assert_eq!(a.packages.len(), 2);
    let inventory = a
        .packages
        .iter()
        .find(|p| p.manifest.id().as_str() == "t.inventory")
        .unwrap();
    assert_eq!(
        inventory.source,
        AvailableSource::RepositoryDir(e.repo.join("t.inventory-1.0.0"))
    );
    assert_eq!(a.warnings.len(), 2, "{:?}", a.warnings);
    assert!(a.warnings.iter().any(|w| w.contains("`gone`")));
    let changed = a
        .warnings
        .iter()
        .find(|w| w.contains("t.inventory 1.0.0"))
        .unwrap();
    assert!(
        changed.contains("different contents") && changed.contains("second"),
        "{changed}"
    );
}

#[test]
fn add_explicit_appends_even_known_packages() {
    let e = env();
    let dir = Pkg::new("t.hotbar", "1.0.0").write_into(&e.repo);
    let mut a = Available::scan(&e.config, &e.store()).unwrap();
    assert_eq!(a.packages.len(), 1);
    let i = a.add_explicit(AvailableSource::Path(dir.clone())).unwrap();
    assert_eq!(i, 1);
    assert_eq!(a.packages[1].hash, a.packages[0].hash);
    assert_eq!(a.packages[1].source, AvailableSource::Path(dir));
    assert!(a.add_explicit(AvailableSource::Store).is_err());
    let err = a
        .add_explicit(AvailableSource::Path(e.root.join("missing")))
        .unwrap_err()
        .to_string();
    assert!(err.contains("missing"), "{err}");
}

// ---------------------------------------------------------------------------------------------
// Locking.

#[test]
fn lock_end_to_end() {
    let e = env();
    Pkg::new("t.hotbar", "1.0.0").write_into(&e.repo);
    Pkg::new("t.hotbar", "1.1.0").write_into(&e.repo);
    Pkg::new("t.hotbar", "2.0.0")
        .api("^2.0")
        .write_into(&e.repo); // needs another mod API
    Pkg::new("t.inventory", "1.0.0")
        .deps(&[("t.hotbar", "^1.0")])
        .write_into(&e.repo);
    let broken = e.repo.join("broken");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("mod.toml"), "not toml [").unwrap();

    let inst = e.instance("survival", &[("t.inventory", "^1.0")]);
    let report = e.lock(&inst);
    assert_eq!(
        locked(&report.lock),
        ["t.hotbar 1.1.0", "t.inventory 1.0.0"]
    );
    assert_eq!(
        report.changes,
        [
            change("t.hotbar", None, Some("1.1.0")),
            change("t.inventory", None, Some("1.0.0"))
        ]
    );
    assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);

    let hotbar = report.lock.package(&pid("t.hotbar")).unwrap();
    assert_eq!(hotbar.kind, ModKind::Mod);
    assert_eq!(
        hotbar.source,
        PackageSource::Repository(e.repo.join("t.hotbar-1.1.0"))
    );
    assert!(hotbar.dependencies.is_empty());
    let inventory = report.lock.package(&pid("t.inventory")).unwrap();
    assert_eq!(inventory.dependencies, [pid("t.hotbar")]);

    // [pwc] pins the source.
    assert_eq!(report.lock.pwc, e.pwc.locked());
    assert_eq!(report.lock.pwc.revision, "unknown");

    // Written, and identical to what was returned.
    assert_eq!(inst.lockfile().unwrap().unwrap(), report.lock);
    let text = std::fs::read_to_string(inst.lock_path()).unwrap();
    assert!(text.contains("repository:"), "{text}");

    // Both packages are in the store, verified, and readable for the builder.
    let store = e.store();
    for p in &report.lock.packages {
        assert!(store.contains(&p.hash));
        assert!(store.verify(&p.hash).unwrap());
    }
    let installed = installed_packages(&store, &report.lock).unwrap();
    assert_eq!(installed.len(), 2);
    let (dir, manifest) = &installed[&pid("t.inventory")];
    assert!(dir.join("src/lib.rs").is_file());
    assert_eq!(manifest.version().to_string(), "1.0.0");

    assert_eq!(inst.stale_reason(&report.lock, &e.pwc).unwrap(), None);
}

#[test]
fn relock_is_conservative_and_update_moves_forward() {
    let e = env();
    Pkg::new("t.hotbar", "1.0.0").write_into(&e.repo);
    Pkg::new("t.inventory", "1.0.0")
        .deps(&[("t.hotbar", "^1.0")])
        .write_into(&e.repo);
    let inst = e.instance("survival", &[("t.inventory", "^1.0")]);
    let first = e.lock(&inst);
    assert_eq!(locked(&first.lock), ["t.hotbar 1.0.0", "t.inventory 1.0.0"]);

    // Newer versions appear.
    Pkg::new("t.hotbar", "1.2.0").write_into(&e.repo);
    Pkg::new("t.inventory", "1.1.0")
        .deps(&[("t.hotbar", "^1.0")])
        .write_into(&e.repo);
    let again = e.lock(&inst);
    assert_eq!(again.lock, first.lock, "re-locking keeps what still works");
    assert!(again.changes.is_empty());

    // Update one id.
    let options = LockOptions {
        update: [pid("t.hotbar")].into(),
        update_all: false,
    };
    let updated = e.lock_with(&inst, &options).unwrap();
    assert_eq!(
        locked(&updated.lock),
        ["t.hotbar 1.2.0", "t.inventory 1.0.0"]
    );
    assert_eq!(
        updated.changes,
        [change("t.hotbar", Some("1.0.0"), Some("1.2.0"))]
    );

    // Update everything.
    let options = LockOptions {
        update_all: true,
        ..LockOptions::default()
    };
    let all = e.lock_with(&inst, &options).unwrap();
    assert_eq!(locked(&all.lock), ["t.hotbar 1.2.0", "t.inventory 1.1.0"]);
    assert_eq!(
        all.changes,
        [change("t.inventory", Some("1.0.0"), Some("1.1.0"))]
    );
}

#[test]
fn removing_a_mod_drops_its_dependencies() {
    let e = env();
    Pkg::new("t.hotbar", "1.0.0").write_into(&e.repo);
    Pkg::new("t.inventory", "1.0.0")
        .deps(&[("t.hotbar", "^1.0")])
        .write_into(&e.repo);
    Pkg::new("t.caves", "1.0.0").write_into(&e.repo);
    let mut inst = e.instance("survival", &[("t.inventory", "*"), ("t.caves", "*")]);
    let first = e.lock(&inst);
    assert_eq!(first.lock.packages.len(), 3);

    inst.manifest.mods.remove(&pid("t.inventory"));
    inst.save().unwrap();
    let reason = inst.stale_reason(&first.lock, &e.pwc).unwrap().unwrap();
    assert!(reason.contains("no longer required"), "{reason}");
    let second = e.lock(&inst);
    assert_eq!(locked(&second.lock), ["t.caves 1.0.0"]);
    assert_eq!(
        second.changes,
        [
            change("t.hotbar", Some("1.0.0"), None),
            change("t.inventory", Some("1.0.0"), None)
        ]
    );
    // The packages stay in the store until gc.
    for p in &first.lock.packages {
        assert!(e.store().contains(&p.hash));
    }
}

#[test]
fn path_entries_are_pinned_and_mutable() {
    let e = env();
    Pkg::new("t.hotbar", "1.0.0").write_into(&e.repo);
    Pkg::new("t.caves", "2.0.0").write_into(&e.repo); // a newer published version than the dev tree
    let dev = Pkg::new("t.caves", "1.5.0")
        .deps(&[("t.hotbar", "*")])
        .write(&e.root.join("dev/caves"));
    let mut inst = e.instance("dev", &[]);
    inst.manifest.mods.insert(
        pid("t.caves"),
        ModRequirement {
            version: req("*"),
            source: ModSource::Path(PathBuf::from("../../../../dev/caves")),
        },
    );
    inst.save().unwrap();
    // Recorded canonicalised.
    let resolved_dev = std::fs::canonicalize(&dev).unwrap();
    assert_eq!(
        inst.source_path(Path::new("../../../../dev/caves")),
        resolved_dev
    );

    let first = e.lock(&inst);
    assert_eq!(locked(&first.lock), ["t.caves 1.5.0", "t.hotbar 1.0.0"]);
    let caves = first.lock.package(&pid("t.caves")).unwrap();
    assert_eq!(caves.source, PackageSource::Path(resolved_dev.clone()));
    assert!(e.store().contains(&caves.hash));
    assert_eq!(inst.stale_reason(&first.lock, &e.pwc).unwrap(), None);

    // Editing the tree makes the lock stale; re-locking picks up the new contents at the same
    // version without complaint (path trees are mutable).
    std::fs::write(
        dev.join("src/lib.rs"),
        "pub fn register() { /* edited */ }\n",
    )
    .unwrap();
    let reason = inst.stale_reason(&first.lock, &e.pwc).unwrap().unwrap();
    assert!(reason.contains("changed"), "{reason}");
    let second = e.lock(&inst);
    let caves2 = second.lock.package(&pid("t.caves")).unwrap();
    assert_ne!(caves2.hash, caves.hash);
    assert_eq!(
        second.changes,
        [change("t.caves", Some("1.5.0"), Some("1.5.0"))]
    );
    assert_ne!(second.lock.environment, first.lock.environment);

    // Switching the entry to the registry: a published version replaces the dev snapshot.
    inst.manifest.mods.insert(
        pid("t.caves"),
        ModRequirement {
            version: req("*"),
            source: ModSource::Registry,
        },
    );
    inst.save().unwrap();
    let reason = inst.stale_reason(&second.lock, &e.pwc).unwrap().unwrap();
    assert!(reason.contains("new source"), "{reason}");
    let third = e
        .lock_with(
            &inst,
            &LockOptions {
                update: [pid("t.caves")].into(),
                update_all: false,
            },
        )
        .unwrap();
    assert_eq!(locked(&third.lock), ["t.caves 2.0.0"]);
}

#[test]
fn path_and_file_entries_are_checked() {
    let e = env();
    let dev = Pkg::new("t.caves", "1.5.0").write(&e.root.join("dev/caves"));
    let mut inst = e.instance("dev", &[]);

    // Wrong id.
    inst.manifest.mods.insert(
        pid("t.other"),
        ModRequirement {
            version: req("*"),
            source: ModSource::Path(dev.clone()),
        },
    );
    let err = e
        .lock_with(&inst, &LockOptions::default())
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("t.caves") && err.contains("mods.t.other"),
        "{err}"
    );

    // Asserted version.
    inst.manifest.mods.clear();
    inst.manifest.mods.insert(
        pid("t.caves"),
        ModRequirement {
            version: req("^2"),
            source: ModSource::Path(dev.clone()),
        },
    );
    let err = e
        .lock_with(&inst, &LockOptions::default())
        .unwrap_err()
        .to_string();
    assert!(err.contains("version 1.5.0"), "{err}");

    // Missing tree.
    inst.manifest.mods.insert(
        pid("t.caves"),
        ModRequirement {
            version: req("*"),
            source: ModSource::Path(e.root.join("nowhere")),
        },
    );
    assert!(e.lock_with(&inst, &LockOptions::default()).is_err());
    assert!(
        inst.lockfile().unwrap().is_none(),
        "nothing written on failure"
    );
}

#[test]
fn file_entries() {
    let e = env();
    let (pkg, _) =
        Package::from_dir(&Pkg::new("t.seasons", "2.3.1").write(&e.root.join("src/seasons")))
            .unwrap();
    let downloads = e.root.join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let file = pkg.write_archive(&downloads).unwrap();
    let mut inst = e.instance("files", &[]);
    inst.manifest.mods.insert(
        pid("t.seasons"),
        ModRequirement {
            version: req("^2"),
            source: ModSource::File(file.clone()),
        },
    );
    inst.save().unwrap();
    let report = e.lock(&inst);
    let seasons = report.lock.package(&pid("t.seasons")).unwrap();
    assert_eq!(
        seasons.source,
        PackageSource::File(std::fs::canonicalize(&file).unwrap())
    );
    assert_eq!(seasons.hash, *pkg.hash());
    assert!(e.store().verify(pkg.hash()).unwrap());
    assert_eq!(inst.stale_reason(&report.lock, &e.pwc).unwrap(), None);
}

#[test]
fn published_versions_are_immutable() {
    let e = env();
    let hotbar = Pkg::new("t.hotbar", "1.0.0").write_into(&e.repo);
    let inst = e.instance("survival", &[("t.hotbar", "*")]);
    let first = e.lock(&inst);

    // The repository's 1.0.0 changes without a version bump.
    std::fs::write(
        hotbar.join("src/lib.rs"),
        "pub fn register() { /* sneaky */ }\n",
    )
    .unwrap();
    match e.lock_with(&inst, &LockOptions::default()) {
        Err(InstanceError::ContentsChanged {
            id,
            version,
            locked,
            found,
        }) => {
            assert_eq!(id, pid("t.hotbar"));
            assert_eq!(version.to_string(), "1.0.0");
            assert_eq!(locked, first.lock.packages[0].hash);
            assert_ne!(found, locked);
        }
        other => panic!("{other:?}"),
    }
    let msg = e
        .lock_with(&inst, &LockOptions::default())
        .unwrap_err()
        .to_string();
    assert!(
        msg.contains("changed contents without a version bump") && msg.contains("bump the version"),
        "{msg}"
    );
    assert_eq!(
        inst.lockfile().unwrap().unwrap(),
        first.lock,
        "the old lock is kept"
    );

    // Bumping the version resolves it: the lock keeps the installed 1.0.0 (from the store)
    // until an update moves to 1.0.1.
    Pkg::new("t.hotbar", "1.0.1")
        .code("pub fn register() { /* sneaky */ }\n")
        .write_into(&e.repo);
    std::fs::remove_dir_all(&hotbar).unwrap();
    let report = e.lock(&inst);
    assert_eq!(locked(&report.lock), ["t.hotbar 1.0.0"]);
    assert_eq!(report.lock.packages[0].hash, first.lock.packages[0].hash);
    assert_eq!(report.lock.packages[0].source, PackageSource::Store);
    let report = e
        .lock_with(
            &inst,
            &LockOptions {
                update_all: true,
                ..LockOptions::default()
            },
        )
        .unwrap();
    assert_eq!(locked(&report.lock), ["t.hotbar 1.0.1"]);
}

#[test]
fn the_store_is_a_source_too() {
    let mut e = env();
    Pkg::new("t.hotbar", "1.0.0").write_into(&e.repo);
    let inst = e.instance("survival", &[("t.hotbar", "*")]);
    let first = e.lock(&inst);

    // Without the repository, the installed package still resolves.
    e.config.repositories.clear();
    let second = e.lock(&inst);
    assert_eq!(locked(&second.lock), locked(&first.lock));
    assert_eq!(second.lock.packages[0].hash, first.lock.packages[0].hash);
    assert_eq!(second.lock.packages[0].source, PackageSource::Store);
    assert!(second.changes.is_empty());

    // A new instance can use it as well.
    let other = e.instance("other", &[("t.hotbar", "^1")]);
    assert_eq!(e.lock(&other).lock.packages[0].source, PackageSource::Store);
}

#[test]
fn bundles_pull_in_their_dependencies() {
    let e = env();
    Pkg::new("t.hotbar", "1.0.0").write_into(&e.repo);
    Pkg::new("t.inventory", "1.0.0")
        .deps(&[("t.hotbar", "^1")])
        .write_into(&e.repo);
    Pkg::new("t.essentials", "1.0.0")
        .bundle()
        .deps(&[("t.inventory", "^1"), ("t.hotbar", "^1")])
        .write_into(&e.repo);
    // A mod depending on the bundle.
    Pkg::new("t.extra", "1.0.0")
        .deps(&[("t.essentials", "^1")])
        .write_into(&e.repo);
    let inst = e.instance("survival", &[("t.extra", "*")]);
    let report = e.lock(&inst);
    assert_eq!(
        locked(&report.lock),
        [
            "t.essentials 1.0.0",
            "t.extra 1.0.0",
            "t.hotbar 1.0.0",
            "t.inventory 1.0.0"
        ]
    );
    let bundle = report.lock.package(&pid("t.essentials")).unwrap();
    assert_eq!(bundle.kind, ModKind::Bundle);
    assert_eq!(bundle.dependencies, [pid("t.hotbar"), pid("t.inventory")]);
}

#[test]
fn lock_errors() {
    let e = env();
    Pkg::new("t.hotbar", "1.0.0").write_into(&e.repo);

    // The instance wants another PWC.
    let mut m = e.manifest("future", &[("t.hotbar", "*")]);
    m.pwc = req("^3");
    let inst = Instance::create(&e.dirs, m).unwrap();
    let err = e
        .lock_with(&inst, &LockOptions::default())
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("requires PWC `^3`") && err.contains("2.1.0"),
        "{err}"
    );

    // Resolution failures come through.
    let inst = e.instance("unknown", &[("t.nothing", "*")]);
    match e.lock_with(&inst, &LockOptions::default()) {
        Err(InstanceError::Resolve(pwc_resolver::ResolveError::Unknown { id, .. })) => {
            assert_eq!(id, pid("t.nothing"))
        }
        other => panic!("{other:?}"),
    }
    let inst = e.instance("toonew", &[("t.hotbar", "^2")]);
    let err = e
        .lock_with(&inst, &LockOptions::default())
        .unwrap_err()
        .to_string();
    assert!(err.contains("available: 1.0.0"), "{err}");

    // A corrupt existing lock is replaced, with a warning.
    let inst = e.instance("corrupt", &[("t.hotbar", "*")]);
    std::fs::write(inst.lock_path(), "garbage").unwrap();
    let report = e.lock(&inst);
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("ignoring the existing pwc.lock")),
        "{:?}",
        report.warnings
    );
    assert_eq!(inst.lockfile().unwrap().unwrap(), report.lock);
}

#[test]
fn an_empty_instance_locks_to_nothing() {
    let e = env();
    let inst = e.instance("vanilla", &[]);
    let report = e.lock(&inst);
    assert!(report.lock.packages.is_empty());
    assert!(report.changes.is_empty());
    assert!(inst.lock_path().is_file());
    assert_eq!(inst.stale_reason(&report.lock, &e.pwc).unwrap(), None);
}

#[test]
fn stale_when_the_pwc_source_changes() {
    let e = env();
    let inst = e.instance("vanilla", &[]);
    let lock = e.lock(&inst).lock;
    let mut moved = e.pwc.clone();
    moved.revision = "abc".into();
    assert!(
        inst.stale_reason(&lock, &moved)
            .unwrap()
            .unwrap()
            .contains("moved")
    );
    let mut newer = e.pwc.clone();
    newer.version = Version::new(2, 2, 0);
    assert!(
        inst.stale_reason(&lock, &newer)
            .unwrap()
            .unwrap()
            .contains("changed")
    );

    // A new requirement.
    let mut inst = inst;
    inst.manifest.mods.insert(
        pid("t.x"),
        ModRequirement {
            version: req("*"),
            source: ModSource::Registry,
        },
    );
    assert!(
        inst.stale_reason(&lock, &e.pwc)
            .unwrap()
            .unwrap()
            .contains("not locked")
    );
}

#[test]
fn gc_keeps_what_instances_reference() {
    let e = env();
    Pkg::new("t.hotbar", "1.0.0").write_into(&e.repo);
    Pkg::new("t.caves", "1.0.0").write_into(&e.repo);
    Pkg::new("t.seasons", "1.0.0").write_into(&e.repo);
    let a = e.instance("a", &[("t.hotbar", "*")]);
    let b = e.instance("b", &[("t.caves", "*")]);
    let lock_a = e.lock(&a).lock;
    let lock_b = e.lock(&b).lock;
    let c = e.instance("c", &[("t.seasons", "*")]);
    let lock_c = e.lock(&c).lock;
    c.remove().unwrap(); // its package becomes garbage

    let keep = referenced_hashes(&e.dirs).unwrap();
    let expected: BTreeSet<PackageHash> = [&lock_a, &lock_b]
        .iter()
        .flat_map(|l| &l.packages)
        .map(|p| p.hash.clone())
        .collect();
    assert_eq!(keep, expected);

    let store = e.store();
    let removed = store.gc(&keep, false).unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].hash, lock_c.packages[0].hash);
    for h in &keep {
        assert!(store.verify(h).unwrap());
    }
    assert!(installed_packages(&store, &lock_a).is_ok());
    match installed_packages(&store, &lock_c) {
        Err(InstanceError::NotInstalled { id, .. }) => assert_eq!(id, pid("t.seasons")),
        other => panic!("{other:?}"),
    }

    // A broken lock stops gc from guessing.
    std::fs::write(a.lock_path(), "garbage").unwrap();
    assert!(referenced_hashes(&e.dirs).is_err());
}

#[test]
fn referenced_hashes_without_instances() {
    let e = env();
    assert!(referenced_hashes(&e.dirs).unwrap().is_empty());
}

#[test]
fn relock_reinstalls_a_collected_package() {
    let e = env();
    Pkg::new("t.hotbar", "1.0.0").write_into(&e.repo);
    let inst = e.instance("a", &[("t.hotbar", "*")]);
    let lock = e.lock(&inst).lock;
    let store = e.store();
    store.gc(&BTreeSet::new(), false).unwrap();
    assert!(!store.contains(&lock.packages[0].hash));
    let again = e.lock(&inst).lock;
    assert_eq!(again, lock);
    assert!(store.verify(&lock.packages[0].hash).unwrap());
}
