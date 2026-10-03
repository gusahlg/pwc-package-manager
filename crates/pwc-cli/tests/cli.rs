//! End-to-end tests of the `pwc` binary: every command runs as a subprocess with `PWC_HOME`
//! pointing into a temporary directory, against a fake PWC source and a temporary repository.
//! `pwc build`/`pwc run` use a fake toolchain behind the configured wrapper (no real Cargo).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// One test's world.
struct Env {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
    pwc: PathBuf,
    repo: PathBuf,
    work: PathBuf,
}

/// A finished `pwc` invocation.
#[derive(Debug)]
struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

/// Write a package source directory.
fn write_mod(dir: &Path, id: &str, version: &str, kind: &str, deps: &[(&str, &str)]) {
    let api = if kind == "bundle" {
        String::new()
    } else {
        "pwc-api = \"^1.0\"\n".to_owned()
    };
    let mut deps_toml = String::new();
    for (dep, req) in deps {
        deps_toml.push_str(&format!("\"{dep}\" = \"{req}\"\n"));
    }
    write(
        &dir.join("mod.toml"),
        &format!(
            "format = 1\n\n[package]\nid = \"{id}\"\nname = \"Test {id}\"\nversion = \"{version}\"\n\
             description = \"The {id} test package.\"\nauthors = [\"Tester\"]\nlicense = \"AGPL-3.0-or-later\"\n\
             license-files = [\"LICENSE\"]\nkind = \"{kind}\"\n{api}keywords = [\"test\"]\n\n[dependencies]\n{deps_toml}"
        ),
    );
    write(&dir.join("README.md"), &format!("# {id}\n"));
    write(
        &dir.join("LICENSE"),
        "GNU AFFERO GENERAL PUBLIC LICENSE, Version 3\n",
    );
    match kind {
        "mod" => write(
            &dir.join("src/lib.rs"),
            "pub fn register(_registrar: &mut pwc_mod_api::ModRegistrar) {}\n",
        ),
        "library" => write(
            &dir.join("src/lib.rs"),
            "pub fn name() -> &'static str { \"names\" }\n",
        ),
        _ => {}
    }
}

impl Env {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let pwc = root.join("project_watt_cubed");
        write(
            &pwc.join("Cargo.toml"),
            "[package]\nname = \"project_watt_cubed\"\nversion = \"2.0.0\"\nedition = \"2024\"\n\n\
             [workspace]\nresolver = \"2\"\nmembers = [\"crates/pwc-mod-api\"]\n\n\
             [profile.release]\nlto = \"fat\"\ncodegen-units = 1\n\n[profile.dev.package.\"*\"]\nopt-level = 2\n",
        );
        write(&pwc.join("Cargo.lock"), "version = 4\n");
        write(
            &pwc.join("crates/pwc-mod-api/Cargo.toml"),
            "[package]\nname = \"pwc-mod-api\"\nversion = \"1.0.0\"\nedition = \"2024\"\n",
        );
        write(&pwc.join("assets/readme.txt"), "assets\n");

        let repo = root.join("repo");
        write_mod(&repo.join("hotbar"), "pwc.hotbar", "1.0.0", "mod", &[]);
        write_mod(&repo.join("hotbar-next"), "pwc.hotbar", "1.1.0", "mod", &[]);
        write_mod(&repo.join("names"), "pwc.names", "1.0.0", "library", &[]);
        write_mod(
            &repo.join("inventory"),
            "pwc.inventory",
            "1.0.0",
            "mod",
            &[("pwc.hotbar", "^1.0"), ("pwc.names", "^1.0")],
        );
        write_mod(
            &repo.join("essentials"),
            "pwc.essentials",
            "1.0.0",
            "bundle",
            &[("pwc.inventory", "^1.0")],
        );

        let work = root.join("work");
        std::fs::create_dir_all(&work).unwrap();
        Self {
            home: root.join("home"),
            _tmp: tmp,
            root,
            pwc,
            repo,
            work,
        }
    }

    fn run(&self, args: &[&str]) -> Out {
        let output = Command::new(env!("CARGO_BIN_EXE_pwc"))
            .args(args)
            .current_dir(&self.work)
            .env("PWC_HOME", &self.home)
            .env_remove("PWC_NO_WRAPPER")
            .env_remove("XDG_DATA_HOME")
            .env_remove("XDG_CACHE_HOME")
            .env_remove("XDG_CONFIG_HOME")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        Out {
            code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    /// Run and require success.
    fn ok(&self, args: &[&str]) -> Out {
        let out = self.run(args);
        assert_eq!(out.code, 0, "pwc {args:?} failed:\n{out:#?}");
        out
    }

    /// Run and require a reported error (exit status 1, `error:` first).
    fn fail(&self, args: &[&str]) -> Out {
        let out = self.run(args);
        assert_eq!(out.code, 1, "pwc {args:?} should fail:\n{out:#?}");
        assert!(
            out.stderr.starts_with("error: "),
            "pwc {args:?}:\n{}",
            out.stderr
        );
        out
    }

    fn setup(&self) {
        self.ok(&[
            "setup",
            "--pwc",
            self.pwc.to_str().unwrap(),
            "--repository",
            self.repo.to_str().unwrap(),
        ]);
    }

    /// `setup` plus an active instance `default`.
    fn with_instance(&self) {
        self.setup();
        self.ok(&["instance", "create", "default", "--use"]);
    }

    fn instance_dir(&self, name: &str) -> PathBuf {
        self.home.join("data/instances").join(name)
    }

    fn instance_toml(&self) -> toml::Table {
        std::fs::read_to_string(self.instance_dir("default").join("instance.toml"))
            .unwrap()
            .parse()
            .unwrap()
    }

    fn lock(&self) -> toml::Table {
        std::fs::read_to_string(self.instance_dir("default").join("pwc.lock"))
            .unwrap()
            .parse()
            .unwrap()
    }

    /// `(id, version)` of every locked package.
    fn locked(&self) -> Vec<(String, String)> {
        let lock = self.lock();
        let Some(packages) = lock.get("package").and_then(|p| p.as_array()) else {
            return Vec::new();
        };
        packages
            .iter()
            .map(|p| {
                (
                    p["id"].as_str().unwrap().to_owned(),
                    p["version"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    }
}

impl Drop for Env {
    /// Store entries are read-only (directories `0555`); make them writable again so the
    /// temporary directory can be deleted.
    fn drop(&mut self) {
        fn make_writable(path: &Path) {
            let Ok(meta) = std::fs::symlink_metadata(path) else {
                return;
            };
            if !meta.is_dir() {
                return;
            }
            let mut permissions = meta.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            let _ = std::fs::set_permissions(path, permissions);
            for entry in std::fs::read_dir(path).into_iter().flatten().flatten() {
                make_writable(&entry.path());
            }
        }
        make_writable(&self.root);
    }
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

#[test]
fn usage_errors_exit_with_status_2() {
    let env = Env::new();
    assert_eq!(env.run(&[]).code, 2);
    assert_eq!(env.run(&["frobnicate"]).code, 2);
    assert_eq!(env.run(&["mod", "add"]).code, 2, "mod add needs a spec");
    assert_eq!(env.run(&["build", "--profile", "fast"]).code, 2);
    let help = env.ok(&["--help"]);
    assert!(help.stdout.contains("instance") && help.stdout.contains("package"));
}

#[test]
fn setup_config_and_repositories() {
    let env = Env::new();
    let out = env.ok(&[
        "setup",
        "--pwc",
        env.pwc.to_str().unwrap(),
        "--repository",
        env.repo.to_str().unwrap(),
    ]);
    assert!(
        out.stdout.contains(&format!(
            "pwc-source  {} (PWC 2.0.0, pwc-mod-api 1.0.0)",
            env.pwc.display()
        )),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout
            .contains(&format!("repository  repo  {}", env.repo.display())),
        "{}",
        out.stdout
    );
    assert!(env.home.join("config/config.toml").is_file());

    let config = env.ok(&["config"]);
    assert!(
        config
            .stdout
            .contains(&format!("pwc-source              {}", env.pwc.display())),
        "{}",
        config.stdout
    );
    assert!(
        config
            .stdout
            .contains(&env.home.join("data/store").display().to_string())
    );
    assert!(
        config.stdout.contains("cargo (no wrapper)"),
        "no flake.nix, so no wrapper:\n{}",
        config.stdout
    );

    let other = env.root.join("other-repo");
    std::fs::create_dir_all(&other).unwrap();
    env.ok(&["repo", "add", other.to_str().unwrap(), "--name", "extra"]);
    let list = env.ok(&["repo", "list"]);
    assert!(list.stdout.starts_with("NAME"), "{}", list.stdout);
    assert!(
        list.stdout.contains("repo ") && list.stdout.contains("extra"),
        "{}",
        list.stdout
    );
    let dup = env.fail(&["repo", "add", other.to_str().unwrap()]);
    assert!(
        dup.stderr
            .contains("already configured as repository `extra`"),
        "{}",
        dup.stderr
    );
    env.fail(&["repo", "add", env.root.join("missing").to_str().unwrap()]);
    env.ok(&["repo", "remove", "extra"]);
    assert!(!env.ok(&["repo", "list"]).stdout.contains("extra"));
    env.fail(&["repo", "remove", "extra"]);

    // Re-running setup without --repository keeps the configured repositories.
    env.ok(&["setup", "--pwc", env.pwc.to_str().unwrap()]);
    assert!(
        env.ok(&["repo", "list"])
            .stdout
            .contains(&env.repo.display().to_string())
    );
}

#[test]
fn setup_rejects_a_directory_that_is_not_a_pwc_source() {
    let env = Env::new();
    let out = env.fail(&["setup", "--pwc", env.repo.to_str().unwrap()]);
    assert!(
        out.stderr.contains("is not a usable PWC source"),
        "{}",
        out.stderr
    );
    assert!(
        out.stderr.contains("\n  cause: "),
        "the cause chain is printed:\n{}",
        out.stderr
    );
    assert!(!env.home.join("config/config.toml").exists());
}

#[test]
fn package_check_build_verify_inspect() {
    let env = Env::new();
    let src = env.root.join("src-hotbar");
    write_mod(&src, "pwc.hotbar", "1.0.0", "mod", &[]);
    write(&src.join("notes.txt"), "not part of the package");

    let check = env.ok(&["package", "check", src.to_str().unwrap()]);
    assert!(
        check
            .stdout
            .starts_with("ok: pwc.hotbar 1.0.0 (mod), 4 files"),
        "{}",
        check.stdout
    );
    assert!(check.stderr.contains("1 path(s)"), "{}", check.stderr);
    let verbose = env.ok(&["package", "check", "-v", src.to_str().unwrap()]);
    assert!(
        verbose.stderr.contains("ignored: notes.txt"),
        "{}",
        verbose.stderr
    );
    let hash = check.stdout.trim().rsplit(", ").next().unwrap().to_owned();
    assert!(hash.starts_with("sha256:") && hash.len() == 71, "{hash}");

    // Default output directory: <dir>/dist.
    let built = env.ok(&["package", "build", src.to_str().unwrap()]);
    let file = src.join("dist/pwc.hotbar-1.0.0.pwcmod");
    assert_eq!(built.stdout, format!("{hash}  {}\n", file.display()));
    assert!(file.is_file());

    let out_dir = env.root.join("out");
    let built = env.ok(&[
        "package",
        "build",
        src.to_str().unwrap(),
        "-o",
        out_dir.to_str().unwrap(),
        "-q",
    ]);
    assert!(
        built.stderr.is_empty(),
        "-q silences status: {}",
        built.stderr
    );
    let file = out_dir.join("pwc.hotbar-1.0.0.pwcmod");
    assert!(built.stdout.starts_with(&hash));

    let verify = env.ok(&["package", "verify", file.to_str().unwrap(), "--hash", &hash]);
    assert_eq!(verify.stdout, format!("ok: pwc.hotbar 1.0.0 {hash}\n"));
    let wrong = format!("sha256:{}", "0".repeat(64));
    let mismatch = env.fail(&[
        "package",
        "verify",
        file.to_str().unwrap(),
        "--hash",
        &wrong,
    ]);
    assert!(
        mismatch.stderr.contains("expected sha256:000"),
        "{}",
        mismatch.stderr
    );
    env.fail(&[
        "package",
        "verify",
        file.to_str().unwrap(),
        "--hash",
        "md5:abc",
    ]);

    let inspect = env.ok(&["package", "inspect", file.to_str().unwrap()]);
    for needle in [
        "id            pwc.hotbar",
        "kind          mod",
        "pwc-api       ^1.0",
        &format!("hash          {hash}"),
        "src/lib.rs",
        "mod.toml",
    ] {
        assert!(
            inspect.stdout.contains(needle),
            "missing {needle:?}:\n{}",
            inspect.stdout
        );
    }

    // A corrupt archive fails verification.
    let corrupt = env.root.join("corrupt.pwcmod");
    std::fs::write(&corrupt, b"not zstd").unwrap();
    env.fail(&["package", "verify", corrupt.to_str().unwrap()]);
}

#[test]
fn package_check_reports_the_cause() {
    let env = Env::new();
    let src = env.root.join("bad");
    write_mod(&src, "pwc.bad", "1.0.0", "mod", &[]);
    let text = std::fs::read_to_string(src.join("mod.toml"))
        .unwrap()
        .replace("AGPL-3.0-or-later", "LicenseRef-Proprietary");
    write(&src.join("mod.toml"), &text);
    let out = env.fail(&["package", "check", src.to_str().unwrap()]);
    assert!(
        out.stderr.starts_with(&format!(
            "error: {} is not a valid package\n  cause: ",
            src.display()
        )),
        "{}",
        out.stderr
    );
    assert!(
        out.stderr.contains("LicenseRef-Proprietary"),
        "{}",
        out.stderr
    );

    let empty = env.root.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    env.fail(&["package", "check", empty.to_str().unwrap()]);
}

#[test]
fn instance_create_list_use_show_remove() {
    let env = Env::new();
    env.setup();
    assert!(
        env.fail(&["mod", "list"])
            .stderr
            .contains("no instance selected")
    );
    env.ok(&["instance", "create", "alpha", "--use"]);
    env.ok(&["instance", "create", "beta", "--pwc", "^2.0"]);
    assert!(
        env.fail(&["instance", "create", "alpha"])
            .stderr
            .contains("already exists")
    );
    env.fail(&["instance", "create", "Bad_Name"]);
    env.fail(&["instance", "create", "gamma", "--pwc", "not a req"]);

    let list = env.ok(&["instance", "list"]);
    let lines: Vec<&str> = list.stdout.lines().collect();
    assert_eq!(lines.len(), 3, "{}", list.stdout);
    assert!(lines[1].starts_with("*  alpha"), "{}", list.stdout);
    assert!(
        lines[2].starts_with("   beta") && lines[2].contains("^2.0"),
        "{}",
        list.stdout
    );

    env.ok(&["instance", "use", "beta"]);
    assert!(
        env.ok(&["instance", "list"])
            .stdout
            .lines()
            .nth(2)
            .unwrap()
            .starts_with("*  beta")
    );
    env.fail(&["instance", "use", "nope"]);

    let show = env.ok(&["instance", "show"]);
    assert!(
        show.stdout.starts_with("instance  beta (active)\n"),
        "{}",
        show.stdout
    );
    assert!(
        show.stdout.contains("requirements: none") && show.stdout.contains("not locked yet"),
        "{}",
        show.stdout
    );
    assert!(
        env.ok(&["instance", "show", "alpha"])
            .stdout
            .starts_with("instance  alpha\n")
    );
    assert!(
        env.ok(&["--instance", "alpha", "instance", "show"])
            .stdout
            .starts_with("instance  alpha\n")
    );

    // Without a terminal, deletion needs --yes.
    let refused = env.fail(&["instance", "remove", "beta"]);
    assert!(refused.stderr.contains("--yes"), "{}", refused.stderr);
    assert!(env.instance_dir("beta").is_dir());
    env.ok(&["instance", "remove", "beta", "--yes"]);
    assert!(!env.instance_dir("beta").exists());
    assert!(
        env.ok(&["config"])
            .stdout
            .contains("active-instance         none")
    );
    assert_eq!(env.ok(&["instance", "list"]).stdout.lines().count(), 2);
}

#[test]
fn mod_add_remove_update_list_lock_env() {
    let env = Env::new();
    env.with_instance();

    let added = env.ok(&["mod", "add", "pwc.essentials"]);
    assert!(
        added.stderr.contains("+ pwc.hotbar 1.1.0"),
        "{}",
        added.stderr
    );
    assert!(
        added.stderr.contains("Added pwc.essentials ^1.0.0"),
        "{}",
        added.stderr
    );
    assert_eq!(
        env.locked(),
        pairs(&[
            ("pwc.essentials", "1.0.0"),
            ("pwc.hotbar", "1.1.0"),
            ("pwc.inventory", "1.0.0"),
            ("pwc.names", "1.0.0")
        ])
    );
    // Like `cargo add`, a bare id is written as a caret requirement on the selected version.
    assert_eq!(
        env.instance_toml()["mods"]["pwc.essentials"].as_str(),
        Some("^1.0.0")
    );

    let list = env.ok(&["mod", "list"]);
    let essentials = list
        .stdout
        .lines()
        .find(|l| l.contains("pwc.essentials"))
        .unwrap();
    assert!(
        essentials.starts_with('*') && essentials.contains("bundle"),
        "{}",
        list.stdout
    );
    let hotbar = list
        .stdout
        .lines()
        .find(|l| l.contains("pwc.hotbar"))
        .unwrap();
    assert!(
        hotbar.starts_with(' ') && hotbar.contains("repository:"),
        "{}",
        list.stdout
    );

    let environment = env.ok(&["env"]).stdout;
    assert!(
        environment.starts_with("sha256:") && environment.trim().len() == 71,
        "{environment}"
    );
    assert_eq!(env.lock()["environment"].as_str(), Some(environment.trim()));
    assert!(env.ok(&["lock"]).stderr.contains("Lock unchanged"));
    assert_eq!(env.ok(&["env"]).stdout, environment);

    // An explicit requirement downgrades; `mod update` respects it.
    env.ok(&["mod", "add", "pwc.hotbar@=1.0.0"]);
    assert_eq!(
        env.instance_toml()["mods"]["pwc.hotbar"].as_str(),
        Some("=1.0.0")
    );
    assert!(
        env.locked()
            .contains(&("pwc.hotbar".to_owned(), "1.0.0".to_owned()))
    );
    env.ok(&["mod", "update", "pwc.hotbar"]);
    assert!(
        env.locked()
            .contains(&("pwc.hotbar".to_owned(), "1.0.0".to_owned()))
    );
    assert_ne!(env.ok(&["env"]).stdout, environment);

    // Removing the pin keeps the locked version (conservative) until an update.
    env.ok(&["mod", "remove", "pwc.hotbar"]);
    assert!(
        env.locked()
            .contains(&("pwc.hotbar".to_owned(), "1.0.0".to_owned()))
    );
    env.ok(&["mod", "update"]);
    assert!(
        env.locked()
            .contains(&("pwc.hotbar".to_owned(), "1.1.0".to_owned()))
    );
    env.fail(&["mod", "update", "pwc.unknown"]);

    // Failures leave instance.toml untouched.
    let before =
        std::fs::read_to_string(env.instance_dir("default").join("instance.toml")).unwrap();
    let unknown = env.fail(&["mod", "add", "pwc.nope"]);
    assert!(unknown.stderr.contains("pwc.nope"), "{}", unknown.stderr);
    assert_eq!(
        std::fs::read_to_string(env.instance_dir("default").join("instance.toml")).unwrap(),
        before
    );
    let indirect = env.fail(&["mod", "remove", "pwc.inventory"]);
    assert!(
        indirect.stderr.contains("dependency of another mod"),
        "{}",
        indirect.stderr
    );
    env.fail(&["mod", "add", "Not An Id"]);
    env.fail(&["mod", "add", "./missing-dir"]);

    env.ok(&["mod", "remove", "pwc.essentials"]);
    assert!(env.locked().is_empty());
    assert!(env.ok(&["mod", "list"]).stderr.contains("No mods"));
}

#[test]
fn mod_add_accepts_source_directories_and_archives() {
    let env = Env::new();
    env.with_instance();

    // A relative source directory is stored as an absolute path.
    let dev = env.work.join("dev-mod");
    write_mod(&dev, "me.dev", "0.1.0", "mod", &[("pwc.hotbar", "^1.0")]);
    env.ok(&["mod", "add", "dev-mod"]);
    let mods = env.instance_toml()["mods"].clone();
    assert_eq!(mods["me.dev"]["path"].as_str(), Some(dev.to_str().unwrap()));
    assert!(
        env.locked()
            .contains(&("me.dev".to_owned(), "0.1.0".to_owned()))
    );
    let lock = env.lock();
    let dev_entry = lock["package"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"].as_str() == Some("me.dev"))
        .unwrap();
    assert_eq!(
        dev_entry["source"].as_str(),
        Some(format!("path:{}", dev.display()).as_str())
    );

    let src = env.root.join("archived");
    write_mod(&src, "me.archived", "2.0.0", "mod", &[]);
    let out = env.ok(&["package", "build", src.to_str().unwrap()]);
    let file = out.stdout.split("  ").nth(1).unwrap().trim().to_owned();
    env.ok(&["mod", "add", &file]);
    assert_eq!(
        env.instance_toml()["mods"]["me.archived"]["file"].as_str(),
        Some(file.as_str())
    );
    assert!(
        env.locked()
            .contains(&("me.archived".to_owned(), "2.0.0".to_owned()))
    );

    let show = env.ok(&["instance", "show"]);
    assert!(
        show.stdout.contains(&format!("path {}", dev.display())),
        "{}",
        show.stdout
    );
    assert!(
        show.stdout.contains("environment sha256:"),
        "{}",
        show.stdout
    );
}

#[test]
fn search_and_info() {
    let env = Env::new();
    env.setup();
    let search = env.ok(&["search"]);
    let lines: Vec<&str> = search.stdout.lines().collect();
    assert!(
        lines[0].starts_with("ID") && lines[0].contains("DESCRIPTION"),
        "{}",
        search.stdout
    );
    assert_eq!(lines.len(), 5, "one row per id:\n{}", search.stdout);
    let hotbar = lines.iter().find(|l| l.starts_with("pwc.hotbar")).unwrap();
    assert!(
        hotbar.contains("1.1.0") && hotbar.contains("mod"),
        "newest version: {hotbar}"
    );

    let filtered = env.ok(&["search", "INVENTORY"]);
    assert_eq!(filtered.stdout.lines().count(), 2, "{}", filtered.stdout);
    assert!(
        env.ok(&["search", "no-such-thing"])
            .stderr
            .contains("No packages match")
    );

    let info = env.ok(&["info", "pwc.hotbar"]);
    assert!(
        info.stdout.starts_with("pwc.hotbar — Test pwc.hotbar\n"),
        "{}",
        info.stdout
    );
    let v11 = info
        .stdout
        .find("pwc.hotbar 1.1.0 (mod)")
        .expect("1.1.0 listed");
    let v10 = info
        .stdout
        .find("pwc.hotbar 1.0.0 (mod)")
        .expect("1.0.0 listed");
    assert!(v11 < v10, "newest first:\n{}", info.stdout);
    let inventory = env.ok(&["info", "pwc.inventory"]);
    assert!(
        inventory
            .stdout
            .contains("dependencies  pwc.hotbar ^1.0, pwc.names ^1.0"),
        "{}",
        inventory.stdout
    );
    env.fail(&["info", "pwc.nope"]);
    env.fail(&["info", "not an id"]);
}

#[test]
fn store_list_verify_gc() {
    let env = Env::new();
    env.with_instance();
    assert!(env.ok(&["store", "list"]).stderr.contains("empty"));
    env.ok(&["mod", "add", "pwc.essentials"]);

    let list = env.ok(&["store", "list"]);
    let rows: Vec<&str> = list.stdout.lines().skip(1).collect();
    assert_eq!(rows.len(), 4, "{}", list.stdout);
    assert!(
        rows.iter()
            .any(|r| r.starts_with("pwc.hotbar") && r.contains("1.1.0") && r.contains("sha256:"))
    );
    assert!(
        env.ok(&["store", "verify"])
            .stderr
            .contains("4 store entries verified")
    );

    // Still referenced: nothing to collect.
    assert!(env.ok(&["store", "gc"]).stdout.is_empty());

    env.ok(&["mod", "remove", "pwc.essentials"]);
    let dry = env.ok(&["store", "gc", "--dry-run"]);
    assert_eq!(
        dry.stdout
            .lines()
            .filter(|l| l.starts_with("would remove"))
            .count(),
        4,
        "{}",
        dry.stdout
    );
    assert_eq!(env.ok(&["store", "list"]).stdout.lines().count(), 5);
    let gc = env.ok(&["store", "gc"]);
    assert_eq!(
        gc.stdout
            .lines()
            .filter(|l| l.starts_with("removed"))
            .count(),
        4,
        "{}",
        gc.stdout
    );
    assert!(env.ok(&["store", "list"]).stdout.is_empty());
}

#[test]
fn commands_explain_missing_setup() {
    let env = Env::new();
    env.ok(&["instance", "create", "default", "--use"]);
    let out = env.fail(&["lock"]);
    assert!(
        out.stderr
            .contains("no PWC source configured (run `pwc setup`)"),
        "{}",
        out.stderr
    );
    assert!(env.fail(&["env"]).stderr.contains("has no pwc.lock yet"));
}

#[cfg(unix)]
mod fake_toolchain {
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    /// Configure a wrapper script that fakes `rustc -vV` and `cargo build` (the "executables" it
    /// builds print their environment and exit with status 7) and runs anything else as is.
    fn install_fake_toolchain(env: &Env) -> PathBuf {
        let log = env.root.join("cargo.log");
        let script = env.root.join("fake-toolchain.sh");
        let game = r#"#!/bin/sh
echo "args=$*"
echo "WATT_DATA_DIR=$WATT_DATA_DIR"
echo "WATT_ASSET_DIR=$WATT_ASSET_DIR"
echo "WATT_GOLDEN_DIR=$WATT_GOLDEN_DIR"
echo "PWC_INSTANCE=$PWC_INSTANCE"
echo "PWC_ENVIRONMENT=$PWC_ENVIRONMENT"
echo "PWD=$(pwd)"
exit 7
"#;
        write(&env.root.join("fake-game.sh"), game);
        let body = format!(
            r#"#!/bin/sh
set -e
tool="$1"; shift
case "$tool" in
  rustc) printf 'rustc 1.95.0 (fake)\nhost: x86_64-unknown-linux-gnu\n' ;;
  cargo)
    echo "cargo $*" >> "{log}"
    profile=debug
    for arg in "$@"; do [ "$arg" = --release ] && profile=release; done
    mkdir -p "$CARGO_TARGET_DIR/$profile"
    for bin in pwc-game pwc-golden; do
      cp "{game}" "$CARGO_TARGET_DIR/$profile/$bin"
      chmod +x "$CARGO_TARGET_DIR/$profile/$bin"
    done
    ;;
  *) exec "$tool" "$@" ;;
esac
"#,
            log = log.display(),
            game = env.root.join("fake-game.sh").display(),
        );
        write(&script, &body);
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let config_path = env.home.join("config/config.toml");
        let mut config: toml::Table = std::fs::read_to_string(&config_path)
            .unwrap()
            .parse()
            .unwrap();
        let build = config
            .entry("build")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        build.as_table_mut().unwrap().insert(
            "wrapper".into(),
            toml::Value::Array(vec![script.to_str().unwrap().into()]),
        );
        std::fs::write(&config_path, toml::to_string(&config).unwrap()).unwrap();
        log
    }

    #[test]
    fn build_cache_run_and_clean() {
        let env = Env::new();
        env.with_instance();
        env.ok(&["mod", "add", "pwc.essentials"]);
        let log = install_fake_toolchain(&env);

        let first = env.ok(&["build"]);
        let game = PathBuf::from(first.stdout.trim());
        assert!(game.ends_with("bin/pwc-game"), "{}", first.stdout);
        assert!(
            game.starts_with(env.home.join("cache/builds")),
            "{}",
            game.display()
        );
        assert!(game.is_file());
        let dir_name = game
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        assert_eq!(
            dir_name.len(),
            32,
            "build directory = first 32 hex digits of the build id"
        );
        assert!(
            first
                .stderr
                .contains("Building `default` (release, 4 packages"),
            "{}",
            first.stderr
        );
        let calls = std::fs::read_to_string(&log).unwrap();
        assert_eq!(calls.lines().count(), 1);
        assert!(
            calls.contains("--bins -p pwc-game-instance --release"),
            "{calls}"
        );

        let second = env.ok(&["build"]);
        assert_eq!(second.stdout, first.stdout);
        assert!(
            second.stderr.contains("Using the cached build"),
            "{}",
            second.stderr
        );
        assert_eq!(
            std::fs::read_to_string(&log).unwrap().lines().count(),
            1,
            "cache hit: Cargo not invoked"
        );

        let dir = env.ok(&["build", "--print-dir"]);
        let build_dir = PathBuf::from(dir.stdout.trim());
        assert_eq!(build_dir, game.parent().unwrap().parent().unwrap());
        assert!(
            build_dir.join("build.toml").is_file() && build_dir.join("bundle/src/lib.rs").is_file()
        );

        env.ok(&["build", "--force", "-q"]);
        assert_eq!(std::fs::read_to_string(&log).unwrap().lines().count(), 2);
        let dev = env.ok(&["build", "--profile", "dev"]);
        assert_ne!(dev.stdout, first.stdout);
        assert!(
            std::fs::read_to_string(&log)
                .unwrap()
                .lines()
                .nth(2)
                .unwrap()
                .ends_with("-p pwc-game-instance")
        );

        // `pwc run`: the game runs with the instance's environment, arguments after `--`, its
        // exit status propagated and the working directory unchanged.
        let environment = env.ok(&["env"]).stdout.trim().to_owned();
        let run = env.run(&["run", "--", "--seed", "42"]);
        assert_eq!(run.code, 7, "{run:#?}");
        let stdout = run.stdout;
        assert!(stdout.contains("args=--seed 42\n"), "{stdout}");
        assert!(
            stdout.contains(&format!(
                "WATT_DATA_DIR={}\n",
                env.instance_dir("default").join("game").display()
            )),
            "{stdout}"
        );
        assert!(
            stdout.contains(&format!(
                "WATT_ASSET_DIR={}\n",
                env.pwc.join("assets").display()
            )),
            "{stdout}"
        );
        assert!(stdout.contains("WATT_GOLDEN_DIR=\n"), "{stdout}");
        assert!(stdout.contains("PWC_INSTANCE=default\n"), "{stdout}");
        assert!(
            stdout.contains(&format!("PWC_ENVIRONMENT={environment}\n")),
            "{stdout}"
        );
        assert!(
            stdout.contains(&format!("PWD={}\n", env.work.display())),
            "{stdout}"
        );
        assert!(env.instance_dir("default").join("game").is_dir());

        let golden = env.run(&["run", "--golden"]);
        assert_eq!(golden.code, 7, "{golden:#?}");
        let goldens = env.instance_dir("default").join("goldens");
        assert!(
            golden
                .stdout
                .contains(&format!("WATT_GOLDEN_DIR={}\n", goldens.display())),
            "{}",
            golden.stdout
        );
        assert!(goldens.is_dir());
        assert_eq!(
            std::fs::read_to_string(&log).unwrap().lines().count(),
            3,
            "run reuses the cached build"
        );

        env.ok(&["cache", "clean"]);
        assert!(!env.home.join("cache/builds").exists() && !env.home.join("cache/target").exists());
        assert!(!game.exists());
    }

    #[test]
    fn build_relocks_when_a_path_package_changes() {
        let env = Env::new();
        env.with_instance();
        let dev = env.root.join("dev-mod");
        write_mod(&dev, "me.dev", "0.1.0", "mod", &[]);
        env.ok(&["mod", "add", dev.to_str().unwrap()]);
        let log = install_fake_toolchain(&env);
        let before = env.ok(&["env"]).stdout;
        let first = env.ok(&["build"]).stdout;

        write(
            &dev.join("src/lib.rs"),
            "pub fn register(_r: &mut pwc_mod_api::ModRegistrar) { /* edited */ }\n",
        );
        let second = env.ok(&["build"]);
        assert_ne!(
            env.ok(&["env"]).stdout,
            before,
            "the lock records the new hash"
        );
        assert_ne!(
            second.stdout, first,
            "a changed path package is a new build"
        );
        assert_eq!(std::fs::read_to_string(&log).unwrap().lines().count(), 2);
    }
}
