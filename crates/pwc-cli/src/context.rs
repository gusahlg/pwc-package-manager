//! What every command needs: directories, configuration, output switches.

use std::fmt::Display;
use std::path::Path;

use anyhow::{Context as _, Result, anyhow};
use pwc_instance::{Config, Dirs, Instance, InstanceError, PwcSource};
use pwc_manifest::InstanceName;
use pwc_store::Store;

/// The command context.
pub(crate) struct Ctx {
    /// Platform state (or `PWC_HOME`) directories.
    pub dirs: Dirs,
    /// `config.toml` (defaults when it does not exist yet).
    pub config: Config,
    /// `--instance`.
    pub instance: Option<String>,
    /// `-v`.
    pub verbose: bool,
    /// `-q`.
    pub quiet: bool,
}

impl Ctx {
    /// Read the directories and the configuration.
    pub fn new(instance: Option<String>, verbose: bool, quiet: bool) -> Result<Self> {
        let dirs = Dirs::from_env().context("cannot determine the pwc directories")?;
        let path = dirs.config_file();
        let config =
            Config::load(&path).with_context(|| format!("cannot read {}", path.display()))?;
        Ok(Self {
            dirs,
            config,
            instance,
            verbose,
            quiet,
        })
    }

    /// A progress or status line on stderr (silenced by `-q`).
    pub fn status(&self, message: impl Display) {
        if !self.quiet {
            eprintln!("{message}");
        }
    }

    /// A detail line on stderr (only with `-v`).
    pub fn detail(&self, message: impl Display) {
        if self.verbose {
            eprintln!("{message}");
        }
    }

    /// A warning on stderr (silenced by `-q`).
    pub fn warn(&self, message: impl Display) {
        if !self.quiet {
            eprintln!("warning: {message}");
        }
    }

    /// Write `config.toml`.
    pub fn save_config(&self) -> Result<()> {
        let path = self.dirs.config_file();
        self.config
            .save(&path)
            .with_context(|| format!("cannot write {}", path.display()))
    }

    /// The package store.
    pub fn store(&self) -> Result<Store> {
        let root = self.dirs.store();
        Store::open(&root).with_context(|| format!("cannot open the store at {}", root.display()))
    }

    /// The configured PWC source directory.
    pub fn pwc_dir(&self) -> Result<&Path> {
        self.config
            .pwc_source
            .as_deref()
            .ok_or_else(|| anyhow!("no PWC source configured (run `pwc setup`)"))
    }

    /// The configured PWC source, inspected.
    pub fn pwc_source(&self) -> Result<PwcSource> {
        let dir = self.pwc_dir()?;
        PwcSource::open(dir)
            .with_context(|| format!("cannot use the PWC source at {}", dir.display()))
    }

    /// `explicit`, else `--instance`, else the active instance.
    pub fn instance_name(&self, explicit: Option<&str>) -> Result<InstanceName> {
        let name = explicit
            .or(self.instance.as_deref())
            .or(self.config.active_instance.as_deref())
            .ok_or(InstanceError::NoActiveInstance)?;
        InstanceName::parse(name).with_context(|| format!("invalid instance name `{name}`"))
    }

    /// Open the selected instance.
    pub fn open_instance(&self, explicit: Option<&str>) -> Result<Instance> {
        let name = self.instance_name(explicit)?;
        Ok(Instance::open(&self.dirs, &name)?)
    }
}
