//! Instances and the environment around them (docs/spec/instances.md, docs/spec/filesystem.md):
//! XDG directories, `config.toml`, repositories, and the locking operation that turns an
//! `instance.toml` into a `pwc.lock` (collect candidates → resolve → fill the store → write).

mod config;
mod dirs;
mod fsutil;
mod instance;
mod pwc_source;
mod repository;

#[cfg(test)]
mod testutil;

pub use config::{Config, NO_WRAPPER_VAR, RepositoryConfig, Wrapper};
pub use dirs::Dirs;
pub use instance::{
    Instance, LockOptions, LockReport, installed_packages, lock_instance, referenced_hashes,
};
pub use pwc_source::{PwcSource, UNKNOWN_REVISION};
pub use repository::{Available, AvailablePackage, AvailableSource};

/// Errors from this crate.
#[derive(Debug, thiserror::Error)]
pub enum InstanceError {
    /// A manifest, instance or lock file is invalid.
    #[error(transparent)]
    Manifest(#[from] pwc_manifest::ManifestError),
    /// A package is invalid.
    #[error(transparent)]
    Package(#[from] pwc_package::PackageError),
    /// A manifest or lock file at a path is invalid.
    #[error("{path}: {source}")]
    ManifestAt {
        /// The file.
        path: std::path::PathBuf,
        /// Why it was rejected.
        source: pwc_manifest::ManifestError,
    },
    /// The package at a path is invalid.
    #[error("{path}: {source}")]
    PackageAt {
        /// The source directory or `.pwcmod`.
        path: std::path::PathBuf,
        /// Why it was rejected.
        source: pwc_package::PackageError,
    },
    /// The store failed.
    #[error(transparent)]
    Store(#[from] pwc_store::StoreError),
    /// Resolution failed.
    #[error(transparent)]
    Resolve(#[from] pwc_resolver::ResolveError),
    /// A published version (not a `path` tree) no longer has the contents it was locked with.
    #[error(
        "{id} {version} changed contents without a version bump (locked {locked}, now {found}); \
         published versions are immutable — bump the version"
    )]
    ContentsChanged {
        /// Id.
        id: pwc_manifest::PackageId,
        /// Version.
        version: pwc_manifest::Version,
        /// The hash in `pwc.lock`.
        locked: pwc_manifest::PackageHash,
        /// The hash of what is offered now.
        found: pwc_manifest::PackageHash,
    },
    /// A locked package is missing from the store.
    #[error("{id} ({hash}) is not in the store; run `pwc lock` to reinstall it")]
    NotInstalled {
        /// Id.
        id: pwc_manifest::PackageId,
        /// Its hash.
        hash: pwc_manifest::PackageHash,
    },
    /// No such instance.
    #[error("no instance named `{0}` (create it with `pwc instance create {0}`)")]
    NoInstance(String),
    /// Already exists.
    #[error("instance `{0}` already exists")]
    InstanceExists(String),
    /// No active instance and none given.
    #[error("no instance selected (use --instance or `pwc instance use <name>`)")]
    NoActiveInstance,
    /// The PWC source is missing or not a PWC checkout.
    #[error("PWC source: {0}")]
    PwcSource(String),
    /// Configuration problem.
    #[error("config: {0}")]
    Config(String),
    /// I/O.
    #[error("{path}: {source}")]
    Io {
        /// Path.
        path: std::path::PathBuf,
        /// Error.
        source: std::io::Error,
    },
}
