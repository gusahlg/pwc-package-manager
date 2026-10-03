//! Package tests: collection from directories, the contents rules, archives and verification.

use std::fs;
use std::path::Path;

use pwc_manifest::{LicenseError, ModKind};
use tempfile::TempDir;

use super::*;

const MOD_TOML: &str = r#"format = 1

[package]
id = "example.thing"
name = "Thing"
version = "1.2.3"
description = "A thing for tests."
authors = ["Tester <tester@example.org>"]
license = "AGPL-3.0-or-later AND CC-BY-SA-4.0"
license-files = ["LICENSES/AGPL-3.0-or-later.txt", "LICENSES/CC-BY-SA-4.0.txt"]
pwc-api = "^1.0"
icon = "assets/icon.svg"
"#;

const BUNDLE_TOML: &str = r#"format = 1

[package]
id = "example.bundle"
name = "Bundle"
version = "1.0.0"
description = "A bundle for tests."
authors = ["Tester"]
license = "CC0-1.0"
license-files = ["LICENSE"]
kind = "bundle"

[dependencies]
"example.thing" = "^1.2"
"#;

fn write(root: &Path, path: &str, bytes: &[u8]) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, bytes).unwrap();
}

fn file(path: &str, bytes: &[u8]) -> PackageFile {
    PackageFile {
        path: path.to_string(),
        bytes: bytes.to_vec(),
    }
}

/// The package files of the sample mod.
fn sample_files() -> Vec<PackageFile> {
    vec![
        file("CHANGELOG.md", b"# 1.2.3\n"),
        file("LICENSES/AGPL-3.0-or-later.txt", b"AGPL text\n"),
        file("LICENSES/CC-BY-SA-4.0.txt", b"CC-BY-SA text\n"),
        file("NOTICE", b"Thing (c) Tester\n"),
        file("README.md", b"# Thing\nPress T.\n"),
        file(
            "assets/icon.svg",
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        ),
        file(
            "assets/sounds/click.ogg",
            &[0x4f, 0x67, 0x67, 0x53, 0, 1, 2, 255],
        ),
        file("data/table.ron", b"(a: 1)\n"),
        file("mod.toml", MOD_TOML.as_bytes()),
        file(
            "src/lib.rs",
            b"mod ui;\npub fn register(_r: &mut pwc_mod_api::ModRegistrar) {}\n",
        ),
        file("src/ui/mod.rs", b"// ui\n"),
    ]
}

/// A source directory with the sample mod plus everything a development tree has around it.
fn sample_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    for f in sample_files() {
        write(dir.path(), &f.path, &f.bytes);
    }
    // Not part of the package: ignored, not errors.
    write(dir.path(), "Cargo.toml", b"[package]\nname = \"dev\"\n");
    write(dir.path(), "Cargo.lock", b"# lock\n");
    write(dir.path(), "build.rs", b"fn main() {}\n");
    write(dir.path(), "target/debug/thing", b"\x7fELF");
    write(dir.path(), "dist/example.thing-1.2.2.pwcmod", b"old");
    write(dir.path(), ".git/HEAD", b"ref: refs/heads/main\n");
    write(dir.path(), ".gitignore", b"target/\n");
    write(dir.path(), "notes.txt", b"todo\n");
    write(dir.path(), "docs/guide.md", b"# Guide\n");
    write(
        dir.path(),
        "LICENSES/README",
        b"which licence covers what\n",
    );
    write(dir.path(), "src.bak/lib.rs", b"old\n");
    dir
}

const SAMPLE_IGNORED: [&str; 11] = [
    ".git/",
    ".gitignore",
    "Cargo.lock",
    "Cargo.toml",
    "LICENSES/README",
    "build.rs",
    "dist/",
    "docs/",
    "notes.txt",
    "src.bak/",
    "target/",
];

fn assert_forbidden(result: Result<impl std::fmt::Debug, PackageError>, expected_path: &str) {
    match result {
        Err(PackageError::Forbidden { path, reason }) => {
            assert_eq!(path, expected_path, "reason: {reason}");
            assert!(!reason.is_empty());
        }
        other => panic!("expected `{expected_path}` to be forbidden, got {other:?}"),
    }
}

fn assert_missing(result: Result<impl std::fmt::Debug, PackageError>, expected: &str) {
    match result {
        Err(PackageError::MissingFile(path)) => assert_eq!(path, expected),
        other => panic!("expected missing `{expected}`, got {other:?}"),
    }
}

/// The canonical tar of arbitrary entries (any order, any paths), compressed.
fn archive_of(entries: &[PackageFile]) -> Vec<u8> {
    zstd::compress(&ustar::to_vec(entries))
}

// ---------------------------------------------------------------------------------------------
// Collecting a source directory

#[test]
fn from_dir_collects_exactly_the_package() {
    let dir = sample_dir();
    let (package, ignored) = Package::from_dir(dir.path()).unwrap();
    assert_eq!(package.files(), sample_files().as_slice());
    assert_eq!(ignored, SAMPLE_IGNORED);
    assert_eq!(package.manifest().id().as_str(), "example.thing");
    assert_eq!(package.file_name(), "example.thing-1.2.3.pwcmod");
}

#[test]
fn from_dir_matches_from_files() {
    let dir = sample_dir();
    let (from_dir, _) = Package::from_dir(dir.path()).unwrap();
    let mut shuffled = sample_files();
    shuffled.reverse();
    let from_files = Package::from_files(shuffled).unwrap();
    assert_eq!(from_dir.hash(), from_files.hash());
    assert_eq!(
        from_dir.files(),
        from_files.files(),
        "from_files sorts by path"
    );
}

#[test]
fn packaging_is_deterministic() {
    let a = sample_dir();
    let (first, _) = Package::from_dir(a.path()).unwrap();
    let (second, _) = Package::from_dir(a.path()).unwrap();
    assert_eq!(first.hash(), second.hash());
    assert_eq!(first.canonical_tar(), second.canonical_tar());
    assert_eq!(first.to_archive_bytes(), second.to_archive_bytes());

    // Another copy, created later, with other permissions and without the noise: same hash.
    let b = TempDir::new().unwrap();
    for f in sample_files().iter().rev() {
        write(b.path(), &f.path, &f.bytes);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            b.path().join("src/lib.rs"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    let (third, ignored) = Package::from_dir(b.path()).unwrap();
    assert!(ignored.is_empty());
    assert_eq!(third.hash(), first.hash());
    assert_eq!(third.to_archive_bytes(), first.to_archive_bytes());
}

#[test]
fn hash_is_sha256_of_the_canonical_tar() {
    let package = Package::from_files(sample_files()).unwrap();
    let tar = package.canonical_tar();
    let digest: [u8; 32] = Sha256::digest(&tar).into();
    assert_eq!(package.hash(), &PackageHash::from_digest(digest));
    assert_eq!(tar.len(), ustar::len(package.files()));
    assert!(package.hash().as_str().starts_with("sha256:"));
}

#[test]
fn golden_hash() {
    // Computed independently by a Python implementation of docs/spec/package-format.md (header
    // layout, PAX record for the 140-byte path, padding, end blocks). Pinned: any change to the
    // canonical tar changes every package hash in every lockfile.
    let manifest = r#"format = 1

[package]
id = "example.golden"
name = "Golden"
version = "1.0.0"
description = "A fixed package whose hash is pinned."
authors = ["Example <example@example.org>"]
license = "AGPL-3.0-or-later"
license-files = ["LICENSE"]
pwc-api = "^1.0"
"#;
    let long = format!("assets/{}icon.svg", "long/".repeat(25));
    assert_eq!(long.len(), 140);
    let package = Package::from_files(vec![
        file("mod.toml", manifest.as_bytes()),
        file("README.md", b"# Golden\n"),
        file("LICENSE", b"AGPL-3.0-or-later text\n"),
        file(
            "src/lib.rs",
            b"pub fn register(_: &mut pwc_mod_api::ModRegistrar) {}\n",
        ),
        file(&long, b"<svg/>"),
    ])
    .unwrap();
    assert_eq!(package.canonical_tar().len(), 7168);
    assert_eq!(
        package.hash().as_str(),
        "sha256:c61d7c62fc461cb94f97629c873af3b323303de5b25774d0933eee640d94d53c"
    );
}

#[test]
fn named_files_in_subdirectories() {
    let dir = TempDir::new().unwrap();
    let manifest = MOD_TOML.replace(
        "icon = \"assets/icon.svg\"\n",
        "readme = \"docs/README.md\"\n",
    );
    write(dir.path(), "mod.toml", manifest.as_bytes());
    write(dir.path(), "docs/README.md", b"# Thing\n");
    write(dir.path(), "docs/extra.md", b"not packaged\n");
    write(
        dir.path(),
        "README.md",
        b"a root readme the manifest does not name\n",
    );
    write(dir.path(), "LICENSES/AGPL-3.0-or-later.txt", b"AGPL\n");
    write(dir.path(), "LICENSES/CC-BY-SA-4.0.txt", b"CC\n");
    write(dir.path(), "src/lib.rs", b"");
    let (package, ignored) = Package::from_dir(dir.path()).unwrap();
    let paths: Vec<&str> = package.files().iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "LICENSES/AGPL-3.0-or-later.txt",
            "LICENSES/CC-BY-SA-4.0.txt",
            "docs/README.md",
            "mod.toml",
            "src/lib.rs"
        ]
    );
    assert_eq!(ignored, ["README.md", "docs/extra.md"]);
}

#[test]
fn forbidden_files_inside_the_trees() {
    for (path, bytes) in [
        ("src/build.rs", &b"fn main() {}"[..]),
        ("src/nested/Cargo.toml", b"[package]"),
        ("assets/Cargo.lock", b""),
        ("assets/.hidden", b""),
        ("data/sub/.DS_Store", b""),
        ("src/.git/config", b""),
        ("assets/build.rs/inner.txt", b""),
    ] {
        let dir = sample_dir();
        write(dir.path(), path, bytes);
        let result = Package::from_dir(dir.path());
        // The error names the first forbidden component's path as found by the walk.
        match result {
            Err(PackageError::Forbidden { path: found, .. }) => {
                assert!(path.starts_with(&found), "{path}: {found}")
            }
            other => panic!("{path}: expected forbidden, got {other:?}"),
        }
    }
}

#[cfg(unix)]
#[test]
fn symlinks_are_errors_and_never_followed() {
    use std::os::unix::fs::symlink;
    let outside = TempDir::new().unwrap();
    write(outside.path(), "secret.txt", b"do not package me");

    let dir = sample_dir();
    symlink(
        outside.path().join("secret.txt"),
        dir.path().join("src/secret.rs"),
    )
    .unwrap();
    assert_forbidden(Package::from_dir(dir.path()), "src/secret.rs");

    let dir = sample_dir();
    symlink(outside.path(), dir.path().join("assets/linked")).unwrap();
    assert_forbidden(Package::from_dir(dir.path()), "assets/linked");

    let dir = sample_dir();
    fs::remove_dir_all(dir.path().join("data")).unwrap();
    symlink(outside.path(), dir.path().join("data")).unwrap();
    assert_forbidden(Package::from_dir(dir.path()), "data");

    let dir = sample_dir();
    fs::rename(dir.path().join("README.md"), dir.path().join("README.real")).unwrap();
    symlink(dir.path().join("README.real"), dir.path().join("README.md")).unwrap();
    assert_forbidden(Package::from_dir(dir.path()), "README.md");

    let dir = sample_dir();
    fs::rename(dir.path().join("mod.toml"), dir.path().join("mod.real")).unwrap();
    symlink(dir.path().join("mod.real"), dir.path().join("mod.toml")).unwrap();
    assert_forbidden(Package::from_dir(dir.path()), "mod.toml");

    // A symlinked directory on the way to a named licence file.
    let dir = sample_dir();
    fs::rename(
        dir.path().join("LICENSES"),
        dir.path().join("licenses-real"),
    )
    .unwrap();
    symlink(
        dir.path().join("licenses-real"),
        dir.path().join("LICENSES"),
    )
    .unwrap();
    assert_forbidden(Package::from_dir(dir.path()), "LICENSES");

    // Symlinks outside the package contents are just ignored.
    let dir = sample_dir();
    symlink(outside.path(), dir.path().join("target-link")).unwrap();
    let (_, ignored) = Package::from_dir(dir.path()).unwrap();
    assert!(ignored.contains(&"target-link".to_string()));
}

#[cfg(unix)]
#[test]
fn special_files_are_errors() {
    let dir = sample_dir();
    let socket = dir.path().join("data/control.sock");
    // Socket paths are limited to ~107 bytes; a deeply nested temporary directory cannot hold one.
    let Ok(_listener) = std::os::unix::net::UnixListener::bind(&socket) else {
        eprintln!("skipped: cannot create a socket at {}", socket.display());
        return;
    };
    assert_forbidden(Package::from_dir(dir.path()), "data/control.sock");
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_names() {
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::OsStr::from_bytes(b"bad\xffname.rs");
    let dir = sample_dir();
    fs::write(dir.path().join("src").join(name), b"").unwrap();
    assert!(matches!(
        Package::from_dir(dir.path()),
        Err(PackageError::Forbidden { .. })
    ));

    let dir = sample_dir();
    fs::write(dir.path().join(name), b"").unwrap();
    let (_, ignored) = Package::from_dir(dir.path()).unwrap();
    assert!(ignored.iter().any(|p| p.starts_with("bad")));
}

#[test]
fn tree_roots_must_be_directories() {
    let dir = sample_dir();
    fs::remove_dir_all(dir.path().join("data")).unwrap();
    write(dir.path(), "data", b"a file named data");
    assert_forbidden(Package::from_dir(dir.path()), "data");

    let dir = sample_dir();
    fs::remove_file(dir.path().join("NOTICE")).unwrap();
    fs::create_dir(dir.path().join("NOTICE")).unwrap();
    assert_forbidden(Package::from_dir(dir.path()), "NOTICE");
}

#[test]
fn hidden_named_files_are_forbidden() {
    let dir = sample_dir();
    let manifest = MOD_TOML.replace("icon = \"assets/icon.svg\"\n", "readme = \".README.md\"\n");
    write(dir.path(), "mod.toml", manifest.as_bytes());
    write(dir.path(), ".README.md", b"hidden");
    assert_forbidden(Package::from_dir(dir.path()), ".README.md");
}

#[test]
fn required_files() {
    let dir = TempDir::new().unwrap();
    assert_missing(Package::from_dir(dir.path()), "mod.toml");

    for (remove, missing) in [
        ("README.md", "README.md"),
        ("LICENSES/CC-BY-SA-4.0.txt", "LICENSES/CC-BY-SA-4.0.txt"),
        ("src/lib.rs", "src/lib.rs"),
        ("assets/icon.svg", "assets/icon.svg"),
    ] {
        let dir = sample_dir();
        fs::remove_file(dir.path().join(remove)).unwrap();
        assert_missing(Package::from_dir(dir.path()), missing);
        let files: Vec<PackageFile> = sample_files()
            .into_iter()
            .filter(|f| f.path != remove)
            .collect();
        assert_missing(Package::from_files(files), missing);
    }

    // CHANGELOG.md, NOTICE, data/ are optional.
    let files: Vec<PackageFile> = sample_files()
        .into_iter()
        .filter(|f| {
            !matches!(f.path.as_str(), "CHANGELOG.md" | "NOTICE") && !f.path.starts_with("data/")
        })
        .collect();
    assert!(Package::from_files(files).is_ok());

    // A library needs src/lib.rs too.
    let lib = MOD_TOML.replace("pwc-api", "kind = \"library\"\npwc-api");
    let files: Vec<PackageFile> = sample_files()
        .into_iter()
        .map(|f| {
            if f.path == "mod.toml" {
                file("mod.toml", lib.as_bytes())
            } else {
                f
            }
        })
        .filter(|f| f.path != "src/lib.rs")
        .collect();
    assert_missing(Package::from_files(files), "src/lib.rs");
}

#[test]
fn bundles_have_no_code() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "mod.toml", BUNDLE_TOML.as_bytes());
    write(dir.path(), "README.md", b"# Bundle\n");
    write(dir.path(), "LICENSE", b"CC0\n");
    write(dir.path(), "assets/banner.png", b"\x89PNG");
    let (package, _) = Package::from_dir(dir.path()).unwrap();
    assert_eq!(package.manifest().kind(), ModKind::Bundle);
    assert_eq!(package.files().len(), 4);

    write(dir.path(), "src/lib.rs", b"");
    assert_forbidden(Package::from_dir(dir.path()), "src");
    assert_forbidden(
        Package::from_files(
            package
                .files()
                .iter()
                .cloned()
                .chain([file("src/lib.rs", b"")])
                .collect(),
        ),
        "src/lib.rs",
    );
}

#[test]
fn manifest_errors_surface() {
    let dir = sample_dir();
    write(
        dir.path(),
        "mod.toml",
        MOD_TOML
            .replace(
                "AGPL-3.0-or-later AND CC-BY-SA-4.0",
                "LicenseRef-Proprietary",
            )
            .as_bytes(),
    );
    match Package::from_dir(dir.path()) {
        Err(PackageError::Manifest(ManifestError::License(LicenseError::NotAllowed(id)))) => {
            assert_eq!(id, "LicenseRef-Proprietary")
        }
        other => panic!("{other:?}"),
    }
    let dir = sample_dir();
    write(dir.path(), "mod.toml", b"format = 1\n[package]\n");
    assert!(matches!(
        Package::from_dir(dir.path()),
        Err(PackageError::Manifest(ManifestError::Missing("package.id")))
    ));
    let dir = sample_dir();
    write(dir.path(), "mod.toml", b"\xff\xfe");
    assert!(matches!(
        Package::from_dir(dir.path()),
        Err(PackageError::Manifest(ManifestError::Toml(_)))
    ));
}

#[test]
fn from_dir_needs_a_directory() {
    let dir = TempDir::new().unwrap();
    assert!(matches!(
        Package::from_dir(&dir.path().join("missing")),
        Err(PackageError::Io { .. })
    ));
    write(dir.path(), "file", b"");
    assert!(matches!(
        Package::from_dir(&dir.path().join("file")),
        Err(PackageError::Io { .. })
    ));
}

#[test]
fn size_limits() {
    // One file over 32 MiB (sparse: nothing is read).
    let dir = sample_dir();
    let big = fs::File::create(dir.path().join("assets/huge.bin")).unwrap();
    big.set_len(MAX_FILE_BYTES + 1).unwrap();
    assert!(
        matches!(Package::from_dir(dir.path()), Err(PackageError::TooLarge(msg)) if msg.contains("assets/huge.bin"))
    );

    // Exactly 32 MiB is fine as far as the per-file limit goes.
    assert!(contents::check_file_size("x", MAX_FILE_BYTES).is_ok());
    assert!(contents::check_total_size(MAX_TOTAL_BYTES).is_ok());
    assert!(contents::check_total_size(MAX_TOTAL_BYTES + 1).is_err());

    // More than 4096 files.
    let dir = sample_dir();
    for i in 0..MAX_FILES {
        write(dir.path(), &format!("data/many/{i:05}"), b"");
    }
    assert!(
        matches!(Package::from_dir(dir.path()), Err(PackageError::TooLarge(msg)) if msg.contains("4096"))
    );
    let mut files = sample_files();
    files.extend((0..MAX_FILES).map(|i| file(&format!("data/{i:05}"), b"")));
    assert!(matches!(
        Package::from_files(files),
        Err(PackageError::TooLarge(_))
    ));

    // Exactly 4096 files is fine.
    let mut files = sample_files();
    let room = MAX_FILES - files.len();
    files.extend((0..room).map(|i| file(&format!("data/{i:05}"), b"")));
    assert_eq!(Package::from_files(files).unwrap().files().len(), MAX_FILES);
}

// ---------------------------------------------------------------------------------------------
// In-memory file sets

#[test]
fn from_files_path_rules() {
    for bad in [
        "../escape",
        "/etc/passwd",
        "src/../../x",
        "src\\lib.rs",
        "src//lib.rs",
        "./mod.toml",
        "src/",
        "",
        "src/a\nb.rs",
        "src/nul\0.rs",
    ] {
        let mut files = sample_files();
        files.push(file(bad, b"x"));
        assert_forbidden(Package::from_files(files), bad);
    }
    let mut files = sample_files();
    files.push(file("target/debug/x", b""));
    assert_forbidden(Package::from_files(files), "target/debug/x");
    let mut files = sample_files();
    files.push(file("Cargo.toml", b""));
    assert_forbidden(Package::from_files(files), "Cargo.toml");
    let mut files = sample_files();
    files.push(file("build.rs", b""));
    assert_forbidden(Package::from_files(files), "build.rs");
    let mut files = sample_files();
    files.push(file("src", b""));
    assert_forbidden(Package::from_files(files), "src");
}

#[test]
fn from_files_rejects_duplicates_and_file_directory_clashes() {
    let mut files = sample_files();
    files.push(file("src/lib.rs", b"another"));
    assert_forbidden(Package::from_files(files), "src/lib.rs");
    let mut files = sample_files();
    files.push(file("src/ui", b"a file where a directory is"));
    assert_forbidden(Package::from_files(files), "src/ui");
}

// ---------------------------------------------------------------------------------------------
// Archives

#[test]
fn archive_round_trip() {
    let dir = sample_dir();
    let (package, _) = Package::from_dir(dir.path()).unwrap();
    let bytes = package.to_archive_bytes();
    let read = Package::from_archive_bytes(&bytes).unwrap();
    assert_eq!(read.hash(), package.hash());
    assert_eq!(read.files(), package.files());
    assert_eq!(read.manifest(), package.manifest());

    let out = TempDir::new().unwrap();
    let path = package.write_archive(&out.path().join("dist")).unwrap();
    assert_eq!(path, out.path().join("dist/example.thing-1.2.3.pwcmod"));
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let from_file = Package::from_archive_file(&path).unwrap();
    assert_eq!(from_file.hash(), package.hash());
    // No temporary files are left behind; writing again replaces the file.
    package.write_archive(&out.path().join("dist")).unwrap();
    assert_eq!(fs::read_dir(out.path().join("dist")).unwrap().count(), 1);
}

#[test]
fn any_zstd_frame_is_accepted() {
    let package = Package::from_files(sample_files()).unwrap();
    let raw = ruzstd::encoding::compress_to_vec(
        &package.canonical_tar()[..],
        ruzstd::encoding::CompressionLevel::Uncompressed,
    );
    assert_ne!(raw, package.to_archive_bytes());
    assert_eq!(
        Package::from_archive_bytes(&raw).unwrap().hash(),
        package.hash()
    );
}

#[test]
fn long_paths_survive_archives() {
    let mut files = sample_files();
    let long = format!("assets/{}/deep-ü-file.txt", "nested-directory".repeat(10));
    assert!(long.len() > 100);
    files.push(file(&long, b"deep"));
    let package = Package::from_files(files).unwrap();
    let read = Package::from_archive_bytes(&package.to_archive_bytes()).unwrap();
    assert_eq!(read.hash(), package.hash());
    assert!(read.files().iter().any(|f| f.path == long));
}

#[test]
fn verify_hash() {
    let package = Package::from_files(sample_files()).unwrap();
    package.verify_hash(&package.hash().clone()).unwrap();
    let other = PackageHash::from_digest([0; 32]);
    match package.verify_hash(&other) {
        Err(PackageError::HashMismatch { expected, found }) => {
            assert_eq!(expected, other);
            assert_eq!(&found, package.hash());
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn tampered_contents_change_the_hash() {
    let package = Package::from_files(sample_files()).unwrap();
    let mut files = sample_files();
    files
        .iter_mut()
        .find(|f| f.path == "src/lib.rs")
        .unwrap()
        .bytes
        .extend_from_slice(b"// evil\n");
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let tampered = Package::from_archive_bytes(&archive_of(&files)).unwrap();
    assert_ne!(tampered.hash(), package.hash());
    assert!(matches!(
        tampered.verify_hash(package.hash()),
        Err(PackageError::HashMismatch { .. })
    ));

    // Flipping one byte of file data inside the tar gives a different (valid) package.
    let mut tar = package.canonical_tar();
    let at = tar.windows(9).position(|w| w == b"Press T.\n").unwrap();
    tar[at] = b'p';
    let flipped = Package::from_archive_bytes(&zstd::compress(&tar)).unwrap();
    assert_ne!(flipped.hash(), package.hash());
}

#[test]
fn non_canonical_archives_are_rejected() {
    let package = Package::from_files(sample_files()).unwrap();
    let tar = package.canonical_tar();

    // A header field changed (and the checksum fixed), e.g. a non-zero mtime or another mode.
    for (range, value) in [
        (136..148, &b"14000000000\0"[..]),
        (100..108, b"0000755\0"),
        (265..269, b"root"),
        (329..337, b"\0\0\0\0\0\0\0\0"),
    ] {
        let mut edited = tar.clone();
        edited[range.clone()].copy_from_slice(value);
        refresh_checksum(&mut edited[..512]);
        let err = Package::from_archive_bytes(&zstd::compress(&edited)).unwrap_err();
        assert!(matches!(err, PackageError::Archive(_)), "{range:?}: {err}");
    }

    // Record-size padding after the end blocks, as `tar` tools write it.
    let mut padded = tar.clone();
    padded.resize(tar.len().next_multiple_of(10240), 0);
    assert!(matches!(
        Package::from_archive_bytes(&zstd::compress(&padded)),
        Err(PackageError::Archive(_))
    ));

    // The same files written by another tar implementation.
    let mut builder = ::tar::Builder::new(Vec::new());
    for f in package.files() {
        let mut header = ::tar::Header::new_ustar();
        header.set_size(f.bytes.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_cksum();
        builder
            .append_data(&mut header, &f.path, &f.bytes[..])
            .unwrap();
    }
    let other = builder.into_inner().unwrap();
    assert_ne!(other, tar);
    assert!(matches!(
        Package::from_archive_bytes(&zstd::compress(&other)),
        Err(PackageError::Archive(_))
    ));

    // Uncompressed tar, gzip-ish garbage, nothing.
    assert!(matches!(
        Package::from_archive_bytes(&tar),
        Err(PackageError::Archive(_))
    ));
    assert!(matches!(
        Package::from_archive_bytes(&[0x1f, 0x8b, 8, 0]),
        Err(PackageError::Archive(_))
    ));
    assert!(matches!(
        Package::from_archive_bytes(&[]),
        Err(PackageError::Archive(_))
    ));

    // Two frames.
    let mut two = package.to_archive_bytes();
    two.extend_from_slice(&package.to_archive_bytes());
    assert!(matches!(
        Package::from_archive_bytes(&two),
        Err(PackageError::Archive(_))
    ));
}

#[test]
fn unsorted_and_duplicate_entries_are_rejected() {
    let mut files = sample_files();
    files.swap(0, 1);
    let err = Package::from_archive_bytes(&archive_of(&files)).unwrap_err();
    assert!(
        matches!(&err, PackageError::Archive(msg) if msg.contains("sorted")),
        "{err}"
    );

    let mut files = sample_files();
    files.insert(1, files[0].clone());
    let err = Package::from_archive_bytes(&archive_of(&files)).unwrap_err();
    assert!(
        matches!(&err, PackageError::Archive(msg) if msg.contains("duplicate")),
        "{err}"
    );
}

#[test]
fn unsafe_and_foreign_entries_are_rejected() {
    for bad in [
        "../../etc/cron.d/evil",
        "/etc/passwd",
        "src/../../../evil",
        "src\\..\\evil",
        "src/./lib2.rs",
        "target/x",
        "src/build.rs",
        "assets/.hidden",
        "Cargo.toml",
    ] {
        let mut files = sample_files();
        files.push(file(bad, b"payload"));
        files.sort_by(|a, b| a.path.cmp(&b.path));
        assert_forbidden(Package::from_archive_bytes(&archive_of(&files)), bad);
    }
    // A directory entry: valid checksum, wrong type.
    let mut tar = Package::from_files(sample_files()).unwrap().canonical_tar();
    tar[156] = b'5';
    refresh_checksum(&mut tar[..512]);
    assert!(
        matches!(Package::from_archive_bytes(&zstd::compress(&tar)), Err(PackageError::Archive(msg)) if msg.contains("regular file"))
    );
}

#[test]
fn too_many_entries_stop_early() {
    let mut files = sample_files();
    files.extend((0..MAX_FILES).map(|i| file(&format!("data/{i:05}"), b"")));
    files.sort_by(|a, b| a.path.cmp(&b.path));
    assert!(matches!(
        Package::from_archive_bytes(&archive_of(&files)),
        Err(PackageError::TooLarge(_))
    ));
}

#[test]
fn decompression_bombs_are_refused() {
    // A hand-made Zstandard frame of RLE blocks (4 bytes each, 128 KiB of zeros each) that
    // expands to more than any package may hold: decoding stops at the limit.
    let block = 128 * 1024u32;
    let blocks = (MAX_TAR_BYTES / u64::from(block)) as u32 + 8;
    let mut bomb = vec![0x28, 0xB5, 0x2F, 0xFD, 0x00, 7 << 3]; // magic, descriptor, 128 KiB window
    for i in 0..blocks {
        let last = u32::from(i + 1 == blocks);
        let header = last | (1 << 1) | (block << 3);
        bomb.extend_from_slice(&header.to_le_bytes()[..3]);
        bomb.push(0);
    }
    assert!(bomb.len() < 16 * 1024);
    let err = Package::from_archive_bytes(&bomb).unwrap_err();
    assert!(
        matches!(&err, PackageError::Archive(msg) if msg.contains("more than")),
        "{err}"
    );
}

#[test]
fn malformed_archives_never_panic() {
    let package = Package::from_files(sample_files()).unwrap();
    let archive = package.to_archive_bytes();
    let tar = package.canonical_tar();
    let mut rng = XorShift(0x9e37_79b9_7f4a_7c15);

    // Truncated archives.
    for cut in 0..archive.len() {
        assert!(
            Package::from_archive_bytes(&archive[..cut]).is_err(),
            "cut at {cut}"
        );
    }
    // Random bit flips, byte overwrites, insertions and deletions in the compressed stream.
    for _ in 0..3000 {
        let mut bytes = archive.clone();
        mutate(&mut bytes, &mut rng);
        let _ = Package::from_archive_bytes(&bytes);
    }
    // Mutations of the tar itself (recompressed), which reach the tar parser.
    for _ in 0..3000 {
        let mut bytes = tar.clone();
        mutate(&mut bytes, &mut rng);
        if let Ok(read) = Package::from_archive_bytes(&zstd::compress(&bytes)) {
            // Only data-byte changes can survive; the result is a valid package of its own.
            assert_eq!(read.canonical_tar(), bytes);
        }
    }
    // Pure garbage.
    for len in [1usize, 4, 5, 13, 512, 4096] {
        let bytes: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
        assert!(Package::from_archive_bytes(&bytes).is_err());
        let mut framed = vec![0x28, 0xB5, 0x2F, 0xFD];
        framed.extend_from_slice(&bytes);
        let _ = Package::from_archive_bytes(&framed);
    }
}

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn mutate(bytes: &mut Vec<u8>, rng: &mut XorShift) {
    let at = rng.next() as usize % bytes.len();
    match rng.next() % 4 {
        0 => bytes[at] ^= 1 << (rng.next() % 8),
        1 => bytes[at] = rng.next() as u8,
        2 => bytes.insert(at, rng.next() as u8),
        _ => {
            bytes.remove(at);
        }
    }
}

fn refresh_checksum(h: &mut [u8]) {
    h[148..156].fill(b' ');
    let sum: u32 = h.iter().map(|&b| u32::from(b)).sum();
    h[148..154].copy_from_slice(format!("{sum:06o}").as_bytes());
    h[154] = 0;
    h[155] = b' ';
}

// ---------------------------------------------------------------------------------------------
// Unpacking

#[test]
fn unpack_and_repackage() {
    let package = Package::from_files(sample_files()).unwrap();
    let root = TempDir::new().unwrap();
    let target = root.path().join("store/abc");
    package.unpack_to(&target).unwrap();
    for f in package.files() {
        assert_eq!(
            fs::read(target.join(&f.path)).unwrap(),
            f.bytes,
            "{}",
            f.path
        );
    }
    // An unpacked package is a source directory with nothing ignored and the same hash.
    let (again, ignored) = Package::from_dir(&target).unwrap();
    assert!(ignored.is_empty());
    assert_eq!(again.hash(), package.hash());

    // A non-empty directory is refused; an existing empty one is fine.
    let err = package.unpack_to(&target).unwrap_err();
    assert!(
        matches!(&err, PackageError::Io { source, .. } if source.kind() == io::ErrorKind::DirectoryNotEmpty),
        "{err}"
    );
    let empty = root.path().join("empty");
    fs::create_dir(&empty).unwrap();
    package.unpack_to(&empty).unwrap();
    // A file in the way is an error, not a panic.
    write(root.path(), "a-file", b"");
    assert!(matches!(
        package.unpack_to(&root.path().join("a-file")),
        Err(PackageError::Io { .. })
    ));
}

#[test]
fn error_messages() {
    let forbidden = PackageError::Forbidden {
        path: "src/build.rs".into(),
        reason: "build scripts are not allowed".into(),
    };
    assert_eq!(
        forbidden.to_string(),
        "forbidden path `src/build.rs`: build scripts are not allowed"
    );
    assert_eq!(
        PackageError::MissingFile("README.md".into()).to_string(),
        "missing required file `README.md`"
    );
}
