//! What the builder reads from the PWC source checkout: the runtime and API crate versions, the
//! root `Cargo.toml` tables that are copied into the generated workspace, and the `Cargo.lock` seed.

use std::path::Path;

use pwc_manifest::{Lockfile, Version};
use toml::{Table, Value};

use crate::{BuildError, io_err};

/// Package name of the runtime crate at the PWC source root.
pub(crate) const RUNTIME_PACKAGE: &str = "project_watt_cubed";

/// Package name of the mod API crate.
pub(crate) const API_PACKAGE: &str = "pwc-mod-api";

/// Location of the mod API crate inside the PWC source.
pub(crate) const API_DIR: [&str; 2] = ["crates", "pwc-mod-api"];

/// The parts of a PWC source the builder needs.
#[derive(Clone, Debug)]
pub(crate) struct PwcSourceInfo {
    /// The root `Cargo.toml`, parsed.
    pub root: Table,
    /// The runtime package version.
    pub version: Version,
    /// The `pwc-mod-api` package version.
    pub api: Version,
    /// The root `Cargo.lock`, if the source has one.
    pub cargo_lock: Option<String>,
}

impl PwcSourceInfo {
    /// Read `dir/Cargo.toml`, `dir/crates/pwc-mod-api/Cargo.toml` and `dir/Cargo.lock`.
    pub fn read(dir: &Path) -> Result<Self, BuildError> {
        let root_path = dir.join("Cargo.toml");
        let root = read_toml(&root_path)?;
        check_name(&root, RUNTIME_PACKAGE, &root_path)?;
        let version = package_version(&root, &root, &root_path)?;

        let api_path = API_DIR
            .iter()
            .fold(dir.to_path_buf(), |p, c| p.join(c))
            .join("Cargo.toml");
        if !api_path.is_file() {
            return Err(BuildError::Source(format!(
                "{} has no `{}` crate (expected {}); this PWC checkout predates the mod API",
                dir.display(),
                API_PACKAGE,
                api_path.display()
            )));
        }
        let api_toml = read_toml(&api_path)?;
        check_name(&api_toml, API_PACKAGE, &api_path)?;
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
                "the checkout provides {API_PACKAGE} {} but the lock pins {}; run `pwc lock`",
                self.api, lock.pwc.api
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
