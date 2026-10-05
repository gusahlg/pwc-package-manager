//! Platform state directories (docs/spec/filesystem.md).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The three roots plus derived paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dirs {
    /// `$XDG_DATA_HOME/pwc`.
    pub data: PathBuf,
    /// `$XDG_CACHE_HOME/pwc`.
    pub cache: PathBuf,
    /// `$XDG_CONFIG_HOME/pwc`.
    pub config: PathBuf,
}

impl Dirs {
    /// From the environment: `PWC_HOME`, else XDG directories, else the native Windows local-app
    /// data directory or the conventional directories below `HOME`.
    pub fn from_env() -> Result<Self, crate::InstanceError> {
        Self::from_vars(|name| std::env::var_os(name))
    }

    /// [`Dirs::from_env`] with the variables read through `var` (for tests and embedders).
    ///
    /// Empty variables count as unset. A relative `PWC_HOME` is taken relative to the current
    /// directory; relative `XDG_*_HOME` values are ignored, as the XDG specification requires.
    pub fn from_vars(var: impl Fn(&str) -> Option<OsString>) -> Result<Self, crate::InstanceError> {
        Self::from_vars_for_platform(var, cfg!(windows))
    }

    fn from_vars_for_platform(
        var: impl Fn(&str) -> Option<OsString>,
        windows: bool,
    ) -> Result<Self, crate::InstanceError> {
        let get = |name: &str| var(name).filter(|v| !v.is_empty()).map(PathBuf::from);
        if let Some(home) = get("PWC_HOME") {
            let home = std::path::absolute(&home)
                .map_err(|source| crate::InstanceError::Io { path: home, source })?;
            return Ok(Self::under(&home));
        }
        let home = get("HOME");
        let windows_root = windows
            .then(|| {
                get("LOCALAPPDATA")
                    .filter(|p| p.is_absolute())
                    .or_else(|| {
                        get("USERPROFILE")
                            .filter(|p| p.is_absolute())
                            .map(|p| p.join("AppData").join("Local"))
                    })
                    .map(|p| p.join("pwc"))
            })
            .flatten();
        let base = |xdg: &str,
                    default: &str,
                    windows_dir: &str|
         -> Result<PathBuf, crate::InstanceError> {
            match get(xdg).filter(|p| p.is_absolute()) {
                Some(dir) => Ok(dir.join("pwc")),
                None => home
                    .as_ref()
                    .map(|h| h.join(default).join("pwc"))
                    .or_else(|| windows_root.as_ref().map(|p| p.join(windows_dir)))
                    .ok_or_else(|| {
                        crate::InstanceError::Config(format!(
                            "cannot locate the PWC directories: neither PWC_HOME, {xdg}, HOME nor a Windows profile directory is set"
                        ))
                    }),
            }
        };
        Ok(Self {
            data: base("XDG_DATA_HOME", ".local/share", "data")?,
            cache: base("XDG_CACHE_HOME", ".cache", "cache")?,
            config: base("XDG_CONFIG_HOME", ".config", "config")?,
        })
    }

    /// All three under one directory (`<root>/data`, `<root>/cache`, `<root>/config`).
    pub fn under(root: &Path) -> Self {
        Self {
            data: root.join("data"),
            cache: root.join("cache"),
            config: root.join("config"),
        }
    }

    /// `data/store`.
    pub fn store(&self) -> PathBuf {
        self.data.join("store")
    }

    /// `data/instances`.
    pub fn instances(&self) -> PathBuf {
        self.data.join("instances")
    }

    /// `cache/builds`.
    pub fn builds(&self) -> PathBuf {
        self.cache.join("builds")
    }

    /// `cache/target`.
    pub fn target(&self) -> PathBuf {
        self.cache.join("target")
    }

    /// `config/config.toml`.
    pub fn config_file(&self) -> PathBuf {
        self.config.join("config.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| OsString::from(v))
        }
    }

    fn absolute(name: &str) -> PathBuf {
        std::env::current_dir().unwrap().join("target").join(name)
    }

    #[test]
    fn defaults_under_home() {
        let home = absolute("home");
        let home_text = home.to_string_lossy();
        let d = Dirs::from_vars(vars(&[("HOME", &home_text)])).unwrap();
        assert_eq!(d.data, home.join(".local/share/pwc"));
        assert_eq!(d.cache, home.join(".cache/pwc"));
        assert_eq!(d.config, home.join(".config/pwc"));
        assert_eq!(d.store(), home.join(".local/share/pwc/store"));
        assert_eq!(d.instances(), home.join(".local/share/pwc/instances"));
        assert_eq!(d.builds(), home.join(".cache/pwc/builds"));
        assert_eq!(d.target(), home.join(".cache/pwc/target"));
        assert_eq!(d.config_file(), home.join(".config/pwc/config.toml"));
    }

    #[test]
    fn xdg_variables_win_over_home() {
        let home = absolute("home");
        let data = absolute("data");
        let cache = absolute("cache");
        let home_text = home.to_string_lossy();
        let data_text = data.to_string_lossy();
        let cache_text = cache.to_string_lossy();
        let d = Dirs::from_vars(vars(&[
            ("HOME", &home_text),
            ("XDG_DATA_HOME", &data_text),
            ("XDG_CACHE_HOME", &cache_text),
            ("XDG_CONFIG_HOME", ""), // empty = unset
        ]))
        .unwrap();
        assert_eq!(d.data, data.join("pwc"));
        assert_eq!(d.cache, cache.join("pwc"));
        assert_eq!(d.config, home.join(".config/pwc"));

        // Relative XDG values are ignored.
        let d =
            Dirs::from_vars(vars(&[("HOME", &home_text), ("XDG_DATA_HOME", "relative")])).unwrap();
        assert_eq!(d.data, home.join(".local/share/pwc"));
    }

    #[test]
    fn pwc_home_overrides_everything() {
        let home = absolute("home");
        let data = absolute("data");
        let portable = absolute("portable");
        let home_text = home.to_string_lossy();
        let data_text = data.to_string_lossy();
        let portable_text = portable.to_string_lossy();
        let d = Dirs::from_vars(vars(&[
            ("HOME", &home_text),
            ("XDG_DATA_HOME", &data_text),
            ("PWC_HOME", &portable_text),
        ]))
        .unwrap();
        assert_eq!(d, Dirs::under(&portable));
        assert_eq!(d.data, portable.join("data"));

        let d = Dirs::from_vars(vars(&[("PWC_HOME", "rel")])).unwrap();
        assert!(d.data.is_absolute());
        assert!(d.data.ends_with("rel/data"));
    }

    #[test]
    fn no_home_is_an_error() {
        assert!(Dirs::from_vars_for_platform(vars(&[]), false).is_err());
        // Fine when every XDG variable is set.
        let data = absolute("d");
        let cache = absolute("c");
        let config = absolute("f");
        let data_text = data.to_string_lossy();
        let cache_text = cache.to_string_lossy();
        let config_text = config.to_string_lossy();
        let d = Dirs::from_vars_for_platform(
            vars(&[
                ("XDG_DATA_HOME", &data_text),
                ("XDG_CACHE_HOME", &cache_text),
                ("XDG_CONFIG_HOME", &config_text),
            ]),
            false,
        )
        .unwrap();
        assert_eq!(d.config, config.join("pwc"));
    }

    #[test]
    fn windows_defaults_under_local_app_data() {
        let local = absolute("local-app-data");
        let local_text = local.to_string_lossy();
        let d = Dirs::from_vars_for_platform(vars(&[("LOCALAPPDATA", &local_text)]), true).unwrap();
        assert_eq!(d, Dirs::under(&local.join("pwc")));

        let profile = absolute("profile");
        let profile_text = profile.to_string_lossy();
        let d =
            Dirs::from_vars_for_platform(vars(&[("USERPROFILE", &profile_text)]), true).unwrap();
        assert_eq!(d, Dirs::under(&profile.join("AppData/Local/pwc")));
    }

    #[test]
    fn from_env_works() {
        // Supported environments provide HOME/PWC_HOME or a Windows profile directory.
        let d = Dirs::from_env().unwrap();
        assert!(d.data.is_absolute());
    }
}
