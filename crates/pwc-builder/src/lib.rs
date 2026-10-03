//! The PWC builder (docs/spec/build.md): given an exact lockfile and the store directories of its
//! packages, generate a Cargo workspace (the instance crate and the mod bundle), compute the Build
//! ID, invoke Cargo, and keep the result in the build cache. It never chooses versions and never
//! reads mod code.
//!
//! The pipeline is split so front ends can report progress between the steps:
//!
//! 1. [`rustc_info`] — the toolchain identity (`rustc -vV`, behind the wrapper) and host triple;
//! 2. [`prepare`] — Build ID, cache lookup, and (on a miss) the generated workspace on disk;
//! 3. [`compile`] — Cargo, then the executables copied into `<build dir>/bin/`.
//!
//! [`build`] runs all three. [`generate`] and [`build_id`] are pure enough to test without Cargo.

mod cargo;
mod generate;
mod metadata;
mod source;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pwc_manifest::{BuildProfile, Lockfile, ModManifest, PackageId};
use sha2::{Digest, Sha256};

pub use cargo::wrapped_command;

/// The builder version recorded in every Build ID (`builder <version>`).
pub const BUILDER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Cargo package name of the generated instance crate (`cargo build -p <this>`). Named so it
/// cannot be confused with the tooling crate `pwc-instance`.
pub const INSTANCE_PACKAGE: &str = "pwc-game-instance";

/// Cargo package name of the generated mod bundle (library `pwc_bundle`).
pub const BUNDLE_PACKAGE: &str = "pwc-bundle";

/// The game executable's name (without the platform suffix).
pub const GAME_BIN: &str = "pwc-game";

/// The golden-harness executable's name (without the platform suffix).
pub const GOLDEN_BIN: &str = "pwc-golden";

/// Everything a build needs. Plain data: the caller (pwc-instance / pwc-cli) gathers it.
#[derive(Clone, Debug)]
pub struct BuildRequest {
    /// The exact lock.
    pub lock: Lockfile,
    /// Store directory and manifest of every package in the lock.
    pub packages: BTreeMap<PackageId, (PathBuf, ModManifest)>,
    /// PWC source checkout (`lock.pwc.source`, possibly overridden).
    pub pwc_source: PathBuf,
    /// Digest of uncommitted PWC changes (`clean` for a clean checkout); computed by the caller
    /// (`pwc_instance::PwcSource::dirty_digest`).
    pub pwc_dirty_digest: String,
    /// Profile.
    pub profile: BuildProfile,
    /// `$XDG_CACHE_HOME/pwc/builds`.
    pub builds_dir: PathBuf,
    /// Shared `CARGO_TARGET_DIR`.
    pub target_dir: PathBuf,
    /// Command prefix for Cargo (empty = none).
    pub wrapper: Vec<String>,
    /// Cargo `-j` (0 = default).
    pub jobs: u32,
    /// Rebuild even when cached.
    pub force: bool,
}

/// A finished (or cached) build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildOutput {
    /// The Build ID (64 hex digits).
    pub build_id: String,
    /// `builds/<id prefix>/`.
    pub dir: PathBuf,
    /// The game executable.
    pub game: PathBuf,
    /// The golden-harness executable.
    pub golden: PathBuf,
    /// Whether the cache answered.
    pub cached: bool,
}

/// A build whose workspace is on disk (or whose cached result is ready), before Cargo runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedBuild {
    /// The Build ID (64 hex digits).
    pub build_id: String,
    /// `builds/<id prefix>/`.
    pub dir: PathBuf,
    /// Whether the cache already holds a finished build (then [`compile`] does not run Cargo).
    pub cached: bool,
    /// The `rustc -vV` output the ID was computed from.
    pub rustc: String,
    /// The target triple.
    pub target: String,
}

impl PreparedBuild {
    /// Where the game executable is (or will be) once built.
    pub fn game(&self) -> PathBuf {
        bin_path(&self.dir, GAME_BIN)
    }

    /// Where the golden-harness executable is (or will be) once built.
    pub fn golden(&self) -> PathBuf {
        bin_path(&self.dir, GOLDEN_BIN)
    }

    fn output(&self) -> BuildOutput {
        BuildOutput {
            build_id: self.build_id.clone(),
            dir: self.dir.clone(),
            game: self.game(),
            golden: self.golden(),
            cached: self.cached,
        }
    }
}

/// The generated files of a build, before anything is written (testable without Cargo).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedWorkspace {
    /// Package-relative path → file contents.
    pub files: BTreeMap<String, String>,
}

/// Compute the Build ID (needs `rustc -vV` output; pass it in for determinism in tests).
///
/// The formula is fixed by docs/spec/build.md: the SHA-256 of the builder version, the lock's
/// environment, the PWC revision and dirty digest, the `rustc -vV` output (newlines replaced by
/// spaces), the profile and the target triple.
pub fn build_id(request: &BuildRequest, rustc_version: &str, target: &str) -> String {
    let text = format!(
        "pwc-build 1\nbuilder {BUILDER_VERSION}\nenvironment {}\npwc-source {} {}\nrustc {}\nprofile {}\ntarget {}\n",
        request.lock.environment,
        request.lock.pwc.revision,
        request.pwc_dirty_digest,
        rustc_version.replace('\n', " "),
        request.profile.as_str(),
        target,
    );
    hex(&Sha256::digest(text.as_bytes()))
}

/// The build directory name for a Build ID: its first 32 hex digits.
pub fn build_dir_name(build_id: &str) -> &str {
    &build_id[..build_id.len().min(32)]
}

/// Generate the workspace files for `request` (registration order: dependencies first, ties by id).
///
/// Reads the PWC source's `Cargo.toml` (profiles, version), `crates/pwc-mod-api/Cargo.toml` (API
/// version) and `Cargo.lock` (the seed), and checks them and every package against the lock.
pub fn generate(request: &BuildRequest) -> Result<GeneratedWorkspace, BuildError> {
    generate::generate(request)
}

/// Compute the Build ID, look it up in the cache and, unless a finished build is cached (and
/// `request.force` is false), validate the inputs and write the generated workspace.
pub fn prepare(
    request: &BuildRequest,
    rustc: &str,
    target: &str,
) -> Result<PreparedBuild, BuildError> {
    let builds_dir = absolute(&request.builds_dir)?;
    let id = build_id(request, rustc, target);
    let dir = builds_dir.join(build_dir_name(&id));
    let mut prepared = PreparedBuild {
        build_id: id,
        dir,
        cached: false,
        rustc: rustc.to_owned(),
        target: target.to_owned(),
    };
    if !request.force && metadata::is_finished(&prepared) {
        prepared.cached = true;
        return Ok(prepared);
    }
    generate::check_store_dirs(request)?;
    let workspace = generate(request)?;
    generate::write_workspace(&prepared.dir, &workspace)?;
    metadata::write(request, &prepared, None)?;
    Ok(prepared)
}

/// Run Cargo for a prepared build and install its executables into `<build dir>/bin/` (a no-op
/// returning the cached output when `prepared.cached`).
pub fn compile(
    request: &BuildRequest,
    prepared: &PreparedBuild,
) -> Result<BuildOutput, BuildError> {
    if prepared.cached {
        return Ok(prepared.output());
    }
    cargo::compile(request, prepared)?;
    metadata::write(request, prepared, Some(std::time::SystemTime::now()))?;
    Ok(prepared.output())
}

/// Build (or return the cached build).
pub fn build(request: &BuildRequest) -> Result<BuildOutput, BuildError> {
    let (rustc, target) = rustc_info(&request.wrapper)?;
    let prepared = prepare(request, &rustc, &target)?;
    compile(request, &prepared)
}

/// `rustc -vV` (via the wrapper) and the host target triple.
///
/// Anything the wrapper prints before rustc's first line (a Nix shell hook, for example) is
/// dropped, so the result is the same with and without a wrapper.
pub fn rustc_info(wrapper: &[String]) -> Result<(String, String), BuildError> {
    cargo::rustc_info(wrapper)
}

/// Delete build outputs and the shared target directory.
pub fn clean(builds_dir: &Path, target_dir: &Path) -> Result<(), BuildError> {
    for dir in [builds_dir, target_dir] {
        match std::fs::remove_dir_all(dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(BuildError::Io {
                    path: dir.to_path_buf(),
                    source,
                });
            }
        }
    }
    Ok(())
}

/// Build failures.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// The PWC source does not match the lock or lacks the runtime/API crates.
    #[error("PWC source: {0}")]
    Source(String),
    /// A package's store directory is missing or does not match.
    #[error("package {0}: {1}")]
    Package(PackageId, String),
    /// The lock cannot be built as it is (dangling dependency, cycle, clashing crate names).
    #[error("lock: {0}")]
    Lock(String),
    /// `rustc` could not be run or its output was not understood.
    #[error("toolchain: {0}")]
    Toolchain(String),
    /// Cargo failed.
    #[error("cargo failed with {0}")]
    Cargo(String),
    /// I/O.
    #[error("{path}: {source}")]
    Io {
        /// Path.
        path: PathBuf,
        /// Error.
        source: std::io::Error,
    },
}

/// Lowercase hex of a digest.
pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// `std::path::absolute` with this crate's error type.
pub(crate) fn absolute(path: &Path) -> Result<PathBuf, BuildError> {
    std::path::absolute(path).map_err(io_err(path))
}

/// `<name><EXE_SUFFIX>`.
pub(crate) fn exe_name(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

/// `<dir>/bin/<name><EXE_SUFFIX>`.
pub(crate) fn bin_path(dir: &Path, name: &str) -> PathBuf {
    dir.join("bin").join(exe_name(name))
}

/// Shorthand for an I/O error at `path`.
pub(crate) fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> BuildError + '_ {
    move |source| BuildError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests;
