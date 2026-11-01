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
///
/// This does not hide a console window. `pwc run` launches the game through it, and the game is
/// a console program. Toolchain processes use [`tool_command`].
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

/// [`wrapped_command`] for a toolchain process (`git` is not spawned here; `rustc`, `cargo` and
/// whatever wrapper prefix is configured). On Windows the process is created with
/// `CREATE_NO_WINDOW`, so Cargo does not open a console of its own.
pub(crate) fn tool_command(wrapper: &[String], program: impl AsRef<OsStr>) -> Command {
    let command = wrapped_command(wrapper, program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        /// `CREATE_NO_WINDOW`.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = command;
        command.creation_flags(CREATE_NO_WINDOW);
        return command;
    }
    #[cfg(not(windows))]
    command
}

/// Human-readable form of a wrapped invocation, for error messages.
fn describe(wrapper: &[String], program: &str) -> String {
    let mut parts: Vec<&str> = wrapper.iter().map(String::as_str).collect();
    parts.push(program);
    parts.join(" ")
}

pub(crate) fn rustc_info(wrapper: &[String]) -> Result<(String, String), BuildError> {
    let what = describe(wrapper, "rustc -vV");
    let output = tool_command(wrapper, "rustc")
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

/// Turn CR LF and a bare CR into LF. Text that is already LF is returned unchanged, so a Unix
/// toolchain's Build ID does not move.
pub(crate) fn normalize_line_endings(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains('\r') {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    std::borrow::Cow::Owned(out)
}

/// Split `rustc -vV` output (possibly preceded by wrapper noise) into (output from the `rustc `
/// line on, host triple). Line endings are normalized to LF first.
pub(crate) fn parse_rustc_verbose(stdout: &str) -> Option<(String, String)> {
    let stdout = normalize_line_endings(stdout);
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

/// `<target-dir>/<profile>`, or `<target-dir>/<triple>/<profile>` when Cargo was given `--target`.
pub(crate) fn artifact_dir(
    target_dir: &Path,
    profile: BuildProfile,
    explicit_target: Option<&str>,
) -> std::path::PathBuf {
    match explicit_target {
        Some(triple) => target_dir.join(triple).join(profile_dir(profile)),
        None => target_dir.join(profile_dir(profile)),
    }
}

/// `CARGO_BUILD_TARGET` when it names a plain target triple. Blank is unset.
///
/// A path, a space or `..` is rejected: Cargo would then write artifacts somewhere other than
/// the directory this function's caller looks in.
pub(crate) fn parse_cargo_build_target(
    value: Option<&str>,
) -> Result<Option<String>, &'static str> {
    let Some(value) = value else {
        return Ok(None);
    };
    let triple = value.trim();
    if triple.is_empty() {
        return Ok(None);
    }
    if !is_target_triple(triple) {
        return Err(
            "CARGO_BUILD_TARGET must be a target triple of ASCII letters, digits, `_`, `-` and `.` (no paths, spaces or `..`)",
        );
    }
    Ok(Some(triple.to_string()))
}

fn is_target_triple(triple: &str) -> bool {
    let bytes = triple.as_bytes();
    (1..=256).contains(&bytes.len())
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        && !triple.split('.').any(str::is_empty)
}

pub(crate) fn explicit_build_target(
    value: Option<&str>,
) -> Result<Option<String>, crate::BuildError> {
    parse_cargo_build_target(value)
        .map_err(|reason| crate::BuildError::Toolchain(reason.to_string()))
}

/// The `CARGO_BUILD_TARGET` of this process, parsed.
pub(crate) fn cargo_build_target_from_env() -> Result<Option<String>, crate::BuildError> {
    let value = match std::env::var("CARGO_BUILD_TARGET") {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(crate::BuildError::Toolchain(
                "CARGO_BUILD_TARGET is not valid Unicode".into(),
            ));
        }
    };
    explicit_build_target(value.as_deref())
}

pub(crate) fn compile(request: &BuildRequest, prepared: &PreparedBuild) -> Result<(), BuildError> {
    let explicit = cargo_build_target_from_env()?;
    if let Some(triple) = &explicit
        && triple != &prepared.target
    {
        return Err(crate::BuildError::Toolchain(format!(
            "CARGO_BUILD_TARGET is `{triple}`, but this build was prepared for target `{}`",
            prepared.target
        )));
    }

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
    let mut command = tool_command(&request.wrapper, "cargo");
    command
        .arg("build")
        .arg("--manifest-path")
        .arg(&manifest)
        .args(["-p", INSTANCE_PACKAGE, "--bin", GAME_BIN]);
    if request.golden {
        command.args(["--bin", GOLDEN_BIN]);
    }
    if request.profile == BuildProfile::Release {
        command.arg("--release");
    }
    if request.jobs > 0 {
        command.arg("-j").arg(request.jobs.to_string());
    }
    if let Some(triple) = &explicit {
        command.arg("--target").arg(triple);
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

    let from = artifact_dir(&target_dir, request.profile, explicit.as_deref());
    install(
        &from.join(exe_name(GAME_BIN)),
        &bin_path(&prepared.dir, GAME_BIN),
    )?;
    if request.golden {
        install(
            &from.join(exe_name(GOLDEN_BIN)),
            &bin_path(&prepared.dir, GOLDEN_BIN),
        )?;
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
