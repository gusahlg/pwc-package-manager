//! Fixtures shared by this crate's tests: a fake PWC source tree and small valid packages.

use std::path::{Path, PathBuf};

/// A minimal PWC-shaped tree at `<root>/pwc` (not a git repository).
pub(crate) fn fake_pwc(root: &Path, version: &str, api: &str) -> PathBuf {
    let dir = root.join("pwc");
    std::fs::create_dir_all(dir.join("crates/pwc-mod-api/src")).unwrap();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        format!("[package]\nname = \"project_watt_cubed\"\nversion = \"{version}\"\nedition = \"2024\"\n"),
    )
    .unwrap();
    std::fs::write(
        dir.join("crates/pwc-mod-api/Cargo.toml"),
        format!("[package]\nname = \"pwc-mod-api\"\nversion = \"{api}\"\nedition = \"2024\"\n"),
    )
    .unwrap();
    std::fs::write(dir.join("src/lib.rs"), "").unwrap();
    dir
}

/// A package source tree to write.
#[derive(Clone, Debug)]
pub(crate) struct Pkg<'a> {
    pub id: &'a str,
    pub version: &'a str,
    pub kind: &'a str,
    pub api: &'a str,
    pub deps: &'a [(&'a str, &'a str)],
    pub conflicts: &'a [(&'a str, &'a str)],
    pub code: &'a str,
}

impl<'a> Pkg<'a> {
    /// A `kind = "mod"` package for `pwc-api = "^1.0"` without dependencies.
    pub fn new(id: &'a str, version: &'a str) -> Self {
        Self {
            id,
            version,
            kind: "mod",
            api: "^1.0",
            deps: &[],
            conflicts: &[],
            code: "pub fn register() {}\n",
        }
    }

    pub fn deps(mut self, deps: &'a [(&'a str, &'a str)]) -> Self {
        self.deps = deps;
        self
    }

    pub fn bundle(mut self) -> Self {
        self.kind = "bundle";
        self
    }

    pub fn api(mut self, api: &'a str) -> Self {
        self.api = api;
        self
    }

    pub fn code(mut self, code: &'a str) -> Self {
        self.code = code;
        self
    }

    pub fn mod_toml(&self) -> String {
        let mut text = format!(
            "format = 1\n\n[package]\nid = \"{}\"\nname = \"Test {}\"\nversion = \"{}\"\n\
             description = \"A package for the pwc-instance tests.\"\nauthors = [\"Tester\"]\n\
             license = \"AGPL-3.0-or-later\"\nlicense-files = [\"LICENSE\"]\nkind = \"{}\"\n",
            self.id, self.id, self.version, self.kind
        );
        if self.kind != "bundle" {
            text.push_str(&format!("pwc-api = \"{}\"\n", self.api));
        }
        if !self.deps.is_empty() {
            text.push_str("\n[dependencies]\n");
            for (id, req) in self.deps {
                text.push_str(&format!("\"{id}\" = \"{req}\"\n"));
            }
        }
        if !self.conflicts.is_empty() {
            text.push_str("\n[conflicts]\n");
            for (id, req) in self.conflicts {
                text.push_str(&format!("\"{id}\" = \"{req}\"\n"));
            }
        }
        text
    }

    /// Write the source tree to `dir` (created; existing files overwritten).
    pub fn write(&self, dir: &Path) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("mod.toml"), self.mod_toml()).unwrap();
        std::fs::write(dir.join("README.md"), format!("# {}\n", self.id)).unwrap();
        std::fs::write(
            dir.join("LICENSE"),
            "GNU AFFERO GENERAL PUBLIC LICENSE\nVersion 3\n",
        )
        .unwrap();
        if self.kind != "bundle" {
            std::fs::create_dir_all(dir.join("src")).unwrap();
            std::fs::write(dir.join("src/lib.rs"), self.code).unwrap();
        }
        dir.to_path_buf()
    }

    /// Write into `<repo>/<id>-<version>/`.
    pub fn write_into(&self, repo: &Path) -> PathBuf {
        self.write(&repo.join(format!("{}-{}", self.id, self.version)))
    }
}

/// Restore owner write permission under `root` (store entries are read-only), so a temp dir holding
/// a store can be deleted when the test ends.
pub(crate) fn make_writable(root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    if let Ok(meta) = std::fs::symlink_metadata(root)
        && meta.is_dir()
    {
        let mut perms = meta.permissions();
        perms.set_mode(perms.mode() | 0o700);
        let _ = std::fs::set_permissions(root, perms);
    }
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            make_writable(&path);
        } else if meta.is_file() {
            let mut perms = meta.permissions();
            perms.set_mode(perms.mode() | 0o600);
            let _ = std::fs::set_permissions(&path, perms);
        }
    }
}
