//! The PWC game checkout the instance builds against.

use std::path::{Path, PathBuf};
use std::process::Command;

use pwc_manifest::{LockedPwc, Version};
use sha2::{Digest, Sha256};

use crate::InstanceError;
use crate::fsutil::io;

/// A PWC source checkout: its version, mod API version and git state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PwcSource {
    /// Root directory (contains the runtime `Cargo.toml` and `crates/pwc-mod-api`).
    pub dir: PathBuf,
    /// `[package] version` of the root `Cargo.toml`.
    pub version: Version,
    /// `[package] version` of `crates/pwc-mod-api/Cargo.toml`.
    pub api: Version,
    /// `git rev-parse HEAD`, or `unknown`.
    pub revision: String,
    /// `git status --porcelain` non-empty.
    pub dirty: bool,
}

/// The revision recorded outside git.
pub const UNKNOWN_REVISION: &str = "unknown";

impl PwcSource {
    /// Inspect a checkout (errors if it is not a PWC source providing `crates/pwc-mod-api`).
    ///
    /// `dir` is canonicalised. Versions may be inherited with `version.workspace = true` from the
    /// root manifest's `[workspace.package]`.
    pub fn open(dir: &Path) -> Result<Self, InstanceError> {
        let dir = std::fs::canonicalize(dir)
            .map_err(|e| InstanceError::PwcSource(format!("{}: {e}", dir.display())))?;
        let root_manifest = dir.join("Cargo.toml");
        let api_manifest = dir.join("crates/pwc-mod-api/Cargo.toml");
        if !root_manifest.is_file() {
            return Err(InstanceError::PwcSource(format!(
                "{} is not a PWC source checkout (no Cargo.toml)",
                dir.display()
            )));
        }
        if !api_manifest.is_file() {
            return Err(InstanceError::PwcSource(format!(
                "{} does not provide the mod API (no crates/pwc-mod-api/Cargo.toml); \
                 this PWC version predates mod packages",
                dir.display()
            )));
        }
        let root = read_toml(&root_manifest)?;
        let version = package_version(&root, &root, &root_manifest)?;
        let api = package_version(&read_toml(&api_manifest)?, &root, &api_manifest)?;
        let (revision, dirty) = match git(&dir, &["rev-parse", "HEAD"]) {
            Some(out) => {
                let revision = String::from_utf8_lossy(&out).trim().to_string();
                let dirty = git(&dir, &["status", "--porcelain"])
                    .is_some_and(|s| !s.iter().all(u8::is_ascii_whitespace));
                (revision, dirty)
            }
            None => (UNKNOWN_REVISION.to_string(), false),
        };
        Ok(Self {
            dir,
            version,
            api,
            revision,
            dirty,
        })
    }

    /// As recorded in a lockfile.
    pub fn locked(&self) -> LockedPwc {
        LockedPwc {
            version: self.version.clone(),
            api: self.api.clone(),
            source: self.dir.clone(),
            revision: self.revision.clone(),
            dirty: self.dirty,
        }
    }

    /// The `dirty-digest` of docs/spec/build.md, computed now (not at [`PwcSource::open`] time).
    ///
    /// - A clean git checkout: `clean`.
    /// - A dirty one: `sha256:<hex>` over `git diff HEAD --binary` followed by every untracked,
    ///   non-ignored file (sorted by path) with its contents:
    ///   `"pwc-dirty 1\n" diff "\n" ("untracked " path "\n" len "\n" contents "\n")*`.
    /// - Outside git: `sha256:<hex>` over every file of the tree (sorted by path, skipping
    ///   `target/` and hidden entries) with its contents, so a changed tree never reuses a build.
    ///
    /// Path dependencies outside the checkout (the sibling `../voxel-engine`) are build inputs too:
    /// when there are any, the digest is `sha256:<hex>` over
    /// `"pwc-sources 1\n" ("source " <path> " " <revision> " " <state> "\n")*` for the checkout
    /// and every such dependency (found recursively through their `Cargo.toml`s, sorted by
    /// canonical path), where `<state>` is the per-tree digest above. A commit or an edit in the
    /// engine therefore changes the Build ID.
    pub fn dirty_digest(&self) -> Result<String, InstanceError> {
        let external = external_path_dependencies(&self.dir)?;
        if external.is_empty() {
            return tree_state(&self.dir);
        }
        let mut hasher = Sha256::new();
        hasher.update(b"pwc-sources 1\n");
        for dir in std::iter::once(self.dir.clone()).chain(external) {
            let revision = git(&dir, &["rev-parse", "HEAD"])
                .map(|out| String::from_utf8_lossy(&out).trim().to_string())
                .unwrap_or_else(|| UNKNOWN_REVISION.to_string());
            let state = tree_state(&dir)?;
            hasher.update(format!("source {} {revision} {state}\n", dir.display()).as_bytes());
        }
        Ok(format!("sha256:{}", hex(&hasher.finalize())))
    }
}

/// Canonical directories of every path dependency reachable from `root`'s `Cargo.toml` that lies
/// outside `root`, following their own manifests, sorted.
fn external_path_dependencies(root: &Path) -> Result<Vec<PathBuf>, InstanceError> {
    let mut found = std::collections::BTreeSet::new();
    let mut queue = vec![root.to_path_buf()];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(dir) = queue.pop() {
        if !seen.insert(dir.clone()) {
            continue;
        }
        let manifest = dir.join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        for dep in manifest_path_dependencies(&read_toml(&manifest)?) {
            let Ok(dep) = std::fs::canonicalize(dir.join(dep)) else {
                continue;
            };
            if !dep.starts_with(root) {
                // An external dependency: hash its whole repository (or tree).
                let top = git(&dep, &["rev-parse", "--show-toplevel"])
                    .map(|out| PathBuf::from(String::from_utf8_lossy(&out).trim()))
                    .and_then(|p| std::fs::canonicalize(p).ok())
                    .filter(|top| !root.starts_with(top))
                    .unwrap_or_else(|| dep.clone());
                found.insert(top);
            }
            queue.push(dep);
        }
    }
    Ok(found.into_iter().collect())
}

/// `path` values of a manifest's dependency tables (`[dependencies]`, `[build-dependencies]`,
/// `[workspace.dependencies]`, `[target.*.dependencies]`, `[patch.*]`).
fn manifest_path_dependencies(manifest: &toml::Table) -> Vec<String> {
    fn paths(table: Option<&toml::Value>, out: &mut Vec<String>) {
        let Some(table) = table.and_then(toml::Value::as_table) else {
            return;
        };
        for dep in table.values() {
            if let Some(path) = dep.get("path").and_then(toml::Value::as_str) {
                out.push(path.to_string());
            }
        }
    }
    let mut out = Vec::new();
    paths(manifest.get("dependencies"), &mut out);
    paths(manifest.get("build-dependencies"), &mut out);
    paths(
        manifest
            .get("workspace")
            .and_then(|w| w.get("dependencies")),
        &mut out,
    );
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        for target in targets.values() {
            paths(target.get("dependencies"), &mut out);
            paths(target.get("build-dependencies"), &mut out);
        }
    }
    if let Some(patches) = manifest.get("patch").and_then(toml::Value::as_table) {
        for patch in patches.values() {
            paths(Some(patch), &mut out);
        }
    }
    out
}

/// The per-tree state: `clean`, or the digest of a dirty git checkout, or of a tree outside git.
fn tree_state(dir: &Path) -> Result<String, InstanceError> {
    if git(dir, &["rev-parse", "HEAD"]).is_none() {
        return tree_digest(dir);
    }
    let status = git(dir, &["status", "--porcelain"]).ok_or_else(|| {
        InstanceError::PwcSource(format!("`git status` failed in {}", dir.display()))
    })?;
    if status.iter().all(u8::is_ascii_whitespace) {
        return Ok("clean".to_string());
    }
    let diff = git(
        dir,
        &[
            "diff",
            "HEAD",
            "--binary",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
        ],
    )
    .ok_or_else(|| InstanceError::PwcSource(format!("`git diff` failed in {}", dir.display())))?;
    let untracked =
        git(dir, &["ls-files", "--others", "--exclude-standard", "-z"]).ok_or_else(|| {
            InstanceError::PwcSource(format!("`git ls-files` failed in {}", dir.display()))
        })?;
    let mut paths: Vec<&[u8]> = untracked
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .collect();
    paths.sort();
    let mut hasher = Sha256::new();
    hasher.update(b"pwc-dirty 1\n");
    hasher.update(&diff);
    hasher.update(b"\n");
    for path in paths {
        let rel = String::from_utf8_lossy(path);
        let contents = file_contents(&dir.join(rel.as_ref()))?;
        hasher.update(b"untracked ");
        hasher.update(path);
        hasher.update(format!("\n{}\n", contents.len()).as_bytes());
        hasher.update(&contents);
        hasher.update(b"\n");
    }
    Ok(format!("sha256:{}", hex(&hasher.finalize())))
}

/// Run git in `dir`; stdout on success, `None` if git is missing or fails (not a repository).
fn git(dir: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

fn read_toml(path: &Path) -> Result<toml::Table, InstanceError> {
    let text = std::fs::read_to_string(path).map_err(|e| io(path, e))?;
    text.parse::<toml::Table>()
        .map_err(|e| InstanceError::PwcSource(format!("{}: {}", path.display(), e.message())))
}

/// `[package] version` of `manifest`, following `version.workspace = true` to `root`.
fn package_version(
    manifest: &toml::Table,
    root: &toml::Table,
    path: &Path,
) -> Result<Version, InstanceError> {
    let missing =
        || InstanceError::PwcSource(format!("{}: no `[package] version`", path.display()));
    let value = manifest
        .get("package")
        .and_then(|p| p.get("version"))
        .ok_or_else(missing)?;
    let text = match value {
        toml::Value::String(s) => s.as_str(),
        toml::Value::Table(t)
            if t.get("workspace").and_then(toml::Value::as_bool) == Some(true) =>
        {
            root.get("workspace")
                .and_then(|w| w.get("package"))
                .and_then(|p| p.get("version"))
                .and_then(toml::Value::as_str)
                .ok_or_else(missing)?
        }
        _ => return Err(missing()),
    };
    Version::parse(text).map_err(|e| {
        InstanceError::PwcSource(format!("{}: invalid version {text:?}: {e}", path.display()))
    })
}

/// A file's bytes, or a symlink's target.
fn file_contents(path: &Path) -> Result<Vec<u8>, InstanceError> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| io(path, e))?;
    if meta.file_type().is_symlink() {
        let target = std::fs::read_link(path).map_err(|e| io(path, e))?;
        return Ok(target.to_string_lossy().into_owned().into_bytes());
    }
    if !meta.is_file() {
        return Ok(Vec::new());
    }
    std::fs::read(path).map_err(|e| io(path, e))
}

/// Digest of a tree outside git (see [`PwcSource::dirty_digest`]).
fn tree_digest(dir: &Path) -> Result<String, InstanceError> {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) -> Result<(), InstanceError> {
        for item in std::fs::read_dir(dir).map_err(|e| io(dir, e))? {
            let item = item.map_err(|e| io(dir, e))?;
            let name = item.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || (rel.is_empty() && name == "target") {
                continue;
            }
            let path = if rel.is_empty() {
                name
            } else {
                format!("{rel}/{name}")
            };
            let kind = item.file_type().map_err(|e| io(&item.path(), e))?;
            if kind.is_dir() {
                walk(&item.path(), &path, out)?;
            } else {
                out.push(path);
            }
        }
        Ok(())
    }
    let mut paths = Vec::new();
    walk(dir, "", &mut paths)?;
    paths.sort();
    let mut hasher = Sha256::new();
    hasher.update(b"pwc-tree 1\n");
    for path in paths {
        let contents = file_contents(&dir.join(&path))?;
        hasher.update(format!("file {path}\n{}\n", contents.len()).as_bytes());
        hasher.update(&contents);
        hasher.update(b"\n");
    }
    Ok(format!("sha256:{}", hex(&hasher.finalize())))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::fake_pwc;

    fn git_available() -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    fn run_git(dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn opens_a_plain_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = fake_pwc(tmp.path(), "2.1.0", "1.0.0");
        let src = PwcSource::open(&dir).unwrap();
        assert_eq!(src.dir, std::fs::canonicalize(&dir).unwrap());
        assert_eq!(src.version, Version::new(2, 1, 0));
        assert_eq!(src.api, Version::new(1, 0, 0));
        assert_eq!(src.revision, "unknown");
        assert!(!src.dirty);
        let locked = src.locked();
        assert_eq!(locked.version, src.version);
        assert_eq!(locked.source, src.dir);

        // The tree digest changes with the contents, ignoring target/ and hidden entries.
        let d1 = src.dirty_digest().unwrap();
        assert!(d1.starts_with("sha256:") && d1.len() == 71, "{d1}");
        std::fs::create_dir_all(dir.join("target/debug")).unwrap();
        std::fs::write(dir.join("target/debug/junk"), "x").unwrap();
        std::fs::write(dir.join(".hidden"), "x").unwrap();
        assert_eq!(src.dirty_digest().unwrap(), d1);
        std::fs::write(dir.join("src/lib.rs"), "// changed").unwrap();
        assert_ne!(src.dirty_digest().unwrap(), d1);
    }

    #[test]
    fn workspace_inherited_versions() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = fake_pwc(tmp.path(), "0.0.0", "0.0.0");
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"project_watt_cubed\"\nversion.workspace = true\n\n\
             [workspace]\nmembers = [\"crates/pwc-mod-api\"]\n[workspace.package]\nversion = \"3.0.0-rc.1\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("crates/pwc-mod-api/Cargo.toml"),
            "[package]\nname = \"pwc-mod-api\"\nversion = { workspace = true }\n",
        )
        .unwrap();
        let src = PwcSource::open(&dir).unwrap();
        assert_eq!(src.version, Version::parse("3.0.0-rc.1").unwrap());
        assert_eq!(src.api, Version::parse("3.0.0-rc.1").unwrap());
    }

    /// The sibling engine is a build input: editing it changes the digest, editing an unrelated
    /// sibling does not.
    #[test]
    fn external_path_dependencies_feed_the_digest() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = fake_pwc(tmp.path(), "2.1.0", "1.0.0");
        let engine = tmp.path().join("engine");
        std::fs::create_dir_all(engine.join("src")).unwrap();
        std::fs::write(
            engine.join("Cargo.toml"),
            "[package]\nname = \"engine\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        std::fs::write(engine.join("src/lib.rs"), "pub fn a() {}\n").unwrap();
        std::fs::create_dir_all(tmp.path().join("other")).unwrap();
        let manifest = dir.join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest).unwrap();
        std::fs::write(
            &manifest,
            format!("{text}\n[dependencies]\nengine = {{ path = \"../engine\" }}\n"),
        )
        .unwrap();
        let source = PwcSource::open(&dir).unwrap();
        assert_eq!(
            external_path_dependencies(&source.dir).unwrap(),
            [std::fs::canonicalize(&engine).unwrap()]
        );
        let before = source.dirty_digest().unwrap();
        std::fs::write(tmp.path().join("other/x"), "unrelated").unwrap();
        assert_eq!(source.dirty_digest().unwrap(), before);
        std::fs::write(engine.join("src/lib.rs"), "pub fn b() {}\n").unwrap();
        assert_ne!(
            source.dirty_digest().unwrap(),
            before,
            "an engine edit must change the build"
        );
    }

    #[test]
    fn rejects_non_pwc_trees() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(PwcSource::open(&tmp.path().join("missing")).is_err());
        let dir = fake_pwc(tmp.path(), "1.0.0", "1.0.0");
        std::fs::remove_dir_all(dir.join("crates")).unwrap();
        let e = PwcSource::open(&dir).unwrap_err().to_string();
        assert!(e.contains("pwc-mod-api"), "{e}");

        let dir = fake_pwc(&tmp.path().join("b"), "1.0.0", "1.0.0");
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        assert!(
            PwcSource::open(&dir)
                .unwrap_err()
                .to_string()
                .contains("version")
        );
        std::fs::write(dir.join("Cargo.toml"), "[package]\nversion = \"one\"\n").unwrap();
        assert!(
            PwcSource::open(&dir)
                .unwrap_err()
                .to_string()
                .contains("invalid version")
        );
        std::fs::write(dir.join("Cargo.toml"), "not toml [").unwrap();
        assert!(PwcSource::open(&dir).is_err());
    }

    #[test]
    fn git_state() {
        if !git_available() {
            eprintln!("git not available; skipping");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let dir = fake_pwc(tmp.path(), "2.0.0", "1.0.0");
        run_git(&dir, &["init", "-q"]);
        std::fs::write(dir.join(".gitignore"), "target/\n").unwrap();
        run_git(&dir, &["add", "-A"]);
        run_git(&dir, &["commit", "-q", "-m", "init"]);

        let src = PwcSource::open(&dir).unwrap();
        assert_eq!(src.revision.len(), 40, "{}", src.revision);
        assert!(!src.dirty);
        assert_eq!(src.dirty_digest().unwrap(), "clean");

        // Ignored files keep it clean.
        std::fs::create_dir_all(dir.join("target")).unwrap();
        std::fs::write(dir.join("target/out"), "x").unwrap();
        assert!(!PwcSource::open(&dir).unwrap().dirty);
        assert_eq!(src.dirty_digest().unwrap(), "clean");

        // A modification.
        std::fs::write(dir.join("src/lib.rs"), "// one").unwrap();
        let src = PwcSource::open(&dir).unwrap();
        assert!(src.dirty);
        let d1 = src.dirty_digest().unwrap();
        assert!(d1.starts_with("sha256:"), "{d1}");
        assert_eq!(src.dirty_digest().unwrap(), d1, "deterministic");
        std::fs::write(dir.join("src/lib.rs"), "// two").unwrap();
        let d2 = src.dirty_digest().unwrap();
        assert_ne!(d1, d2);

        // An untracked file's contents count.
        std::fs::write(dir.join("new.rs"), "a").unwrap();
        let d3 = src.dirty_digest().unwrap();
        assert_ne!(d2, d3);
        std::fs::write(dir.join("new.rs"), "b").unwrap();
        assert_ne!(src.dirty_digest().unwrap(), d3);

        // Back to clean.
        std::fs::remove_file(dir.join("new.rs")).unwrap();
        run_git(&dir, &["checkout", "-q", "--", "src/lib.rs"]);
        assert_eq!(src.dirty_digest().unwrap(), "clean");
        assert!(!PwcSource::open(&dir).unwrap().dirty);
    }
}
