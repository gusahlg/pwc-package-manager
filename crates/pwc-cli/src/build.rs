//! `pwc build`, `pwc run`, `pwc cache clean`.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::process::ExitCode;

use anyhow::{Context as _, Result, anyhow};
use pwc_builder::{BuildOutput, BuildRequest};
use pwc_instance::{Instance, LockOptions, PwcSource};
use pwc_manifest::{BuildProfile, Lockfile};

use crate::context::Ctx;
use crate::instance::relock;
use crate::output::count;

/// A build plus what running it needs.
struct Built {
    output: BuildOutput,
    instance: Instance,
    pwc: PwcSource,
    lock: Lockfile,
    wrapper: Vec<String>,
}

/// Re-lock the selected instance (conservatively: this picks up changed `path:` packages and a
/// moved PWC checkout, and validates everything), then build it or reuse the cached build.
fn build_instance(ctx: &Ctx, profile: Option<BuildProfile>, force: bool) -> Result<Built> {
    let instance = ctx.open_instance(None)?;
    let (report, pwc) = relock(ctx, &instance, &LockOptions::default())?;
    let lock = report.lock;

    let store = ctx.store()?;
    let mut packages = BTreeMap::new();
    for package in &lock.packages {
        let entry = store
            .get(&package.hash)
            .with_context(|| format!("cannot read store entry {}", package.hash))?
            .ok_or_else(|| {
                anyhow!(
                    "{} {} ({}) is missing from the store; run `pwc lock`",
                    package.id,
                    package.version,
                    package.hash
                )
            })?;
        packages.insert(package.id.clone(), (entry.dir, entry.manifest));
    }

    let profile = profile.unwrap_or(instance.manifest.profile);
    let wrapper = ctx.config.wrapper_command(&pwc.dir);
    let request = BuildRequest {
        lock: lock.clone(),
        packages,
        pwc_source: pwc.dir.clone(),
        pwc_dirty_digest: pwc
            .dirty_digest()
            .context("cannot compute the PWC source's dirty digest")?,
        profile,
        builds_dir: ctx.dirs.builds(),
        target_dir: ctx.dirs.target(),
        wrapper: wrapper.clone(),
        jobs: ctx.config.jobs,
        force,
    };
    let (rustc, target) = pwc_builder::rustc_info(&request.wrapper)?;
    let prepared = pwc_builder::prepare(&request, &rustc, &target)?;
    let name = &instance.manifest.name;
    let short = &prepared.build_id[..12];
    if prepared.cached {
        ctx.status(format!("Using the cached build {short} of `{name}`"));
    } else {
        ctx.status(format!(
            "Building `{name}` ({}, {}, build {short})",
            profile.as_str(),
            count(lock.packages.len(), "package")
        ));
        ctx.detail(format!("workspace {}", prepared.dir.display()));
        if !wrapper.is_empty() {
            ctx.detail(format!("wrapper {}", wrapper.join(" ")));
        }
    }
    let output = pwc_builder::compile(&request, &prepared)?;
    Ok(Built {
        output,
        instance,
        pwc,
        lock,
        wrapper,
    })
}

pub(crate) fn build(
    ctx: &Ctx,
    profile: Option<BuildProfile>,
    force: bool,
    print_dir: bool,
) -> Result<()> {
    let built = build_instance(ctx, profile, force)?;
    if print_dir {
        println!("{}", built.output.dir.display());
    } else {
        println!("{}", built.output.game.display());
    }
    Ok(())
}

/// Build, then run the game (or the golden harness) with the instance's environment
/// (docs/spec/filesystem.md, "Game data per instance"). The executable runs behind the same
/// wrapper Cargo ran behind, so the runtime libraries of the build environment (the Vulkan
/// loader, Wayland/X11) are found; the working directory is left unchanged.
pub(crate) fn run(
    ctx: &Ctx,
    golden: bool,
    profile: Option<BuildProfile>,
    args: &[OsString],
) -> Result<ExitCode> {
    let built = build_instance(ctx, profile, false)?;
    let exe = if golden {
        &built.output.golden
    } else {
        &built.output.game
    };
    let game_dir = built.instance.game_dir();
    std::fs::create_dir_all(&game_dir)
        .with_context(|| format!("cannot create {}", game_dir.display()))?;

    let mut command = pwc_builder::wrapped_command(&built.wrapper, exe);
    command
        .args(args)
        .env("WATT_DATA_DIR", &game_dir)
        .env("WATT_ASSET_DIR", built.pwc.dir.join("assets"))
        .env("PWC_INSTANCE", built.instance.manifest.name.as_str())
        .env("PWC_ENVIRONMENT", &built.lock.environment);
    if golden {
        let goldens = built.instance.dir.join("goldens");
        std::fs::create_dir_all(&goldens)
            .with_context(|| format!("cannot create {}", goldens.display()))?;
        command.env("WATT_GOLDEN_DIR", goldens);
    }
    ctx.detail(format!("running {}", exe.display()));
    exec(command, exe)
}

#[cfg(unix)]
fn exec(mut command: std::process::Command, exe: &std::path::Path) -> Result<ExitCode> {
    use std::os::unix::process::CommandExt as _;
    let err = command.exec();
    Err(err).with_context(|| format!("cannot run {}", exe.display()))
}

#[cfg(not(unix))]
fn exec(mut command: std::process::Command, exe: &std::path::Path) -> Result<ExitCode> {
    let status = command
        .status()
        .with_context(|| format!("cannot run {}", exe.display()))?;
    Ok(match status.code() {
        Some(0) => ExitCode::SUCCESS,
        Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        None => ExitCode::FAILURE,
    })
}

pub(crate) fn cache_clean(ctx: &Ctx) -> Result<()> {
    let (builds, target) = (ctx.dirs.builds(), ctx.dirs.target());
    pwc_builder::clean(&builds, &target)?;
    ctx.status(format!(
        "Removed {} and {}",
        builds.display(),
        target.display()
    ));
    Ok(())
}
