//! Running the toolchain: `rustc -vV` and `cargo build`, behind the configured wrapper.

use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Stdio};

use pwc_manifest::BuildProfile;

use crate::{
    BuildError, BuildRequest, GAME_BIN, GOLDEN_BIN, INSTANCE_PACKAGE, PreparedBuild, absolute,
    bin_path, exe_name, io_err,
};

/// The lock file serialising builds that share a target directory: Cargo locks the target
/// directory itself, but the executables must also be copied out before another build replaces
/// them.
const TARGET_LOCK: &str = ".pwc-build.lock";

/// `program` behind `wrapper` (`[]` = run it directly; otherwise `wrapper[0] wrapper[1..] program`).
pub fn wrapped_command(wrapper: &[String], program: impl AsRef<OsStr>) -> Command {
    match wrapper.split_first() {
        Some((first, rest)) => {
            let mut command = Command::new(first);
            command.args(rest).arg(program);
            command
        }
        None => Command::new(program),
    }
}

/// Human-readable form of a wrapped invocation, for error messages.
fn describe(wrapper: &[String], program: &str) -> String {
    let mut parts: Vec<&str> = wrapper.iter().map(String::as_str).collect();
    parts.push(program);
    parts.join(" ")
}

pub(crate) fn rustc_info(wrapper: &[String]) -> Result<(String, String), BuildError> {
    let what = describe(wrapper, "rustc -vV");
    let output = wrapped_command(wrapper, "rustc")
        .arg("-vV")
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| BuildError::Toolchain(format!("cannot run `{what}`: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = stderr
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        return Err(BuildError::Toolchain(format!(
            "`{what}` failed with {}: {}",
            output.status,
            tail.join(" / ")
        )));
    }
    parse_rustc_verbose(&String::from_utf8_lossy(&output.stdout)).ok_or_else(|| {
        BuildError::Toolchain(format!("`{what}` printed no `rustc …` / `host: …` lines"))
    })
}

/// Split `rustc -vV` output (possibly preceded by wrapper noise) into (output from the `rustc `
/// line on, host triple).
pub(crate) fn parse_rustc_verbose(stdout: &str) -> Option<(String, String)> {
    let start = stdout
        .match_indices("rustc ")
        .map(|(i, _)| i)
        .find(|&i| i == 0 || stdout.as_bytes()[i - 1] == b'\n')?;
    let verbose = &stdout[start..];
    let host = verbose
        .lines()
        .find_map(|l| l.strip_prefix("host: "))?
        .trim();
    if host.is_empty() {
        return None;
    }
    Some((verbose.to_owned(), host.to_owned()))
}

/// The directory Cargo puts a profile's final artifacts in.
fn profile_dir(profile: BuildProfile) -> &'static str {
    match profile {
        BuildProfile::Release => "release",
        BuildProfile::Dev => "debug",
    }
}

pub(crate) fn compile(request: &BuildRequest, prepared: &PreparedBuild) -> Result<(), BuildError> {
    let target_dir = absolute(&request.target_dir)?;
    std::fs::create_dir_all(&target_dir).map_err(io_err(&target_dir))?;
    let lock_path = target_dir.join(TARGET_LOCK);
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(io_err(&lock_path))?;
    lock.lock().map_err(io_err(&lock_path))?;

    let manifest = prepared.dir.join("Cargo.toml");
    let mut command = wrapped_command(&request.wrapper, "cargo");
    command
        .arg("build")
        .arg("--manifest-path")
        .arg(&manifest)
        .args(["--bins", "-p", INSTANCE_PACKAGE]);
    if request.profile == BuildProfile::Release {
        command.arg("--release");
    }
    if request.jobs > 0 {
        command.arg("-j").arg(request.jobs.to_string());
    }
    // Cargo's own output goes to stderr (stdout is for results such as the executable path); the
    // working directory is the build directory so no stray `.cargo/config.toml` applies.
    command
        .env("CARGO_TARGET_DIR", &target_dir)
        .current_dir(&prepared.dir)
        .stdin(Stdio::null())
        .stdout(std::io::stderr());
    let what = describe(&request.wrapper, "cargo build");
    let status = command
        .status()
        .map_err(|e| BuildError::Toolchain(format!("cannot run `{what}`: {e}")))?;
    if !status.success() {
        return Err(BuildError::Cargo(format!(
            "{status} (generated workspace: {})",
            prepared.dir.display()
        )));
    }

    let from = target_dir.join(profile_dir(request.profile));
    for name in [GAME_BIN, GOLDEN_BIN] {
        install(&from.join(exe_name(name)), &bin_path(&prepared.dir, name))?;
    }
    drop(lock);
    Ok(())
}

/// Copy `src` (`<target>/<profile>/<name>`) to `dst` atomically.
fn install(src: &Path, dst: &Path) -> Result<(), BuildError> {
    if !src.is_file() {
        return Err(BuildError::Cargo(format!(
            "success, but {} was not produced",
            src.display()
        )));
    }
    let parent = dst.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(io_err(parent))?;
    let tmp = parent.join(format!(
        ".{}.tmp",
        dst.file_name().unwrap_or_default().to_string_lossy()
    ));
    std::fs::copy(src, &tmp).map_err(io_err(src))?;
    std::fs::rename(&tmp, dst).map_err(io_err(dst))
}
