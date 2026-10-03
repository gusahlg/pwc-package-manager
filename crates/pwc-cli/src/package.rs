//! `pwc package …`, `pwc search`, `pwc info`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use pwc_instance::{Available, AvailablePackage};
use pwc_manifest::{ModManifest, PackageHash, PackageId};
use pwc_package::Package;

use crate::context::Ctx;
use crate::output::{available_source, bytes, print_table, requirements};

/// Scan the configured repositories and the store, reporting skipped entries as warnings.
fn scan(ctx: &Ctx) -> Result<Available> {
    let store = ctx.store()?;
    let available = Available::scan(&ctx.config, &store)
        .context("cannot scan the repositories and the store")?;
    for warning in &available.warnings {
        ctx.warn(warning);
    }
    Ok(available)
}

/// The newest version, preferring releases over pre-releases.
fn newest<'a>(versions: &[&'a AvailablePackage]) -> &'a AvailablePackage {
    versions
        .iter()
        .copied()
        .max_by(|a, b| {
            let (a, b) = (a.manifest.version(), b.manifest.version());
            (a.pre.is_empty(), a).cmp(&(b.pre.is_empty(), b))
        })
        .expect("at least one version")
}

pub(crate) fn search(ctx: &Ctx, text: Option<&str>) -> Result<()> {
    let available = scan(ctx)?;
    let needle = text.map(str::to_lowercase);
    let mut by_id: BTreeMap<&PackageId, Vec<&AvailablePackage>> = BTreeMap::new();
    for package in &available.packages {
        by_id
            .entry(package.manifest.id())
            .or_default()
            .push(package);
    }
    let rows: Vec<Vec<String>> = by_id
        .values()
        .map(|versions| newest(versions))
        .filter(|p| needle.as_deref().is_none_or(|n| matches(&p.manifest, n)))
        .map(|p| {
            let m = &p.manifest.package;
            vec![
                m.id.to_string(),
                m.version.to_string(),
                m.kind.as_str().to_owned(),
                m.description.clone(),
            ]
        })
        .collect();
    if rows.is_empty() {
        ctx.status(match text {
            Some(text) => format!("No packages match `{text}`."),
            None => "No packages available (configure a repository with `pwc repo add <dir>`)."
                .to_owned(),
        });
        return Ok(());
    }
    print_table(&["ID", "VERSION", "KIND", "DESCRIPTION"], &rows);
    Ok(())
}

fn matches(manifest: &ModManifest, needle: &str) -> bool {
    let p = &manifest.package;
    p.id.as_str().contains(needle)
        || p.name.to_lowercase().contains(needle)
        || p.description.to_lowercase().contains(needle)
        || p.keywords.iter().any(|k| k.contains(needle))
}

pub(crate) fn info(ctx: &Ctx, id: &str) -> Result<()> {
    let id = PackageId::parse(id).with_context(|| format!("`{id}` is not a package id"))?;
    let available = scan(ctx)?;
    let mut versions: Vec<&AvailablePackage> = available
        .packages
        .iter()
        .filter(|p| p.manifest.id() == &id)
        .collect();
    if versions.is_empty() {
        bail!("unknown package `{id}` (not in any configured repository or the store)");
    }
    versions.sort_by(|a, b| {
        b.manifest
            .version()
            .cmp(a.manifest.version())
            .then_with(|| a.hash.cmp(&b.hash))
    });
    let latest = &newest(&versions).manifest.package;
    println!("{id} — {}", latest.name);
    println!("{}", latest.description);
    for package in versions {
        let m = &package.manifest;
        let p = &m.package;
        println!();
        println!("{} {} ({})", p.id, p.version, p.kind.as_str());
        println!("  hash          {}", package.hash);
        println!("  source        {}", available_source(&package.source));
        if let Some(api) = &p.pwc_api {
            println!("  pwc-api       {api}");
        }
        println!("  license       {}", p.license);
        println!("  authors       {}", p.authors.join(", "));
        println!("  dependencies  {}", requirements(&m.dependencies));
        if !m.conflicts.is_empty() {
            println!("  conflicts     {}", requirements(&m.conflicts));
        }
        if p.name != latest.name || p.description != latest.description {
            println!("  name          {}", p.name);
            println!("  description   {}", p.description);
        }
        for (key, value) in [
            ("repository", &p.repository),
            ("homepage", &p.homepage),
            ("documentation", &p.documentation),
        ] {
            if let Some(value) = value {
                println!("  {key:<13} {value}");
            }
        }
    }
    Ok(())
}

/// `dir` or the current directory.
fn source_dir(dir: Option<PathBuf>) -> PathBuf {
    dir.unwrap_or_else(|| PathBuf::from("."))
}

fn collect(dir: &Path) -> Result<(Package, Vec<String>)> {
    Package::from_dir(dir).with_context(|| format!("{} is not a valid package", dir.display()))
}

fn report_ignored(ctx: &Ctx, ignored: &[String]) {
    if ignored.is_empty() {
        return;
    }
    if ctx.verbose {
        for path in ignored {
            eprintln!("ignored: {path}");
        }
    } else {
        ctx.status(format!(
            "{} path(s) in the directory are not part of the package (list them with -v)",
            ignored.len()
        ));
    }
}

fn total_bytes(package: &Package) -> u64 {
    package.files().iter().map(|f| f.bytes.len() as u64).sum()
}

pub(crate) fn check(ctx: &Ctx, dir: Option<PathBuf>) -> Result<()> {
    let dir = source_dir(dir);
    let (package, ignored) = collect(&dir)?;
    let m = &package.manifest().package;
    println!(
        "ok: {} {} ({}), {} files, {}, {}",
        m.id,
        m.version,
        m.kind.as_str(),
        package.files().len(),
        bytes(total_bytes(&package)),
        package.hash()
    );
    if ctx.verbose {
        for file in package.files() {
            eprintln!("included: {}", file.path);
        }
    }
    report_ignored(ctx, &ignored);
    Ok(())
}

pub(crate) fn build(ctx: &Ctx, dir: Option<PathBuf>, out_dir: Option<PathBuf>) -> Result<()> {
    let dir = source_dir(dir);
    let (package, ignored) = collect(&dir)?;
    let out_dir = out_dir.unwrap_or_else(|| dir.join("dist"));
    let path = package
        .write_archive(&out_dir)
        .with_context(|| format!("cannot write the package into {}", out_dir.display()))?;
    let m = &package.manifest().package;
    ctx.status(format!(
        "Packaged {} {} ({} files, {})",
        m.id,
        m.version,
        package.files().len(),
        bytes(total_bytes(&package))
    ));
    report_ignored(ctx, &ignored);
    println!("{}  {}", package.hash(), path.display());
    Ok(())
}

pub(crate) fn verify(ctx: &Ctx, file: &Path, hash: Option<&str>) -> Result<()> {
    let expected = hash
        .map(PackageHash::parse)
        .transpose()
        .context("invalid --hash")?;
    let package = Package::from_archive_file(file)
        .with_context(|| format!("{} failed verification", file.display()))?;
    if let Some(expected) = expected
        && &expected != package.hash()
    {
        bail!(
            "{} failed verification: expected {expected}, found {}",
            file.display(),
            package.hash()
        );
    }
    let m = &package.manifest().package;
    ctx.detail(format!("{} files verified", package.files().len()));
    println!("ok: {} {} {}", m.id, m.version, package.hash());
    Ok(())
}

pub(crate) fn inspect(file: &Path) -> Result<()> {
    let package = Package::from_archive_file(file)
        .with_context(|| format!("{} is not a valid package", file.display()))?;
    let manifest = package.manifest();
    let p = &manifest.package;
    let mut rows: Vec<(&str, String)> = vec![
        ("id", p.id.to_string()),
        ("name", p.name.clone()),
        ("version", p.version.to_string()),
        ("kind", p.kind.as_str().to_owned()),
        ("description", p.description.clone()),
        ("authors", p.authors.join(", ")),
        ("license", p.license.clone()),
    ];
    if let Some(api) = &p.pwc_api {
        rows.push(("pwc-api", api.to_string()));
    }
    rows.push(("edition", p.edition.as_str().to_owned()));
    rows.push(("dependencies", requirements(&manifest.dependencies)));
    if !manifest.conflicts.is_empty() {
        rows.push(("conflicts", requirements(&manifest.conflicts)));
    }
    rows.push(("hash", package.hash().to_string()));
    for (key, value) in rows {
        println!("{key:<12}  {value}");
    }
    println!(
        "files ({}, {}):",
        package.files().len(),
        bytes(total_bytes(&package))
    );
    let width = package
        .files()
        .iter()
        .map(|f| f.bytes.len().to_string().len())
        .max()
        .unwrap_or(1);
    for file in package.files() {
        println!("  {:>width$}  {}", file.bytes.len(), file.path);
    }
    Ok(())
}
