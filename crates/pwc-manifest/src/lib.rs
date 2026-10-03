//! The PWC manifest formats: what a mod package declares (`mod.toml`), what an instance wants
//! (`instance.toml`) and what it resolved to (`pwc.lock`).
//!
//! Specifications: `docs/spec/mod-manifest.md`, `docs/spec/instances.md`. This crate parses,
//! validates and serialises; it touches no filesystem except through the `*_file` helpers, and it
//! knows nothing about stores, resolution or Cargo.
//!
//! Every type round-trips: `parse(to_toml_string(x)) == x`.

mod id;
mod instance;
mod license;
mod lock;
mod manifest;
mod text;

pub use id::{InstanceName, PackageId};
pub use instance::{BuildProfile, InstanceManifest, ModRequirement, ModSource};
pub use license::{
    ALLOWED_EXCEPTIONS, CONTENT_LICENSES, LicenseError, SOFTWARE_LICENSES,
    check_license_expression, check_package_license,
};
pub use lock::{LOCK_HEADER, LockedPackage, LockedPwc, Lockfile, PackageSource, environment_hash};
pub use manifest::{Category, Edition, ModKind, ModManifest, PackageMetadata};
pub use text::{MAX_PATH_BYTES, check_package_path};

/// Re-exported so every crate names versions with the same types.
pub use semver::{Version, VersionReq};

/// The manifest format this crate reads and writes (`format = 1`).
pub const FORMAT: u32 = 1;

/// A package hash: `sha256:` followed by 64 lowercase hex digits.
#[derive(
    Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, serde::Serialize, serde::Deserialize,
)]
#[serde(try_from = "String", into = "String")]
pub struct PackageHash(String);

impl PackageHash {
    /// From the raw 32-byte digest.
    pub fn from_digest(digest: [u8; 32]) -> Self {
        let mut s = String::with_capacity(7 + 64);
        s.push_str("sha256:");
        for b in digest {
            s.push_str(&format!("{b:02x}"));
        }
        Self(s)
    }

    /// Parse `sha256:<64 lowercase hex>`.
    pub fn parse(s: &str) -> Result<Self, ManifestError> {
        let hex = s
            .strip_prefix("sha256:")
            .ok_or_else(|| ManifestError::invalid("hash", s, "must start with `sha256:`"))?;
        if hex.len() != 64
            || !hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ManifestError::invalid(
                "hash",
                s,
                "must be 64 lowercase hex digits after `sha256:`",
            ));
        }
        Ok(Self(s.to_string()))
    }

    /// The full `sha256:…` string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The 64 hex digits (the store directory name).
    pub fn hex(&self) -> &str {
        &self.0[7..]
    }
}

impl std::fmt::Display for PackageHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for PackageHash {
    type Error = ManifestError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<PackageHash> for String {
    fn from(h: PackageHash) -> String {
        h.0
    }
}

/// Why a manifest, instance or lock file was rejected. Messages name the offending key.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// Not valid TOML, or the wrong shape (unknown key, wrong type).
    #[error("{0}")]
    Toml(String),
    /// `format` is missing or not supported.
    #[error(
        "unsupported format {found}; this pwc reads format {FORMAT} (upgrade pwc for newer formats)"
    )]
    Format {
        /// The format found in the file.
        found: u32,
    },
    /// A key is missing.
    #[error("missing required key `{0}`")]
    Missing(&'static str),
    /// A value breaks a rule.
    #[error("invalid `{key}` = {value:?}: {reason}")]
    Invalid {
        /// The key (dotted path, e.g. `package.id`).
        key: String,
        /// The offending value.
        value: String,
        /// The rule it breaks.
        reason: String,
    },
    /// The licence expression breaks the licence policy.
    #[error("licence policy: {0}")]
    License(#[from] LicenseError),
    /// Reading or writing a file failed.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: std::path::PathBuf,
        /// The I/O error.
        source: std::io::Error,
    },
}

impl ManifestError {
    /// Shorthand for [`ManifestError::Invalid`].
    pub fn invalid(
        key: impl Into<String>,
        value: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::Invalid {
            key: key.into(),
            value: value.into(),
            reason: reason.into(),
        }
    }
}
