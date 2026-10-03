//! XDG directories (docs/spec/filesystem.md).

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
    /// From the environment: `PWC_HOME`, else `XDG_*_HOME`, else `~/.local/share`, `~/.cache`, `~/.config`.
    pub fn from_env() -> Result<Self, crate::InstanceError> {
        Self::from_vars(|name| std::env::var_os(name))
    }

    /// [`Dirs::from_env`] with the variables read through `var` (for tests and embedders).
    ///
    /// Empty variables count as unset. A relative `PWC_HOME` is taken relative to the current
    /// directory; relative `XDG_*_HOME` values are ignored, as the XDG specification requires.
    pub fn from_vars(var: impl Fn(&str) -> Option<OsString>) -> Result<Self, crate::InstanceError> {
        let get = |name: &str| var(name).filter(|v| !v.is_empty()).map(PathBuf::from);
        if let Some(home) = get("PWC_HOME") {
            let home = std::path::absolute(&home)
                .map_err(|source| crate::InstanceError::Io { path: home, source })?;
            return Ok(Self::under(&home));
        }
        let home = get("HOME");
        let base = |xdg: &str, default: &str| -> Result<PathBuf, crate::InstanceError> {
            match get(xdg).filter(|p| p.is_absolute()) {
                Some(dir) => Ok(dir),
                None => home.as_ref().map(|h| h.join(default)).ok_or_else(|| {
                    crate::InstanceError::Config(format!(
                        "cannot locate the PWC directories: neither PWC_HOME, {xdg} nor HOME is set"
                    ))
                }),
            }
        };
        Ok(Self {
            data: base("XDG_DATA_HOME", ".local/share")?.join("pwc"),
            cache: base("XDG_CACHE_HOME", ".cache")?.join("pwc"),
            config: base("XDG_CONFIG_HOME", ".config")?.join("pwc"),
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

    #[test]
    fn defaults_under_home() {
        let d = Dirs::from_vars(vars(&[("HOME", "/home/u")])).unwrap();
        assert_eq!(d.data, Path::new("/home/u/.local/share/pwc"));
        assert_eq!(d.cache, Path::new("/home/u/.cache/pwc"));
        assert_eq!(d.config, Path::new("/home/u/.config/pwc"));
        assert_eq!(d.store(), Path::new("/home/u/.local/share/pwc/store"));
        assert_eq!(
            d.instances(),
            Path::new("/home/u/.local/share/pwc/instances")
        );
        assert_eq!(d.builds(), Path::new("/home/u/.cache/pwc/builds"));
        assert_eq!(d.target(), Path::new("/home/u/.cache/pwc/target"));
        assert_eq!(
            d.config_file(),
            Path::new("/home/u/.config/pwc/config.toml")
        );
    }

    #[test]
    fn xdg_variables_win_over_home() {
        let d = Dirs::from_vars(vars(&[
            ("HOME", "/home/u"),
            ("XDG_DATA_HOME", "/data"),
            ("XDG_CACHE_HOME", "/cache"),
            ("XDG_CONFIG_HOME", ""), // empty = unset
        ]))
        .unwrap();
        assert_eq!(d.data, Path::new("/data/pwc"));
        assert_eq!(d.cache, Path::new("/cache/pwc"));
        assert_eq!(d.config, Path::new("/home/u/.config/pwc"));

        // Relative XDG values are ignored.
        let d =
            Dirs::from_vars(vars(&[("HOME", "/home/u"), ("XDG_DATA_HOME", "relative")])).unwrap();
        assert_eq!(d.data, Path::new("/home/u/.local/share/pwc"));
    }

    #[test]
    fn pwc_home_overrides_everything() {
        let d = Dirs::from_vars(vars(&[
            ("HOME", "/home/u"),
            ("XDG_DATA_HOME", "/data"),
            ("PWC_HOME", "/portable"),
        ]))
        .unwrap();
        assert_eq!(d, Dirs::under(Path::new("/portable")));
        assert_eq!(d.data, Path::new("/portable/data"));

        let d = Dirs::from_vars(vars(&[("PWC_HOME", "rel")])).unwrap();
        assert!(d.data.is_absolute());
        assert!(d.data.ends_with("rel/data"));
    }

    #[test]
    fn no_home_is_an_error() {
        assert!(Dirs::from_vars(vars(&[])).is_err());
        // Fine when every XDG variable is set.
        let d = Dirs::from_vars(vars(&[
            ("XDG_DATA_HOME", "/d"),
            ("XDG_CACHE_HOME", "/c"),
            ("XDG_CONFIG_HOME", "/f"),
        ]))
        .unwrap();
        assert_eq!(d.config, Path::new("/f/pwc"));
    }

    #[test]
    fn from_env_works() {
        // Whatever the test environment is, it has HOME or PWC_HOME.
        let d = Dirs::from_env().unwrap();
        assert!(d.data.is_absolute());
    }
}
