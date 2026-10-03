//! Builder tests: Build IDs, the generated workspace, and the build pipeline driven through a
//! fake Cargo (a wrapper script), so no real compilation happens.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pwc_manifest::{
    BuildProfile, Edition, LockedPackage, LockedPwc, Lockfile, ModKind, ModManifest, PackageHash,
    PackageId, PackageMetadata, PackageSource, Version, VersionReq,
};

use super::*;
use crate::cargo::parse_rustc_verbose;
use crate::generate::{rust_str, toml_str};
use crate::metadata::{civil_from_days, rfc3339};

const RUSTC: &str = "rustc 1.95.0 (59807616e 2026-04-14)\nbinary: rustc\nhost: x86_64-unknown-linux-gnu\nrelease: 1.95.0\n";
const TARGET: &str = "x86_64-unknown-linux-gnu";

const PWC_CARGO_TOML: &str = r#"[package]
name = "project_watt_cubed"
version = "2.0.0"
edition = "2024"

[workspace]
resolver = "2"
members = ["crates/pwc-mod-api"]

[dependencies]
voxel_engine = { path = "../voxel-engine" }

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = true

[profile.dev.package."*"]
opt-level = 2

[profile.bench]
inherits = "release"
debug = true
"#;

fn id(s: &str) -> PackageId {
    PackageId::parse(s).unwrap()
}

fn hash_of(s: &str) -> PackageHash {
    use sha2::Digest as _;
    PackageHash::from_digest(sha2::Sha256::digest(s.as_bytes()).into())
}

/// A test package: id, kind and dependency ids (version 1.0.0).
struct Pkg {
    id: &'static str,
    kind: ModKind,
    deps: &'static [&'static str],
}

const fn pkg(id: &'static str, kind: ModKind, deps: &'static [&'static str]) -> Pkg {
    Pkg { id, kind, deps }
}

fn manifest(p: &Pkg) -> ModManifest {
    ModManifest {
        package: PackageMetadata {
            id: id(p.id),
            name: "Test package".to_owned(),
            version: Version::new(1, 0, 0),
            description: "A test package.".to_owned(),
            authors: vec!["Tester".to_owned()],
            license: "AGPL-3.0-or-later".to_owned(),
            license_files: vec!["LICENSE".to_owned()],
            kind: p.kind,
            pwc_api: (p.kind != ModKind::Bundle).then(|| VersionReq::parse("^1.0").unwrap()),
            edition: if p.id.ends_with("old") {
                Edition::E2021
            } else {
                Edition::E2024
            },
            readme: "README.md".to_owned(),
            icon: None,
            repository: None,
            homepage: None,
            documentation: None,
            keywords: Vec::new(),
            categories: Vec::new(),
        },
        dependencies: p
            .deps
            .iter()
            .map(|d| (id(d), VersionReq::parse("^1.0").unwrap()))
            .collect(),
        conflicts: BTreeMap::new(),
        metadata: None,
    }
}

/// A fake world: a PWC source, a store, cache directories.
struct World {
    root: tempfile::TempDir,
    pwc: PathBuf,
}

impl World {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let pwc = root.path().join("project_watt_cubed");
        write(&pwc.join("Cargo.toml"), PWC_CARGO_TOML);
        write(&pwc.join("Cargo.lock"), "# seed lock\nversion = 4\n");
        write(
            &pwc.join("crates/pwc-mod-api/Cargo.toml"),
            "[package]\nname = \"pwc-mod-api\"\nversion = \"1.0.0\"\nedition = \"2024\"\n",
        );
        Self { root, pwc }
    }

    fn store_dir(&self, p: &Pkg) -> PathBuf {
        self.root.path().join("store").join(hash_of(p.id).hex())
    }

    /// A request for `pkgs`, with store directories populated (mod/library: `src/lib.rs`).
    fn request(&self, pkgs: &[Pkg]) -> BuildRequest {
        let locked: Vec<LockedPackage> = pkgs
            .iter()
            .map(|p| LockedPackage {
                id: id(p.id),
                version: Version::new(1, 0, 0),
                kind: p.kind,
                hash: hash_of(p.id),
                source: PackageSource::Store,
                dependencies: p.deps.iter().map(|d| id(d)).collect(),
            })
            .collect();
        let pwc = LockedPwc {
            version: Version::new(2, 0, 0),
            api: Version::new(1, 0, 0),
            source: self.pwc.clone(),
            revision: "0123abc".to_owned(),
            dirty: false,
        };
        let lock = Lockfile::new(pwc, locked);
        let mut packages = BTreeMap::new();
        for p in pkgs {
            let dir = self.store_dir(p);
            std::fs::create_dir_all(&dir).unwrap();
            if p.kind != ModKind::Bundle {
                write(
                    &dir.join("src/lib.rs"),
                    "pub fn register(_: &mut pwc_mod_api::ModRegistrar) {}\n",
                );
            }
            packages.insert(id(p.id), (dir, manifest(p)));
        }
        BuildRequest {
            lock,
            packages,
            pwc_source: self.pwc.clone(),
            pwc_dirty_digest: "clean".to_owned(),
            profile: BuildProfile::Release,
            builds_dir: self.root.path().join("cache/builds"),
            target_dir: self.root.path().join("cache/target"),
            wrapper: Vec::new(),
            jobs: 0,
            force: false,
        }
    }
}

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

const MODS: &[Pkg] = &[
    pkg("pwc.inventory", ModKind::Mod, &["pwc.hotbar", "pwc.names"]),
    pkg("pwc.hotbar", ModKind::Mod, &[]),
    pkg("pwc.names", ModKind::Library, &[]),
    pkg(
        "pwc.essentials",
        ModKind::Bundle,
        &["pwc.inventory", "pwc.zoom"],
    ),
    pkg("pwc.zoom", ModKind::Mod, &[]),
    pkg("pwc.alpha", ModKind::Mod, &["pwc.essentials"]),
];

// ---------------------------------------------------------------------------------------------
// Build ID

#[test]
fn build_id_matches_the_spec_formula() {
    let world = World::new();
    let request = world.request(MODS);
    let text = format!(
        "pwc-build 1\nbuilder {BUILDER_VERSION}\nenvironment {}\npwc-source 0123abc clean\nrustc {}\nprofile release\ntarget {TARGET}\n",
        request.lock.environment,
        RUSTC.replace('\n', " ")
    );
    use sha2::Digest as _;
    let expected = hex(&sha2::Sha256::digest(text.as_bytes()));
    let id = build_id(&request, RUSTC, TARGET);
    assert_eq!(id, expected);
    assert_eq!(id.len(), 64);
    assert!(
        id.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );
    assert_eq!(build_dir_name(&id), &id[..32]);
}

#[test]
fn build_id_changes_with_every_input() {
    let world = World::new();
    let base = world.request(MODS);
    let id = build_id(&base, RUSTC, TARGET);
    assert_eq!(id, build_id(&base.clone(), RUSTC, TARGET), "deterministic");

    let mut other = base.clone();
    other.profile = BuildProfile::Dev;
    assert_ne!(id, build_id(&other, RUSTC, TARGET), "profile");

    let mut other = base.clone();
    other.pwc_dirty_digest = "abc".to_owned();
    assert_ne!(id, build_id(&other, RUSTC, TARGET), "dirty digest");

    let mut other = base.clone();
    other.lock.pwc.revision = "fedcba".to_owned();
    assert_ne!(id, build_id(&other, RUSTC, TARGET), "revision");

    let other = world.request(&MODS[1..2]);
    assert_ne!(id, build_id(&other, RUSTC, TARGET), "environment");

    assert_ne!(
        id,
        build_id(&base, &RUSTC.replace("1.95.0", "1.96.0"), TARGET),
        "rustc"
    );
    assert_ne!(
        id,
        build_id(&base, RUSTC, "aarch64-unknown-linux-gnu"),
        "target"
    );

    // Inputs that are not part of the formula do not matter.
    let mut other = base.clone();
    other.jobs = 8;
    other.force = true;
    other.wrapper = vec!["nix".into()];
    assert_eq!(id, build_id(&other, RUSTC, TARGET));
}

// ---------------------------------------------------------------------------------------------
// Generated workspace

#[test]
fn bundle_registers_mods_in_topological_order_with_ties_by_id() {
    let world = World::new();
    let ws = generate(&world.request(MODS)).unwrap();
    let lib = &ws.files["bundle/src/lib.rs"];
    let registered: Vec<&str> = lib
        .lines()
        .filter_map(|l| l.trim().strip_prefix("id: \""))
        .map(|l| l.split('"').next().unwrap())
        .collect();
    // Dependencies first; among ready packages the smallest id. The library (pwc.names) and the
    // bundle (pwc.essentials) take part in the ordering but are not registered.
    assert_eq!(
        registered,
        ["pwc.hotbar", "pwc.inventory", "pwc.zoom", "pwc.alpha"]
    );
    assert!(lib.contains("register: pwc_hotbar::register,"));
    assert!(
        !lib.contains("pwc_names"),
        "libraries are not registered:\n{lib}"
    );
    assert!(
        !lib.contains("pwc_essentials"),
        "bundles generate nothing:\n{lib}"
    );
}

#[test]
fn bundle_lib_has_the_spec_shape() {
    let world = World::new();
    let request = world.request(&MODS[1..2]);
    let ws = generate(&request).unwrap();
    let expected = format!(
        "//! Generated by pwc. Do not edit.\n\n\
         /// The environment hash of the lock this bundle was generated from.\n\
         pub const ENVIRONMENT: &str = \"{}\";\n\n\
         /// The game build: every `kind = \"mod\"` package, dependencies before dependents, ties by id.\n\
         pub fn game_build() -> pwc_mod_api::GameBuild {{\n    \
         pwc_mod_api::GameBuild::new()\n        \
         .with_environment(ENVIRONMENT)\n        \
         .with_mod(pwc_mod_api::ModDescriptor {{\n            \
         id: \"pwc.hotbar\", name: \"Test package\", version: \"1.0.0\",\n            \
         register: pwc_hotbar::register,\n        \
         }})\n}}\n",
        request.lock.environment
    );
    assert_eq!(ws.files["bundle/src/lib.rs"], expected);
}

#[test]
fn every_mod_and_library_gets_a_crate_and_bundles_none() {
    let world = World::new();
    let request = world.request(MODS);
    let ws = generate(&request).unwrap();
    let crates: Vec<&str> = ws
        .files
        .keys()
        .filter_map(|k| {
            k.strip_prefix("mods/")
                .and_then(|k| k.strip_suffix("/Cargo.toml"))
        })
        .collect();
    assert_eq!(
        crates,
        [
            "pwc_alpha",
            "pwc_hotbar",
            "pwc_inventory",
            "pwc_names",
            "pwc_zoom"
        ]
    );
    let mut paths: Vec<&str> = ws
        .files
        .keys()
        .map(String::as_str)
        .filter(|k| !k.starts_with("mods/"))
        .collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        [
            "Cargo.lock",
            "Cargo.toml",
            "bundle/Cargo.toml",
            "bundle/src/lib.rs",
            "instance/Cargo.toml",
            "instance/src/bin/pwc-game.rs",
            "instance/src/bin/pwc-golden.rs",
        ]
    );

    // The root lists every member explicitly.
    let root: toml::Table = ws.files["Cargo.toml"].parse().unwrap();
    let members: Vec<&str> = root["workspace"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        members,
        [
            "bundle",
            "instance",
            "mods/pwc_alpha",
            "mods/pwc_hotbar",
            "mods/pwc_inventory",
            "mods/pwc_names",
            "mods/pwc_zoom"
        ]
    );
    assert_eq!(root["workspace"]["resolver"].as_str(), Some("2"));
}

#[test]
fn mod_crates_depend_on_the_api_and_their_code_dependencies() {
    let world = World::new();
    let request = world.request(MODS);
    let ws = generate(&request).unwrap();
    let api = world.pwc.join("crates/pwc-mod-api");

    let inventory: toml::Table = ws.files["mods/pwc_inventory/Cargo.toml"].parse().unwrap();
    assert_eq!(inventory["package"]["name"].as_str(), Some("pwc_inventory"));
    assert_eq!(inventory["package"]["version"].as_str(), Some("1.0.0"));
    assert_eq!(inventory["package"]["edition"].as_str(), Some("2024"));
    assert_eq!(inventory["package"]["publish"].as_bool(), Some(false));
    let lib = world.store_dir(&MODS[0]).join("src/lib.rs");
    assert_eq!(
        inventory["lib"]["path"].as_str(),
        Some(lib.to_str().unwrap())
    );
    let deps = inventory["dependencies"].as_table().unwrap();
    assert_eq!(
        deps.keys().collect::<Vec<_>>(),
        ["pwc-mod-api", "pwc_hotbar", "pwc_names"]
    );
    assert_eq!(
        deps["pwc-mod-api"]["path"].as_str(),
        Some(api.to_str().unwrap())
    );
    assert_eq!(deps["pwc_hotbar"]["path"].as_str(), Some("../pwc_hotbar"));

    // A mod depending on a bundle gets no crate dependency for it (bundles have no code).
    let alpha: toml::Table = ws.files["mods/pwc_alpha/Cargo.toml"].parse().unwrap();
    assert_eq!(
        alpha["dependencies"]
            .as_table()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["pwc-mod-api"]
    );

    // The bundle crate depends on the API and every mod (not on libraries).
    let bundle: toml::Table = ws.files["bundle/Cargo.toml"].parse().unwrap();
    assert_eq!(bundle["package"]["name"].as_str(), Some(BUNDLE_PACKAGE));
    assert_eq!(bundle["lib"]["name"].as_str(), Some("pwc_bundle"));
    assert_eq!(
        bundle["dependencies"]
            .as_table()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        [
            "pwc-mod-api",
            "pwc_alpha",
            "pwc_hotbar",
            "pwc_inventory",
            "pwc_zoom"
        ]
    );
    assert_eq!(
        bundle["dependencies"]["pwc_zoom"]["path"].as_str(),
        Some("../mods/pwc_zoom")
    );

    // The instance crate: two binaries, the runtime by absolute path, the bundle.
    let instance: toml::Table = ws.files["instance/Cargo.toml"].parse().unwrap();
    assert_eq!(instance["package"]["name"].as_str(), Some(INSTANCE_PACKAGE));
    let bins: Vec<&str> = instance["bin"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["name"].as_str().unwrap())
        .collect();
    assert_eq!(bins, [GAME_BIN, GOLDEN_BIN]);
    assert_eq!(
        instance["dependencies"]["project_watt_cubed"]["path"].as_str(),
        Some(world.pwc.to_str().unwrap())
    );
    assert_eq!(
        instance["dependencies"]["pwc-bundle"]["path"].as_str(),
        Some("../bundle")
    );
    assert_eq!(
        ws.files["instance/src/bin/pwc-game.rs"],
        "//! Generated by pwc. Do not edit.\n\nfn main() {\n    project_watt_cubed::run(pwc_bundle::game_build());\n}\n"
    );
    assert!(
        ws.files["instance/src/bin/pwc-golden.rs"]
            .contains("project_watt_cubed::harness::golden_main(pwc_bundle::game_build());")
    );
}

#[test]
fn edition_follows_the_manifest() {
    let world = World::new();
    let ws = generate(&world.request(&[pkg("pwc.old", ModKind::Mod, &[])])).unwrap();
    let old: toml::Table = ws.files["mods/pwc_old/Cargo.toml"].parse().unwrap();
    assert_eq!(old["package"]["edition"].as_str(), Some("2021"));
}

#[test]
fn profiles_are_copied_including_nested_tables() {
    let world = World::new();
    let ws = generate(&world.request(MODS)).unwrap();
    let root: toml::Table = ws.files["Cargo.toml"].parse().unwrap();
    let source: toml::Table = PWC_CARGO_TOML.parse().unwrap();
    assert_eq!(root["profile"], source["profile"]);
    assert_eq!(
        root["profile"]["dev"]["package"]["*"]["opt-level"].as_integer(),
        Some(2)
    );
    assert_eq!(root["profile"]["release"]["lto"].as_str(), Some("fat"));
    assert!(!root.contains_key("dependencies") && !root.contains_key("package"));
}

#[test]
fn patch_tables_are_copied_with_absolute_paths() {
    let world = World::new();
    let text = format!(
        "{PWC_CARGO_TOML}\n[patch.crates-io]\nglam = {{ path = \"../glam\" }}\nash = {{ git = \"https://example.org/ash\" }}\n"
    );
    write(&world.pwc.join("Cargo.toml"), &text);
    let ws = generate(&world.request(MODS)).unwrap();
    let root: toml::Table = ws.files["Cargo.toml"].parse().unwrap();
    let glam = world.pwc.join("../glam");
    assert_eq!(
        root["patch"]["crates-io"]["glam"]["path"].as_str(),
        Some(glam.to_str().unwrap())
    );
    assert_eq!(
        root["patch"]["crates-io"]["ash"]["git"].as_str(),
        Some("https://example.org/ash")
    );
}

#[test]
fn cargo_lock_is_seeded_from_the_pwc_source() {
    let world = World::new();
    let ws = generate(&world.request(MODS)).unwrap();
    assert_eq!(ws.files["Cargo.lock"], "# seed lock\nversion = 4\n");

    std::fs::remove_file(world.pwc.join("Cargo.lock")).unwrap();
    let ws = generate(&world.request(MODS)).unwrap();
    assert!(!ws.files.contains_key("Cargo.lock"));
}

#[test]
fn names_and_paths_are_escaped() {
    const TRICKY: &str = "Quote \"Q\" \\ back\tslash Höhle \u{202e}rtl";
    let world = World::new();
    let mut request = world.request(&MODS[1..2]);
    request
        .packages
        .get_mut(&id("pwc.hotbar"))
        .unwrap()
        .1
        .package
        .name = TRICKY.to_owned();
    let ws = generate(&request).unwrap();
    let lib = &ws.files["bundle/src/lib.rs"];
    assert!(
        lib.contains(r#"name: "Quote \"Q\" \\ back\u{9}slash H\u{f6}hle \u{202e}rtl""#),
        "{lib}"
    );
    assert!(lib.is_ascii(), "generated Rust is pure ASCII");

    assert_eq!(rust_str("plain"), "\"plain\"");
    assert_eq!(toml_str("a\"b\\c\nd é"), "\"a\\\"b\\\\c\\u000Ad é\"");
    let parsed: toml::Table = format!("x = {}", toml_str(TRICKY)).parse().unwrap();
    assert_eq!(parsed["x"].as_str(), Some(TRICKY));
}

#[test]
fn generation_is_deterministic() {
    let world = World::new();
    assert_eq!(
        generate(&world.request(MODS)).unwrap(),
        generate(&world.request(MODS)).unwrap()
    );
}

#[test]
fn empty_lock_builds_vanilla() {
    let world = World::new();
    let ws = generate(&world.request(&[])).unwrap();
    assert!(ws.files["bundle/src/lib.rs"].contains(".with_environment(ENVIRONMENT)\n}\n"));
    let root: toml::Table = ws.files["Cargo.toml"].parse().unwrap();
    assert_eq!(root["workspace"]["members"].as_array().unwrap().len(), 2);
}

// ---------------------------------------------------------------------------------------------
// Validation

fn generate_err(request: &BuildRequest) -> String {
    generate(request).unwrap_err().to_string()
}

#[test]
fn pwc_version_and_api_must_match_the_lock() {
    let world = World::new();
    let mut request = world.request(MODS);
    request.lock.pwc.version = Version::new(2, 1, 0);
    assert!(generate_err(&request).contains("the checkout is PWC 2.0.0 but the lock pins 2.1.0"));

    let mut request = world.request(MODS);
    request.lock.pwc.api = Version::new(1, 1, 0);
    assert!(generate_err(&request).contains("pwc-mod-api 1.0.0 but the lock pins 1.1.0"));
}

#[test]
fn workspace_inherited_versions_are_followed() {
    let world = World::new();
    write(
        &world.pwc.join("crates/pwc-mod-api/Cargo.toml"),
        "[package]\nname = \"pwc-mod-api\"\nversion.workspace = true\n",
    );
    let text = PWC_CARGO_TOML.replace(
        "resolver = \"2\"",
        "resolver = \"2\"\n[workspace.package]\nversion = \"1.0.0\"",
    );
    write(&world.pwc.join("Cargo.toml"), &text);
    let mut request = world.request(MODS);
    generate(&request).unwrap();
    request.lock.pwc.api = Version::new(2, 0, 0);
    assert!(generate_err(&request).contains("pwc-mod-api 1.0.0"));
}

#[test]
fn a_source_without_the_api_crate_is_rejected() {
    let world = World::new();
    std::fs::remove_dir_all(world.pwc.join("crates")).unwrap();
    assert!(generate_err(&world.request(MODS)).contains("has no `pwc-mod-api` crate"));

    let world = World::new();
    write(
        &world.pwc.join("Cargo.toml"),
        "[package]\nname = \"something-else\"\nversion = \"2.0.0\"\n",
    );
    assert!(generate_err(&world.request(MODS)).contains("is not the `project_watt_cubed` package"));
}

#[test]
fn packages_must_accept_the_api_version() {
    let world = World::new();
    let mut request = world.request(MODS);
    request
        .packages
        .get_mut(&id("pwc.zoom"))
        .unwrap()
        .1
        .package
        .pwc_api = Some(VersionReq::parse("^2.0").unwrap());
    let err = generate(&request).unwrap_err();
    assert!(
        matches!(&err, BuildError::Package(p, m) if p.as_str() == "pwc.zoom" && m.contains("requires pwc-api ^2.0")),
        "{err}"
    );
}

#[test]
fn store_manifests_must_match_the_lock() {
    let world = World::new();
    let mut request = world.request(MODS);
    request
        .packages
        .get_mut(&id("pwc.zoom"))
        .unwrap()
        .1
        .package
        .version = Version::new(1, 0, 1);
    assert!(
        generate_err(&request)
            .contains("store entry is pwc.zoom 1.0.1 (mod) but the lock says pwc.zoom 1.0.0 (mod)")
    );

    let mut request = world.request(MODS);
    request.packages.remove(&id("pwc.zoom"));
    assert!(generate_err(&request).contains("package pwc.zoom: not in the store"));
}

#[test]
fn clashing_and_reserved_crate_names_are_rejected() {
    let world = World::new();
    let request = world.request(&[
        pkg("foo.bar-baz", ModKind::Mod, &[]),
        pkg("foo.bar.baz", ModKind::Mod, &[]),
    ]);
    assert!(generate_err(&request).contains("have the same crate name `foo_bar_baz`"));

    let request = world.request(&[pkg("pwc.mod-api", ModKind::Library, &[])]);
    assert!(generate_err(&request).contains("crate name `pwc_mod_api` is reserved"));

    // A bundle has no crate, so its name cannot clash.
    let request = world.request(&[
        pkg("pwc.bundle", ModKind::Bundle, &["pwc.x"]),
        pkg("pwc.x", ModKind::Mod, &[]),
    ]);
    generate(&request).unwrap();
}

#[test]
fn cycles_and_dangling_dependencies_are_rejected() {
    let world = World::new();
    let mut request = world.request(&[
        pkg("pwc.a", ModKind::Mod, &["pwc.b"]),
        pkg("pwc.b", ModKind::Mod, &[]),
    ]);
    request.lock.packages[1].dependencies = vec![id("pwc.a")];
    assert!(generate_err(&request).contains("dependency cycle among pwc.a, pwc.b"));

    let mut request = world.request(&[pkg("pwc.a", ModKind::Mod, &[])]);
    request.lock.packages[0].dependencies = vec![id("pwc.gone")];
    assert!(generate_err(&request).contains("pwc.a depends on pwc.gone, which is not in the lock"));
}

// ---------------------------------------------------------------------------------------------
// The pipeline, with a fake toolchain behind the wrapper

#[test]
fn rustc_output_is_parsed_after_wrapper_noise() {
    let noisy = format!("project_watt_cubed dev shell — run 'cargo run --release'\n{RUSTC}");
    let (verbose, host) = parse_rustc_verbose(&noisy).unwrap();
    assert_eq!(verbose, RUSTC);
    assert_eq!(host, TARGET);
    assert_eq!(parse_rustc_verbose(RUSTC).unwrap().0, RUSTC);
    assert!(parse_rustc_verbose("no rustc here\n").is_none());
    assert!(parse_rustc_verbose("rustc 1.0\nbinary: rustc\n").is_none());
}

#[cfg(unix)]
mod fake_toolchain {
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    /// A wrapper standing in for `nix develop … --command`: answers `rustc -vV` (after some
    /// shell-hook noise), "builds" by writing executables into `$CARGO_TARGET_DIR/<profile>/`,
    /// and logs every Cargo invocation.
    fn fake_wrapper(dir: &Path, fail: bool) -> Vec<String> {
        let script = dir.join("fake-toolchain.sh");
        let body = format!(
            r#"#!/bin/sh
set -e
tool="$1"; shift
case "$tool" in
  rustc)
    echo "dev shell hook noise"
    printf 'rustc 1.95.0 (fake)\nbinary: rustc\nhost: {TARGET}\nrelease: 1.95.0\n'
    ;;
  cargo)
    echo "cargo $* | cwd=$(pwd)" >> "{log}"
    {fail}
    manifest=""
    profile=debug
    while [ $# -gt 0 ]; do
      case "$1" in
        --manifest-path) manifest="$2"; shift ;;
        --release) profile=release ;;
      esac
      shift
    done
    test -f "$manifest"
    test -f "$(dirname "$manifest")/bundle/src/lib.rs"
    mkdir -p "$CARGO_TARGET_DIR/$profile"
    for bin in pwc-game pwc-golden; do
      printf '#!/bin/sh\necho %s\n' "$bin" > "$CARGO_TARGET_DIR/$profile/$bin"
      chmod +x "$CARGO_TARGET_DIR/$profile/$bin"
    done
    ;;
  *) exec "$tool" "$@" ;;
esac
"#,
            log = dir.join("cargo.log").display(),
            fail = if fail { "exit 101" } else { ":" },
        );
        std::fs::write(&script, body).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        vec![script.to_str().unwrap().to_owned()]
    }

    fn cargo_calls(world: &World) -> Vec<String> {
        std::fs::read_to_string(world.root.path().join("cargo.log"))
            .map(|s| s.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    #[test]
    fn rustc_info_runs_behind_the_wrapper() {
        let world = World::new();
        let wrapper = fake_wrapper(world.root.path(), false);
        let (verbose, host) = rustc_info(&wrapper).unwrap();
        assert!(verbose.starts_with("rustc 1.95.0 (fake)\n"), "{verbose}");
        assert_eq!(host, TARGET);

        let err = rustc_info(&["/nonexistent/wrapper".to_owned()]).unwrap_err();
        assert!(matches!(err, BuildError::Toolchain(_)), "{err}");
    }

    #[test]
    fn build_then_cache_then_force() {
        let world = World::new();
        let mut request = world.request(MODS);
        request.wrapper = fake_wrapper(world.root.path(), false);
        request.jobs = 3;

        let first = build(&request).unwrap();
        assert!(!first.cached);
        assert_eq!(first.dir, request.builds_dir.join(&first.build_id[..32]));
        assert_eq!(first.game, first.dir.join("bin/pwc-game"));
        assert_eq!(
            std::fs::read_to_string(&first.game).unwrap(),
            "#!/bin/sh\necho pwc-game\n"
        );
        assert!(first.golden.is_file());
        let calls = cargo_calls(&world);
        assert_eq!(calls.len(), 1);
        let manifest = first.dir.join("Cargo.toml");
        assert_eq!(
            calls[0],
            format!(
                "cargo build --manifest-path {} --bins -p pwc-game-instance --release -j 3 | cwd={}",
                manifest.display(),
                first.dir.display()
            )
        );

        // The generated files are on disk; build.toml records the finished build.
        assert!(first.dir.join("mods/pwc_inventory/Cargo.toml").is_file());
        assert_eq!(
            std::fs::read_to_string(first.dir.join("Cargo.lock")).unwrap(),
            "# seed lock\nversion = 4\n"
        );
        let meta: toml::Table = std::fs::read_to_string(first.dir.join("build.toml"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(meta["build-id"].as_str(), Some(first.build_id.as_str()));
        assert_eq!(
            meta["environment"].as_str(),
            Some(request.lock.environment.as_str())
        );
        assert_eq!(meta["profile"].as_str(), Some("release"));
        assert_eq!(meta["target"].as_str(), Some(TARGET));
        assert!(meta["finished"].as_str().unwrap().ends_with('Z'));
        assert_eq!(meta["outputs"]["game"].as_str(), Some("bin/pwc-game"));
        assert_eq!(meta["pwc"]["dirty-digest"].as_str(), Some("clean"));
        assert_eq!(meta["package"].as_array().unwrap().len(), MODS.len());

        // Second build: answered by the cache, Cargo not invoked.
        let second = build(&request).unwrap();
        assert!(second.cached);
        assert_eq!(second.game, first.game);
        assert_eq!(cargo_calls(&world).len(), 1);

        // --force rebuilds into the same directory.
        request.force = true;
        let third = build(&request).unwrap();
        assert!(!third.cached);
        assert_eq!(third.dir, first.dir);
        assert_eq!(cargo_calls(&world).len(), 2);

        // A missing executable invalidates the cache entry.
        request.force = false;
        std::fs::remove_file(&first.golden).unwrap();
        assert!(!build(&request).unwrap().cached);
        assert_eq!(cargo_calls(&world).len(), 3);

        // Another profile is another build.
        request.profile = BuildProfile::Dev;
        let dev = build(&request).unwrap();
        assert_ne!(dev.build_id, first.build_id);
        assert!(cargo_calls(&world)[3].contains("-p pwc-game-instance -j 3 |"));

        clean(&request.builds_dir, &request.target_dir).unwrap();
        assert!(!request.builds_dir.exists() && !request.target_dir.exists());
        clean(&request.builds_dir, &request.target_dir).unwrap();
    }

    #[test]
    fn failed_cargo_is_reported_and_not_cached() {
        let world = World::new();
        let mut request = world.request(MODS);
        request.wrapper = fake_wrapper(world.root.path(), true);
        let err = build(&request).unwrap_err();
        assert!(
            matches!(&err, BuildError::Cargo(m) if m.contains("generated workspace")),
            "{err}"
        );
        let (rustc, target) = rustc_info(&request.wrapper).unwrap();
        let prepared = prepare(&request, &rustc, &target).unwrap();
        assert!(!prepared.cached);
        let meta: toml::Table = std::fs::read_to_string(prepared.dir.join("build.toml"))
            .unwrap()
            .parse()
            .unwrap();
        assert!(!meta.contains_key("finished"));
    }

    #[test]
    fn prepare_rewrites_only_changed_files_and_drops_stale_crates() {
        let world = World::new();
        let request = world.request(MODS);
        let prepared = prepare(&request, RUSTC, TARGET).unwrap();
        let hotbar = prepared.dir.join("mods/pwc_hotbar/Cargo.toml");
        let stamp = std::fs::metadata(&hotbar).unwrap().modified().unwrap();
        let stale = prepared.dir.join("mods/pwc_stale/Cargo.toml");
        write(&stale, "stale");
        std::fs::write(prepared.dir.join("Cargo.lock"), "# cargo updated this\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));

        prepare(&request, RUSTC, TARGET).unwrap();
        assert_eq!(
            std::fs::metadata(&hotbar).unwrap().modified().unwrap(),
            stamp
        );
        assert!(!stale.parent().unwrap().exists());
        assert_eq!(
            std::fs::read_to_string(prepared.dir.join("Cargo.lock")).unwrap(),
            "# cargo updated this\n"
        );
    }

    #[test]
    fn missing_store_sources_fail_before_cargo() {
        let world = World::new();
        let request = world.request(MODS);
        std::fs::remove_file(world.store_dir(&MODS[1]).join("src/lib.rs")).unwrap();
        let err = prepare(&request, RUSTC, TARGET).unwrap_err();
        assert!(err.to_string().contains("has no src/lib.rs"), "{err}");
    }
}

// ---------------------------------------------------------------------------------------------
// Metadata

#[test]
fn timestamps_are_rfc3339_utc() {
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    assert_eq!(civil_from_days(20_729), (2026, 10, 3));
    let t = std::time::UNIX_EPOCH
        + std::time::Duration::from_secs(20_729 * 86_400 + 10 * 3600 + 7 * 60 + 42);
    assert_eq!(rfc3339(t), "2026-10-03T10:07:42Z");
}

/// End-to-end against the real game: `cargo test -p pwc-builder -- --ignored real_game`.
/// Uses `PWC_SOURCE` (default: the sibling `project_watt_cubed` checkout), which must provide
/// `crates/pwc-mod-api` and `examples/mods/hello-hud`; compiles the full game (minutes, about
/// 3 GB). The cache goes to `PWC_TEST_CACHE` (default: `target/pwc-builder-e2e` of this
/// workspace) so reruns are incremental.
#[test]
#[ignore = "compiles the real game"]
fn real_game_builds_with_an_example_mod() {
    let pwc = std::env::var_os("PWC_SOURCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../project_watt_cubed")
        });
    let pwc = pwc.canonicalize().expect("PWC source");
    let example = pwc.join("examples/mods/hello-hud");
    let read = |p: &Path| -> toml::Table { std::fs::read_to_string(p).unwrap().parse().unwrap() };
    // The builder does not look at licences; keep the example usable whatever it declares.
    let mut text = read(&example.join("mod.toml"));
    text["package"]
        .as_table_mut()
        .unwrap()
        .insert("license".into(), "AGPL-3.0-or-later".into());
    let manifest = ModManifest::parse(&toml::to_string(&text).unwrap()).expect("example manifest");
    let version =
        |t: &toml::Table| Version::parse(t["package"]["version"].as_str().unwrap()).unwrap();
    let lock = Lockfile::new(
        LockedPwc {
            version: version(&read(&pwc.join("Cargo.toml"))),
            api: version(&read(&pwc.join("crates/pwc-mod-api/Cargo.toml"))),
            source: pwc.clone(),
            revision: "test".to_owned(),
            dirty: true,
        },
        vec![LockedPackage {
            id: manifest.id().clone(),
            version: manifest.version().clone(),
            kind: manifest.kind(),
            hash: hash_of("hello-hud"),
            source: PackageSource::Path(example.clone()),
            dependencies: Vec::new(),
        }],
    );
    let cache = std::env::var_os("PWC_TEST_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/pwc-builder-e2e")
        });
    let mut request = BuildRequest {
        lock,
        packages: BTreeMap::from([(manifest.id().clone(), (example, manifest))]),
        pwc_dirty_digest: "e2e".to_owned(),
        pwc_source: pwc,
        profile: BuildProfile::Dev,
        builds_dir: cache.join("builds"),
        target_dir: cache.join("target"),
        wrapper: Vec::new(),
        jobs: 0,
        // The digest above is a constant, so always compile (incrementally) on the first call.
        force: true,
    };
    let output = build(&request).expect("build");
    assert!(!output.cached && output.game.is_file() && output.golden.is_file());
    request.force = false;
    assert!(build(&request).unwrap().cached);
}
