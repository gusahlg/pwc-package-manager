//! Store tests on temporary directories.

use std::collections::BTreeSet;
use std::path::Path;

use pwc_package::{Package, PackageFile};

use super::*;

fn manifest(id: &str, version: &str) -> String {
    format!(
        r#"format = 1

[package]
id = "{id}"
name = "Test package"
version = "{version}"
description = "A package used by the store tests."
authors = ["Tester"]
license = "AGPL-3.0-or-later"
license-files = ["LICENSE"]
pwc-api = "^1.0"
"#
    )
}

fn package(id: &str, version: &str, code: &str) -> Package {
    let file = |path: &str, text: &str| PackageFile {
        path: path.to_string(),
        bytes: text.as_bytes().to_vec(),
    };
    Package::from_files(vec![
        file("LICENSE", "GNU AFFERO GENERAL PUBLIC LICENSE\n"),
        file("README.md", "# Test\n"),
        file("mod.toml", &manifest(id, version)),
        file("src/lib.rs", code),
        file("src/util/mod.rs", "pub fn x() {}\n"),
    ])
    .unwrap()
}

/// A temporary directory holding a store. Entries are read-only, so dropping it first gives write
/// permission back; otherwise the directory could not be deleted and would leak.
struct Tmp(tempfile::TempDir);

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = make_writable(self.0.path());
    }
}

fn store() -> (Tmp, Store) {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path().join("data/store")).unwrap();
    (Tmp(tmp), store)
}

/// Names in the store root.
fn root_names(store: &Store) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(store.root())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[cfg(unix)]
fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// Make an entry writable again so a test can tamper with it.
#[cfg(unix)]
fn unfreeze(dir: &Path) {
    make_writable(dir).unwrap();
}

/// Set a directory's modification time `secs` seconds into the past.
fn age(dir: &Path, secs: u64) {
    let past = std::time::SystemTime::now() - std::time::Duration::from_secs(secs);
    std::fs::File::open(dir)
        .unwrap()
        .set_modified(past)
        .unwrap();
}

#[test]
fn open_creates_the_root() {
    let (_tmp, store) = store();
    assert!(store.root().is_dir());
    assert!(store.list().unwrap().is_empty());
    // Opening again is fine.
    Store::open(store.root()).unwrap();
}

#[test]
fn insert_unpacks_read_only_under_the_hash() {
    let (_tmp, store) = store();
    let pkg = package("test.alpha", "1.0.0", "pub fn register() {}\n");
    assert!(!store.contains(pkg.hash()));
    let entry = store.insert(&pkg).unwrap();
    assert_eq!(&entry.hash, pkg.hash());
    assert_eq!(entry.dir, store.root().join(pkg.hash().hex()));
    assert_eq!(entry.manifest.id().as_str(), "test.alpha");
    assert!(store.contains(pkg.hash()));
    assert_eq!(
        std::fs::read_to_string(entry.dir.join("src/lib.rs")).unwrap(),
        "pub fn register() {}\n"
    );
    assert_eq!(
        root_names(&store),
        [pkg.hash().hex().to_string()],
        "no temporaries left behind"
    );
    #[cfg(unix)]
    {
        assert_eq!(mode(&entry.dir), 0o555);
        assert_eq!(mode(&entry.dir.join("src")), 0o555);
        assert_eq!(mode(&entry.dir.join("src/util")), 0o555);
        assert_eq!(mode(&entry.dir.join("mod.toml")), 0o444);
        assert_eq!(mode(&entry.dir.join("src/util/mod.rs")), 0o444);
    }
    // The unpacked entry is exactly the package.
    let (again, ignored) = Package::from_dir(&entry.dir).unwrap();
    assert_eq!(again.hash(), pkg.hash());
    assert!(ignored.is_empty());
    assert!(store.verify(pkg.hash()).unwrap());
}

#[test]
fn insert_is_idempotent() {
    let (_tmp, store) = store();
    let pkg = package("test.alpha", "1.0.0", "");
    let first = store.insert(&pkg).unwrap();
    let second = store.insert(&pkg).unwrap();
    assert_eq!(first.dir, second.dir);
    assert_eq!(root_names(&store).len(), 1);
}

#[test]
fn concurrent_inserts_of_one_package_agree() {
    let (_tmp, store) = store();
    let pkg = package("test.alpha", "1.0.0", "// racing\n");
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..8)
            .map(|_| s.spawn(|| store.insert(&pkg).unwrap()))
            .collect();
        for h in handles {
            assert_eq!(h.join().unwrap().hash, *pkg.hash());
        }
    });
    assert_eq!(root_names(&store), [pkg.hash().hex().to_string()]);
    assert!(store.verify(pkg.hash()).unwrap());
}

#[test]
fn get_and_list() {
    let (_tmp, store) = store();
    let b = package("test.beta", "1.0.0", "");
    let a2 = package("test.alpha", "2.0.0", "");
    let a1 = package("test.alpha", "1.0.0", "");
    let a10 = package("test.alpha", "10.0.0", "");
    for p in [&b, &a2, &a1, &a10] {
        store.insert(p).unwrap();
    }
    let listed: Vec<String> = store
        .list()
        .unwrap()
        .iter()
        .map(|e| format!("{} {}", e.manifest.id(), e.manifest.version()))
        .collect();
    assert_eq!(
        listed,
        [
            "test.alpha 1.0.0",
            "test.alpha 2.0.0",
            "test.alpha 10.0.0",
            "test.beta 1.0.0"
        ]
    );

    let got = store.get(b.hash()).unwrap().unwrap();
    assert_eq!(got.manifest.id().as_str(), "test.beta");
    let missing = PackageHash::from_digest([7; 32]);
    assert!(store.get(&missing).unwrap().is_none());
    assert!(!store.contains(&missing));
    assert_eq!(store.hashes().unwrap().len(), 4);
}

#[test]
fn junk_in_the_root_is_not_an_entry() {
    let (_tmp, store) = store();
    let pkg = package("test.alpha", "1.0.0", "");
    store.insert(&pkg).unwrap();
    std::fs::create_dir(store.root().join("not-a-hash")).unwrap();
    std::fs::write(store.root().join("a".repeat(64)), "a file, not a directory").unwrap();
    std::fs::create_dir(store.root().join("A".repeat(64))).unwrap(); // uppercase: not a hash
    assert_eq!(store.hashes().unwrap(), [pkg.hash().clone()]);
    assert_eq!(store.list().unwrap().len(), 1);
}

#[test]
fn unreadable_entries() {
    let (_tmp, store) = store();
    let pkg = package("test.alpha", "1.0.0", "");
    store.insert(&pkg).unwrap();
    let broken = PackageHash::from_digest([1; 32]);
    std::fs::create_dir(store.dir_of(&broken)).unwrap();
    std::fs::write(store.dir_of(&broken).join("mod.toml"), "this is not toml [").unwrap();

    assert!(store.list().is_err());
    let (entries, bad) = store.list_lenient().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].0, broken);
    assert!(!store.verify(&broken).unwrap());
}

#[cfg(unix)]
#[test]
fn verify_detects_tampering() {
    let (_tmp, store) = store();
    let pkg = package("test.alpha", "1.0.0", "pub fn a() {}\n");
    let entry = store.insert(&pkg).unwrap();
    assert!(store.verify(pkg.hash()).unwrap());
    assert!(store.verify_all().unwrap().is_empty());

    unfreeze(&entry.dir);
    std::fs::write(entry.dir.join("src/lib.rs"), "pub fn evil() {}\n").unwrap();
    assert!(!store.verify(pkg.hash()).unwrap());
    assert_eq!(store.verify_all().unwrap(), [pkg.hash().clone()]);

    // Restored contents verify again; an extra file (even one packaging ignores) does not.
    std::fs::write(entry.dir.join("src/lib.rs"), "pub fn a() {}\n").unwrap();
    assert!(store.verify(pkg.hash()).unwrap());
    std::fs::write(entry.dir.join("notes.txt"), "extra").unwrap();
    assert!(!store.verify(pkg.hash()).unwrap());
    std::fs::remove_file(entry.dir.join("notes.txt")).unwrap();

    // A missing required file makes it an invalid package: corrupt, not an error.
    std::fs::remove_file(entry.dir.join("README.md")).unwrap();
    assert!(!store.verify(pkg.hash()).unwrap());
}

#[test]
fn verify_of_a_missing_entry_is_an_error() {
    let (_tmp, store) = store();
    match store.verify(&PackageHash::from_digest([3; 32])) {
        Err(StoreError::Io { source, .. }) => {
            assert_eq!(source.kind(), std::io::ErrorKind::NotFound)
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn gc_removes_unreferenced_entries() {
    let (_tmp, store) = store();
    let keep_me = package("test.alpha", "1.0.0", "");
    let drop_me = package("test.beta", "1.0.0", "");
    let drop_too = package("test.alpha", "0.9.0", "");
    for p in [&keep_me, &drop_me, &drop_too] {
        store.insert(p).unwrap();
    }
    let broken = PackageHash::from_digest([9; 32]);
    std::fs::create_dir(store.dir_of(&broken)).unwrap();
    let stale = store.root().join(".tmp-0123456789abcdef");
    std::fs::create_dir_all(stale.join("package/src")).unwrap();
    std::fs::write(stale.join("package/src/lib.rs"), "").unwrap();
    #[cfg(unix)]
    set_read_only_below(&stale).unwrap();
    age(&stale, 2 * 3600);
    // A young temporary may belong to a running insertion: kept.
    let young = store.root().join(".tmp-fedcba9876543210");
    std::fs::create_dir_all(&young).unwrap();

    let keep: BTreeSet<PackageHash> = [keep_me.hash().clone()].into();
    let dry = store.gc_report(&keep, true).unwrap();
    let names = |r: &GcReport| -> Vec<String> {
        r.removed
            .iter()
            .map(|e| format!("{} {}", e.manifest.id(), e.manifest.version()))
            .collect()
    };
    assert_eq!(names(&dry), ["test.alpha 0.9.0", "test.beta 1.0.0"]);
    assert_eq!(dry.removed_unreadable, std::slice::from_ref(&broken));
    assert_eq!(dry.temporaries, std::slice::from_ref(&stale));
    assert_eq!(store.hashes().unwrap().len(), 4, "dry run removes nothing");
    assert!(stale.exists());

    let removed = store.gc(&keep, false).unwrap();
    assert_eq!(removed.len(), 2);
    assert_eq!(store.hashes().unwrap(), [keep_me.hash().clone()]);
    assert_eq!(
        root_names(&store),
        [
            ".tmp-fedcba9876543210".to_string(),
            keep_me.hash().hex().to_string()
        ]
    );
    assert!(!stale.exists());
    assert!(store.verify(keep_me.hash()).unwrap());

    // Nothing left to collect.
    let again = store.gc_report(&keep, false).unwrap();
    assert!(
        again.removed.is_empty()
            && again.removed_unreadable.is_empty()
            && again.temporaries.is_empty()
    );
}

#[test]
fn remove_deletes_read_only_entries() {
    let (_tmp, store) = store();
    let pkg = package("test.alpha", "1.0.0", "");
    store.insert(&pkg).unwrap();
    assert!(store.remove(pkg.hash()).unwrap());
    assert!(!store.contains(pkg.hash()));
    assert!(root_names(&store).is_empty());
    assert!(!store.remove(pkg.hash()).unwrap());
    // And it can be inserted again.
    store.insert(&pkg).unwrap();
    assert!(store.verify(pkg.hash()).unwrap());
}
