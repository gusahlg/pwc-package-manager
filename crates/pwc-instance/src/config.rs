//! `config.toml` (docs/spec/filesystem.md).

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::InstanceError;
use crate::fsutil::{io, write_atomic};

/// One configured repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepositoryConfig {
    /// Display name.
    pub name: String,
    /// Directory.
    pub path: PathBuf,
}

/// How Cargo is invoked.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Wrapper {
    /// `nix develop <pwc> --command` when the PWC source has a flake and `nix` is on `PATH`.
    #[default]
    Auto,
    /// This prefix verbatim (`{pwc}` expands to the PWC source). Empty = run cargo directly.
    Command(Vec<String>),
}

/// The tooling configuration.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Config {
    /// PWC game checkout.
    pub pwc_source: Option<PathBuf>,
    /// `pwc instance use`.
    pub active_instance: Option<String>,
    /// Repositories in search order.
    pub repositories: Vec<RepositoryConfig>,
    /// `[build] wrapper`.
    pub wrapper: Wrapper,
    /// `[build] jobs` (0 = default).
    pub jobs: u32,
}

/// The environment variable that disables every Cargo wrapper when set (to anything but `0`).
pub const NO_WRAPPER_VAR: &str = "PWC_NO_WRAPPER";

/// The file as written, before validation.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct RawConfig {
    format: Option<i64>,
    pwc_source: Option<PathBuf>,
    active_instance: Option<String>,
    #[serde(default)]
    repository: Vec<RawRepository>,
    #[serde(default)]
    build: RawBuild,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRepository {
    name: String,
    path: PathBuf,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawBuild {
    wrapper: Option<RawWrapper>,
    #[serde(default)]
    jobs: u32,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum RawWrapper {
    Word(String),
    Command(Vec<String>),
}

impl Config {
    /// Load, or the default when the file does not exist.
    pub fn load(path: &Path) -> Result<Self, InstanceError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text).map_err(|e| {
                InstanceError::Config(format!("{}: {}", path.display(), config_msg(e)))
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(io(path, e)),
        }
    }

    /// Write atomically.
    pub fn save(&self, path: &Path) -> Result<(), InstanceError> {
        write_atomic(path, self.to_toml_string().as_bytes())
    }

    /// Parse text.
    pub fn parse(text: &str) -> Result<Self, InstanceError> {
        let raw: RawConfig =
            toml::from_str(text).map_err(|e| InstanceError::Config(e.message().to_string()))?;
        match raw.format {
            Some(1) => {}
            None => {
                return Err(InstanceError::Config(
                    "missing required key `format` (format = 1)".into(),
                ));
            }
            Some(found) => {
                return Err(InstanceError::Config(format!(
                    "unsupported format {found}; this pwc reads format 1 (upgrade pwc for newer formats)"
                )));
            }
        }
        if let Some(name) = &raw.active_instance {
            pwc_manifest::InstanceName::parse(name)
                .map_err(|e| InstanceError::Config(format!("`active-instance`: {e}")))?;
        }
        let mut names = BTreeSet::new();
        for repo in &raw.repository {
            if repo.name.trim().is_empty() {
                return Err(InstanceError::Config(
                    "a `[[repository]]` has an empty `name`".into(),
                ));
            }
            if repo.path.as_os_str().is_empty() {
                return Err(InstanceError::Config(format!(
                    "repository `{}` has an empty `path`",
                    repo.name
                )));
            }
            if !names.insert(repo.name.as_str()) {
                return Err(InstanceError::Config(format!(
                    "repository name `{}` is used twice",
                    repo.name
                )));
            }
        }
        let wrapper = match raw.build.wrapper {
            None => Wrapper::Auto,
            Some(RawWrapper::Word(w)) if w == "auto" => Wrapper::Auto,
            Some(RawWrapper::Word(w)) => {
                return Err(InstanceError::Config(format!(
                    "`build.wrapper` = {w:?}: must be \"auto\" or a list of strings"
                )));
            }
            Some(RawWrapper::Command(v)) => Wrapper::Command(v),
        };
        Ok(Self {
            pwc_source: raw.pwc_source.filter(|p| !p.as_os_str().is_empty()),
            active_instance: raw.active_instance,
            repositories: raw
                .repository
                .into_iter()
                .map(|r| RepositoryConfig {
                    name: r.name,
                    path: r.path,
                })
                .collect(),
            wrapper,
            jobs: raw.build.jobs,
        })
    }

    /// Canonical text.
    pub fn to_toml_string(&self) -> String {
        let mut out = String::from("format = 1\n");
        if let Some(pwc) = &self.pwc_source {
            out.push_str(&format!("pwc-source = {}\n", quote(&pwc.to_string_lossy())));
        }
        if let Some(active) = &self.active_instance {
            out.push_str(&format!("active-instance = {}\n", quote(active)));
        }
        for repo in &self.repositories {
            out.push_str(&format!(
                "\n[[repository]]\nname = {}\npath = {}\n",
                quote(&repo.name),
                quote(&repo.path.to_string_lossy())
            ));
        }
        let wrapper = match &self.wrapper {
            Wrapper::Auto => quote("auto"),
            Wrapper::Command(v) => format!(
                "[{}]",
                v.iter().map(|a| quote(a)).collect::<Vec<_>>().join(", ")
            ),
        };
        out.push_str(&format!(
            "\n[build]\nwrapper = {wrapper}\njobs = {}\n",
            self.jobs
        ));
        out
    }

    /// The resolved wrapper prefix for building against `pwc_source` (empty = none).
    ///
    /// `Auto` wraps with `nix develop <pwc_source> --command` when `<pwc_source>/flake.nix` exists
    /// and `nix` is on `PATH` (also inside another Nix shell: the game's shell provides its native
    /// libraries). `PWC_NO_WRAPPER` set to anything but `0` disables every wrapper.
    pub fn wrapper_command(&self, pwc_source: &Path) -> Vec<String> {
        self.wrapper_command_with(pwc_source, |name| std::env::var_os(name))
    }

    /// [`Config::wrapper_command`] with environment variables read through `var`.
    pub fn wrapper_command_with(
        &self,
        pwc_source: &Path,
        var: impl Fn(&str) -> Option<OsString>,
    ) -> Vec<String> {
        if var(NO_WRAPPER_VAR).is_some_and(|v| !v.is_empty() && v != "0") {
            return Vec::new();
        }
        let pwc = pwc_source.to_string_lossy();
        match &self.wrapper {
            Wrapper::Auto => {
                if pwc_source.join("flake.nix").is_file() && on_path("nix", var("PATH")) {
                    vec![
                        "nix".into(),
                        "develop".into(),
                        pwc.into_owned(),
                        "--command".into(),
                    ]
                } else {
                    Vec::new()
                }
            }
            Wrapper::Command(v) => v.iter().map(|a| a.replace("{pwc}", &pwc)).collect(),
        }
    }
}

/// Strip a duplicated "config: " prefix when re-wrapping a parse error with the file name.
fn config_msg(e: InstanceError) -> String {
    match e {
        InstanceError::Config(m) => m,
        other => other.to_string(),
    }
}

/// A TOML basic string.
fn quote(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

/// Whether an executable `program` exists in a directory of `path_var`.
fn on_path(program: &str, path_var: Option<OsString>) -> bool {
    let Some(path_var) = path_var else {
        return false;
    };
    std::env::split_paths(&path_var).any(|dir| {
        let candidate = dir.join(program);
        let Ok(meta) = std::fs::metadata(&candidate) else {
            return false;
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            meta.is_file() && meta.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            meta.is_file()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"format = 1
pwc-source = "/home/alice/src/project_watt_cubed"       # the PWC game checkout to build against
active-instance = "survival"                             # set by `pwc instance use`

[[repository]]
name = "first-party"
path = "/home/alice/src/pwc-package-manager/mods"

[build]
wrapper = "auto"
jobs = 0                                                  # 0 = Cargo's default
"#;

    #[test]
    fn parses_the_spec_example() {
        let c = Config::parse(EXAMPLE).unwrap();
        assert_eq!(
            c.pwc_source.as_deref(),
            Some(Path::new("/home/alice/src/project_watt_cubed"))
        );
        assert_eq!(c.active_instance.as_deref(), Some("survival"));
        assert_eq!(
            c.repositories,
            [RepositoryConfig {
                name: "first-party".into(),
                path: "/home/alice/src/pwc-package-manager/mods".into()
            }]
        );
        assert_eq!(c.wrapper, Wrapper::Auto);
        assert_eq!(c.jobs, 0);
    }

    #[test]
    fn round_trips() {
        let c = Config::parse(EXAMPLE).unwrap();
        let text = c.to_toml_string();
        assert_eq!(Config::parse(&text).unwrap(), c);
        assert_eq!(
            text,
            "format = 1\npwc-source = \"/home/alice/src/project_watt_cubed\"\nactive-instance = \"survival\"\n\n\
             [[repository]]\nname = \"first-party\"\npath = \"/home/alice/src/pwc-package-manager/mods\"\n\n\
             [build]\nwrapper = \"auto\"\njobs = 0\n"
        );

        let c = Config {
            pwc_source: Some("/tmp/with \"quotes\" and \\ backslash".into()),
            active_instance: None,
            repositories: vec![
                RepositoryConfig {
                    name: "a".into(),
                    path: "/a".into(),
                },
                RepositoryConfig {
                    name: "b".into(),
                    path: "/b".into(),
                },
            ],
            wrapper: Wrapper::Command(vec!["env".into(), "FOO={pwc}".into()]),
            jobs: 8,
        };
        assert_eq!(Config::parse(&c.to_toml_string()).unwrap(), c);
        assert!(
            c.to_toml_string()
                .contains("wrapper = [\"env\", \"FOO={pwc}\"]")
        );

        assert_eq!(
            Config::parse(&Config::default().to_toml_string()).unwrap(),
            Config::default()
        );
    }

    #[test]
    fn minimal_and_defaults() {
        let c = Config::parse("format = 1\n").unwrap();
        assert_eq!(c, Config::default());
        let c = Config::parse("format = 1\n[build]\nwrapper = []\n").unwrap();
        assert_eq!(c.wrapper, Wrapper::Command(vec![]));
    }

    #[test]
    fn rejects_bad_files() {
        for (text, needle) in [
            ("", "format"),
            ("format = 2\n", "unsupported format 2"),
            ("format = 1\nunknown = 1\n", "unknown"),
            (
                "format = 1\n[build]\nwrapper = \"always\"\n",
                "build.wrapper",
            ),
            ("format = 1\n[build]\nwrapper = 3\n", ""),
            ("format = 1\n[build]\njobs = -1\n", ""),
            (
                "format = 1\nactive-instance = \"Bad Name\"\n",
                "active-instance",
            ),
            (
                "format = 1\n[[repository]]\nname = \"a\"\npath = \"/a\"\n[[repository]]\nname = \"a\"\npath = \"/b\"\n",
                "twice",
            ),
            (
                "format = 1\n[[repository]]\nname = \"\"\npath = \"/a\"\n",
                "empty",
            ),
            ("format = 1\n[[repository]]\nname = \"a\"\n", "path"),
            (
                "format = 1\n[[repository]]\nname = \"a\"\npath = \"/a\"\nextra = 1\n",
                "extra",
            ),
            ("not toml [", ""),
        ] {
            let e = Config::parse(text).unwrap_err().to_string();
            assert!(e.contains(needle), "{text:?}: {e}");
        }
    }

    #[test]
    fn load_and_save() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config/config.toml");
        assert_eq!(
            Config::load(&path).unwrap(),
            Config::default(),
            "missing file = defaults"
        );
        let c = Config {
            jobs: 3,
            active_instance: Some("dev".into()),
            ..Config::default()
        };
        c.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap(), c);
        std::fs::write(&path, "format = 7\n").unwrap();
        let e = Config::load(&path).unwrap_err().to_string();
        assert!(
            e.starts_with("config: ") && e.contains("config.toml") && e.contains("format 7"),
            "{e}"
        );
    }

    fn env<'a>(pairs: &'a [(&'a str, OsString)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.clone())
        }
    }

    #[test]
    fn auto_wrapper() {
        let tmp = tempfile::tempdir().unwrap();
        let pwc = tmp.path().join("pwc");
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&pwc).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        let fake_nix = bin.join("nix");
        std::fs::write(&fake_nix, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake_nix, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path_var = std::env::join_paths([tmp.path().join("nowhere"), bin.clone()]).unwrap();
        let c = Config::default();

        // No flake: no wrapper.
        assert!(
            c.wrapper_command_with(&pwc, env(&[("PATH", path_var.clone())]))
                .is_empty()
        );

        std::fs::write(pwc.join("flake.nix"), "{}").unwrap();
        let expected = vec![
            "nix".to_string(),
            "develop".into(),
            pwc.to_string_lossy().into_owned(),
            "--command".into(),
        ];
        assert_eq!(
            c.wrapper_command_with(&pwc, env(&[("PATH", path_var.clone())])),
            expected
        );
        // Inside a Nix shell too.
        assert_eq!(
            c.wrapper_command_with(
                &pwc,
                env(&[
                    ("PATH", path_var.clone()),
                    ("IN_NIX_SHELL", "impure".into())
                ])
            ),
            expected
        );
        // No nix on PATH.
        assert!(
            c.wrapper_command_with(&pwc, env(&[("PATH", tmp.path().join("nowhere").into())]))
                .is_empty()
        );
        assert!(c.wrapper_command_with(&pwc, env(&[])).is_empty());
        // Disabled.
        assert!(
            c.wrapper_command_with(
                &pwc,
                env(&[("PATH", path_var.clone()), (NO_WRAPPER_VAR, "1".into())])
            )
            .is_empty()
        );
        assert_eq!(
            c.wrapper_command_with(
                &pwc,
                env(&[("PATH", path_var.clone()), (NO_WRAPPER_VAR, "0".into())])
            ),
            expected
        );
    }

    #[test]
    fn explicit_wrapper() {
        let c = Config {
            wrapper: Wrapper::Command(vec![
                "nix".into(),
                "develop".into(),
                "{pwc}#ci".into(),
                "-c".into(),
            ]),
            ..Config::default()
        };
        let pwc = Path::new("/src/pwc");
        assert_eq!(
            c.wrapper_command_with(pwc, env(&[])),
            ["nix", "develop", "/src/pwc#ci", "-c"]
        );
        assert!(
            c.wrapper_command_with(pwc, env(&[(NO_WRAPPER_VAR, "yes".into())]))
                .is_empty()
        );
        let direct = Config {
            wrapper: Wrapper::Command(vec![]),
            ..Config::default()
        };
        assert!(direct.wrapper_command_with(pwc, env(&[])).is_empty());
        // The real environment does not panic.
        let _ = c.wrapper_command(pwc);
    }
}
