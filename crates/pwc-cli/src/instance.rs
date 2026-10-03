//! `pwc instance …`, `pwc mod …`, `pwc lock`, `pwc env`.

use std::collections::BTreeSet;
use std::io::{BufRead as _, IsTerminal as _, Write as _};
use std::path::Path;

use anyhow::{Context as _, Result, bail};
use pwc_instance::{Instance, LockOptions, LockReport, PwcSource, lock_instance};
use pwc_manifest::{
    InstanceManifest, InstanceName, ModManifest, ModRequirement, ModSource, PackageId, VersionReq,
};
use pwc_package::Package;

use crate::config::canonical;
use crate::context::Ctx;
use crate::output::{count, print_table, requirement};

/// Lock `instance` against the configured PWC source and report what changed.
pub(crate) fn relock(
    ctx: &Ctx,
    instance: &Instance,
    options: &LockOptions,
) -> Result<(LockReport, PwcSource)> {
    let pwc = ctx.pwc_source()?;
    let report = lock_instance(&ctx.dirs, &ctx.config, instance, &pwc, options)
        .with_context(|| format!("cannot lock instance `{}`", instance.manifest.name))?;
    for warning in &report.warnings {
        ctx.warn(warning);
    }
    for (id, old, new) in &report.changes {
        match (old, new) {
            (None, Some(new)) => ctx.status(format!("  + {id} {new}")),
            (Some(old), None) => ctx.status(format!("  - {id} {old}")),
            (Some(old), Some(new)) => ctx.status(format!("  ~ {id} {old} -> {new}")),
            (None, None) => {}
        }
    }
    Ok((report, pwc))
}

/// Save `instance` (already modified) and re-lock; on failure restore `original` so a failed
/// `mod add/remove` leaves `instance.toml` as it was.
fn save_and_relock(
    ctx: &Ctx,
    instance: &mut Instance,
    original: InstanceManifest,
) -> Result<LockReport> {
    instance.save().context("cannot write instance.toml")?;
    match relock(ctx, instance, &LockOptions::default()) {
        Ok((report, _)) => Ok(report),
        Err(err) => {
            instance.manifest = original;
            if let Err(restore) = instance.save() {
                return Err(err.context(format!("also failed to restore instance.toml: {restore}")));
            }
            Err(err)
        }
    }
}

fn locked_summary(ctx: &Ctx, report: &LockReport) {
    if report.changes.is_empty() {
        ctx.status(format!(
            "Lock unchanged ({})",
            count(report.lock.packages.len(), "package")
        ));
    } else {
        ctx.status(format!(
            "Locked {}",
            count(report.lock.packages.len(), "package")
        ));
    }
    ctx.detail(format!("environment {}", report.lock.environment));
}

pub(crate) fn create(mut ctx: Ctx, name: &str, pwc: Option<&str>, use_it: bool) -> Result<()> {
    let name =
        InstanceName::parse(name).with_context(|| format!("invalid instance name `{name}`"))?;
    let req = pwc.unwrap_or("*");
    let req = VersionReq::parse(req).with_context(|| format!("invalid PWC requirement `{req}`"))?;
    let instance = Instance::create(&ctx.dirs, InstanceManifest::new(name.clone(), req))?;
    ctx.status(format!(
        "Created instance `{name}` ({})",
        instance.dir.display()
    ));
    if use_it {
        ctx.config.active_instance = Some(name.to_string());
        ctx.save_config()?;
        ctx.status(format!("`{name}` is now the active instance"));
    }
    Ok(())
}

pub(crate) fn list(ctx: &Ctx) -> Result<()> {
    let instances = Instance::list(&ctx.dirs)?;
    if instances.is_empty() {
        ctx.status("No instances (create one with `pwc instance create <name> --use`).");
        return Ok(());
    }
    let active = ctx.config.active_instance.as_deref();
    let rows: Vec<Vec<String>> = instances
        .iter()
        .map(|i| {
            let m = &i.manifest;
            let marker = if active == Some(m.name.as_str()) {
                "*"
            } else {
                ""
            };
            vec![
                marker.to_owned(),
                m.name.to_string(),
                m.mods.len().to_string(),
                m.pwc.to_string(),
                m.description.clone().unwrap_or_default(),
            ]
        })
        .collect();
    print_table(&["", "NAME", "MODS", "PWC", "DESCRIPTION"], &rows);
    Ok(())
}

pub(crate) fn use_instance(mut ctx: Ctx, name: &str) -> Result<()> {
    let instance = ctx.open_instance(Some(name))?;
    ctx.config.active_instance = Some(instance.manifest.name.to_string());
    ctx.save_config()?;
    ctx.status(format!(
        "`{}` is now the active instance",
        instance.manifest.name
    ));
    Ok(())
}

pub(crate) fn show(ctx: &Ctx, name: Option<&str>) -> Result<()> {
    let instance = ctx.open_instance(name)?;
    let m = &instance.manifest;
    let active = if ctx.config.active_instance.as_deref() == Some(m.name.as_str()) {
        " (active)"
    } else {
        ""
    };
    println!("instance  {}{active}", m.name);
    println!("dir       {}", instance.dir.display());
    if let Some(description) = &m.description {
        println!("about     {description}");
    }
    println!("pwc       {}", m.pwc);
    println!("profile   {}", m.profile.as_str());
    println!();
    if m.mods.is_empty() {
        println!("requirements: none");
    } else {
        println!("requirements:");
        let rows: Vec<Vec<String>> = m
            .mods
            .iter()
            .map(|(id, req)| vec![format!("  {id}"), requirement(req)])
            .collect();
        print!("{}", crate::output::table(&["  ID", "REQUIREMENT"], &rows));
    }
    println!();
    match instance.lockfile()? {
        None => println!("not locked yet (run `pwc lock`)"),
        Some(lock) => {
            let pwc = &lock.pwc;
            let dirty = if pwc.dirty {
                ", uncommitted changes"
            } else {
                ""
            };
            println!(
                "locked: PWC {} (pwc-mod-api {}, revision {}{dirty})",
                pwc.version, pwc.api, pwc.revision
            );
            println!("environment {}", lock.environment);
            if !lock.packages.is_empty() {
                print_locked(&instance, &lock);
            }
        }
    }
    Ok(())
}

fn print_locked(instance: &Instance, lock: &pwc_manifest::Lockfile) {
    let rows: Vec<Vec<String>> = lock
        .packages
        .iter()
        .map(|p| {
            let direct = if instance.manifest.mods.contains_key(&p.id) {
                "*"
            } else {
                ""
            };
            vec![
                direct.to_owned(),
                p.id.to_string(),
                p.version.to_string(),
                p.kind.as_str().to_owned(),
                p.source.to_string(),
            ]
        })
        .collect();
    print_table(&["", "ID", "VERSION", "KIND", "SOURCE"], &rows);
}

pub(crate) fn remove(mut ctx: Ctx, name: &str, yes: bool) -> Result<()> {
    let instance = ctx.open_instance(Some(name))?;
    let name = instance.manifest.name.to_string();
    if !yes {
        if !std::io::stdin().is_terminal() {
            bail!("refusing to delete instance `{name}` without confirmation (pass --yes)");
        }
        eprint!(
            "Delete instance `{name}` and its game data (worlds, settings) in {}? [y/N] ",
            instance.dir.display()
        );
        std::io::stderr().flush().ok();
        let mut answer = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut answer)
            .context("cannot read the answer")?;
        if !matches!(answer.trim(), "y" | "Y" | "yes" | "YES" | "Yes") {
            bail!("not deleted");
        }
    }
    instance.remove()?;
    ctx.status(format!(
        "Removed instance `{name}` (store entries remain until `pwc store gc`)"
    ));
    if ctx.config.active_instance.as_deref() == Some(name.as_str()) {
        ctx.config.active_instance = None;
        ctx.save_config()?;
    }
    Ok(())
}

/// One `pwc mod add` argument.
struct ModSpec {
    id: PackageId,
    requirement: ModRequirement,
    /// An id without `@req`: written as `^<locked version>` once resolved.
    pin_to_locked: bool,
}

fn parse_mod_spec(spec: &str) -> Result<ModSpec> {
    let path = Path::new(spec);
    if path.exists() {
        let path = canonical(path)?;
        if path.is_dir() {
            let manifest_path = path.join("mod.toml");
            let manifest = ModManifest::from_file(&manifest_path)
                .with_context(|| format!("{} is not a package source directory", path.display()))?;
            return Ok(ModSpec {
                id: manifest.id().clone(),
                requirement: ModRequirement {
                    version: VersionReq::STAR,
                    source: ModSource::Path(path),
                },
                pin_to_locked: false,
            });
        }
        let package = Package::from_archive_file(&path)
            .with_context(|| format!("{} is not a valid .pwcmod", path.display()))?;
        return Ok(ModSpec {
            id: package.manifest().id().clone(),
            requirement: ModRequirement {
                version: VersionReq::STAR,
                source: ModSource::File(path),
            },
            pin_to_locked: false,
        });
    }
    if spec.contains('/') || spec.contains(std::path::MAIN_SEPARATOR) || spec.ends_with(".pwcmod") {
        bail!("{spec}: no such file or directory");
    }
    let (id, req) = match spec.split_once('@') {
        Some((id, req)) => (id, Some(req)),
        None => (spec, None),
    };
    let id = PackageId::parse(id).with_context(|| {
        format!("`{spec}` is not a package id, id@requirement, source directory or .pwcmod file")
    })?;
    let version = match req {
        Some(req) => VersionReq::parse(req)
            .with_context(|| format!("invalid version requirement `{req}`"))?,
        None => VersionReq::STAR,
    };
    Ok(ModSpec {
        id,
        requirement: ModRequirement {
            version,
            source: ModSource::Registry,
        },
        pin_to_locked: req.is_none(),
    })
}

pub(crate) fn mod_add(ctx: &Ctx, specs: &[String]) -> Result<()> {
    let mut instance = ctx.open_instance(None)?;
    let original = instance.manifest.clone();
    let mut added = Vec::new();
    for spec in specs {
        let spec = parse_mod_spec(spec)?;
        added.push((spec.id.clone(), spec.pin_to_locked));
        instance.manifest.mods.insert(spec.id, spec.requirement);
    }
    let report = save_and_relock(ctx, &mut instance, original)?;
    let mut pinned = false;
    for (id, pin) in &added {
        if *pin && let Some(locked) = report.lock.package(id) {
            let req = VersionReq::parse(&format!("^{}", locked.version))
                .context("cannot pin the locked version")?;
            if let Some(entry) = instance.manifest.mods.get_mut(id) {
                entry.version = req;
                pinned = true;
            }
        }
    }
    if pinned {
        instance.save().context("cannot write instance.toml")?;
    }
    for (id, _) in &added {
        let req = instance
            .manifest
            .mods
            .get(id)
            .map(requirement)
            .unwrap_or_default();
        ctx.status(format!("Added {id} {req}"));
    }
    locked_summary(ctx, &report);
    Ok(())
}

pub(crate) fn mod_remove(ctx: &Ctx, ids: &[String]) -> Result<()> {
    let mut instance = ctx.open_instance(None)?;
    let original = instance.manifest.clone();
    for id in ids {
        let id = PackageId::parse(id).with_context(|| format!("`{id}` is not a package id"))?;
        if instance.manifest.mods.remove(&id).is_none() {
            let hint = match instance.lockfile().ok().flatten() {
                Some(lock) if lock.package(&id).is_some() => {
                    " (it is locked as a dependency of another mod)"
                }
                _ => "",
            };
            bail!(
                "`{id}` is not a mod of instance `{}`{hint}",
                instance.manifest.name
            );
        }
    }
    let report = save_and_relock(ctx, &mut instance, original)?;
    for id in ids {
        ctx.status(format!("Removed {id}"));
    }
    locked_summary(ctx, &report);
    Ok(())
}

pub(crate) fn mod_update(ctx: &Ctx, ids: &[String]) -> Result<()> {
    let instance = ctx.open_instance(None)?;
    let mut update = BTreeSet::new();
    if !ids.is_empty() {
        let lock = instance.lockfile()?;
        for id in ids {
            let id = PackageId::parse(id).with_context(|| format!("`{id}` is not a package id"))?;
            let known = instance.manifest.mods.contains_key(&id)
                || lock.as_ref().is_some_and(|l| l.package(&id).is_some());
            if !known {
                bail!(
                    "`{id}` is not used by instance `{}`",
                    instance.manifest.name
                );
            }
            update.insert(id);
        }
    }
    let options = LockOptions {
        update_all: update.is_empty(),
        update,
    };
    let (report, _) = relock(ctx, &instance, &options)?;
    locked_summary(ctx, &report);
    Ok(())
}

pub(crate) fn mod_list(ctx: &Ctx) -> Result<()> {
    let instance = ctx.open_instance(None)?;
    if let Some(lock) = instance.lockfile()?
        && !lock.packages.is_empty()
    {
        print_locked(&instance, &lock);
        return Ok(());
    }
    if instance.manifest.mods.is_empty() {
        ctx.status("No mods (add one with `pwc mod add <id>`).");
        return Ok(());
    }
    ctx.status("Not locked yet (run `pwc lock`); requirements:");
    let rows: Vec<Vec<String>> = instance
        .manifest
        .mods
        .iter()
        .map(|(id, req)| vec!["*".to_owned(), id.to_string(), requirement(req)])
        .collect();
    print_table(&["", "ID", "REQUIREMENT"], &rows);
    Ok(())
}

pub(crate) fn lock(ctx: &Ctx) -> Result<()> {
    let instance = ctx.open_instance(None)?;
    let (report, _) = relock(ctx, &instance, &LockOptions::default())?;
    locked_summary(ctx, &report);
    Ok(())
}

pub(crate) fn env(ctx: &Ctx) -> Result<()> {
    let instance = ctx.open_instance(None)?;
    let Some(lock) = instance.lockfile()? else {
        bail!(
            "instance `{}` has no pwc.lock yet (run `pwc lock`)",
            instance.manifest.name
        );
    };
    println!("{}", lock.environment);
    Ok(())
}
