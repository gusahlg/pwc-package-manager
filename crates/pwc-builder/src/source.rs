//! What the builder reads from the PWC source checkout: the runtime and API crate versions, the
//! root `Cargo.toml` tables that are copied into the generated workspace, and the `Cargo.lock` seed.

use std::path::Path;

use pwc_manifest::{Lockfile, Version};
use toml::{Table, Value};

use crate::{BuildError, io_err};

/// Package name of the runtime crate at the PWC source root.
pub(crate) const RUNTIME_PACKAGE: &str = "project_watt_cubed";

/// Package name of the module API crate (since module API 3.4.0).
pub(crate) const API_PACKAGE: &str = "pwc-module-api";

/// Location of the module API crate inside the PWC source.
pub(crate) const API_DIR: [&str; 2] = ["crates", "pwc-module-api"];

/// Package name of the module API crate before the rename (PWC 0.7.0 and earlier); after the
/// rename a crate of this name is a shim re-exporting [`API_PACKAGE`].
pub(crate) const OLD_API_PACKAGE: &str = "pwc-mod-api";

/// Location of the old-named API crate inside the PWC source.
pub(crate) const OLD_API_DIR: [&str; 2] = ["crates", "pwc-mod-api"];

/// The parts of a PWC source the builder needs.
#[derive(Clone, Debug)]
pub(crate) struct PwcSourceInfo {
    /// The root `Cargo.toml`, parsed.
    pub root: Table,
    /// The runtime package version.
    pub version: Version,
    /// The version of the real API crate (`pwc-module-api`, or `pwc-mod-api` before the rename).
    pub api: Version,
    /// The package name of the real API crate.
    pub api_package: &'static str,
    /// The directory of the real API crate.
    pub api_dir: std::path::PathBuf,
    /// Whether the source is from after the rename (it has `crates/pwc-module-api`), so generated
    /// code uses the `Module*` names.
    pub renamed: bool,
    /// The compatibility shim crate `pwc-mod-api`, when the source is renamed and still has it.
    pub shim_dir: Option<std::path::PathBuf>,
    /// The root `Cargo.lock`, if the source has one.
    pub cargo_lock: Option<String>,
}

impl PwcSourceInfo {
    /// Read `dir/Cargo.toml`, the API crate's `Cargo.toml` and `dir/Cargo.lock`. The API crate is
    /// `crates/pwc-module-api` when it exists; a source from before the rename has only
    /// `crates/pwc-mod-api`, which is then the real API crate.
    pub fn read(dir: &Path) -> Result<Self, BuildError> {
        let root_path = dir.join("Cargo.toml");
        let root = read_toml(&root_path)?;
        check_name(&root, RUNTIME_PACKAGE, &root_path)?;
        let version = package_version(&root, &root, &root_path)?;

        let join = |parts: &[&str; 2]| parts.iter().fold(dir.to_path_buf(), |p, c| p.join(c));
        let new_dir = join(&API_DIR);
        let old_dir = join(&OLD_API_DIR);
        let (api_package, api_dir, renamed) = if new_dir.join("Cargo.toml").is_file() {
            (API_PACKAGE, new_dir, true)
        } else if old_dir.join("Cargo.toml").is_file() {
            (OLD_API_PACKAGE, old_dir.clone(), false)
        } else {
            return Err(BuildError::Source(format!(
                "{} has no `{}` crate (expected {}); this PWC checkout predates the module API",
                dir.display(),
                API_PACKAGE,
                new_dir.join("Cargo.toml").display()
            )));
        };
        let shim_dir = (renamed && old_dir.join("Cargo.toml").is_file()).then_some(old_dir);
        let api_path = api_dir.join("Cargo.toml");
        let api_toml = read_toml(&api_path)?;
        check_name(&api_toml, api_package, &api_path)?;
        let api = package_version(&api_toml, &root, &api_path)?;

        let lock_path = dir.join("Cargo.lock");
        let cargo_lock = match std::fs::read_to_string(&lock_path) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(io_err(&lock_path)(e)),
        };
        Ok(Self {
            root,
            version,
            api,
            api_package,
            api_dir,
            renamed,
            shim_dir,
            cargo_lock,
        })
    }

    /// The source must still be what the lock pinned (version and API version).
    pub fn check_against(&self, lock: &Lockfile) -> Result<(), BuildError> {
        if self.version != lock.pwc.version {
            return Err(BuildError::Source(format!(
                "the checkout is PWC {} but the lock pins {}; run `pwc lock`",
                self.version, lock.pwc.version
            )));
        }
        if self.api != lock.pwc.api {
            return Err(BuildError::Source(format!(
                "the checkout provides {} {} but the lock pins {}; run `pwc lock`",
                self.api_package, self.api, lock.pwc.api
            )));
        }
        Ok(())
    }

    /// `[workspace] resolver` of the PWC root (so dependency features unify as in vanilla).
    pub fn resolver(&self) -> &str {
        self.root
            .get("workspace")
            .and_then(|w| w.get("resolver"))
            .and_then(Value::as_str)
            .unwrap_or("2")
    }
}

fn read_toml(path: &Path) -> Result<Table, BuildError> {
    let text = std::fs::read_to_string(path).map_err(io_err(path))?;
    text.parse::<Table>()
        .map_err(|e| BuildError::Source(format!("{}: {e}", path.display())))
}

fn check_name(manifest: &Table, expected: &str, path: &Path) -> Result<(), BuildError> {
    let name = manifest
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(Value::as_str);
    if name == Some(expected) {
        Ok(())
    } else {
        Err(BuildError::Source(format!(
            "{} is not the `{expected}` package (found {})",
            path.display(),
            name.map_or_else(|| "no [package] name".to_owned(), |n| format!("`{n}`"))
        )))
    }
}

/// `[package] version`, following `version.workspace = true` to the PWC root's
/// `[workspace.package] version`.
fn package_version(
    manifest: &Table,
    workspace_root: &Table,
    path: &Path,
) -> Result<Version, BuildError> {
    let missing = || BuildError::Source(format!("{} has no [package] version", path.display()));
    let text = match manifest.get("package").and_then(|p| p.get("version")) {
        Some(Value::String(s)) => s.as_str(),
        Some(Value::Table(t)) if t.get("workspace").and_then(Value::as_bool) == Some(true) => {
            workspace_root
                .get("workspace")
                .and_then(|w| w.get("package"))
                .and_then(|p| p.get("version"))
                .and_then(Value::as_str)
                .ok_or_else(missing)?
        }
        _ => return Err(missing()),
    };
    Version::parse(text)
        .map_err(|e| BuildError::Source(format!("{}: version {text:?}: {e}", path.display())))
}
