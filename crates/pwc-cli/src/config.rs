//! `pwc setup`, `pwc config`, `pwc repo …`.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use pwc_instance::{PwcSource, RepositoryConfig, Wrapper};

use crate::context::Ctx;
use crate::output::print_table;

/// The name `pwc setup` gives this tooling checkout's own `mods/` repository.
const FIRST_PARTY: &str = "first-party";

/// The pwc-package-manager checkout this binary was built from (compile-time guess).
fn tooling_repo() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    dir.canonicalize().unwrap_or(dir)
}

/// `path` made absolute with symlinks resolved; it must exist.
pub(crate) fn canonical(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .with_context(|| format!("{}: no such file or directory", path.display()))
}

/// A repository name derived from its directory, unique among `taken`.
fn repository_name(path: &Path, taken: &[RepositoryConfig]) -> String {
    let base = if path == tooling_repo().join("mods") {
        FIRST_PARTY.to_owned()
    } else {
        path.file_name().map_or_else(
            || "repository".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        )
    };
    let mut name = base.clone();
    let mut n = 2;
    while taken.iter().any(|r| r.name == name) {
        name = format!("{base}-{n}");
        n += 1;
    }
    name
}

pub(crate) fn setup(mut ctx: Ctx, pwc: Option<PathBuf>, repositories: Vec<PathBuf>) -> Result<()> {
    let tooling = tooling_repo();
    let pwc_dir = match pwc {
        Some(dir) => canonical(&dir)?,
        None => {
            let sibling = tooling
                .parent()
                .map(|p| p.join("project_watt_cubed"))
                .filter(|p| p.is_dir());
            match (sibling, &ctx.config.pwc_source) {
                (Some(sibling), _) => canonical(&sibling)?,
                (None, Some(configured)) => configured.clone(),
                (None, None) => bail!(
                    "no `project_watt_cubed` checkout found next to {}; pass --pwc <dir>",
                    tooling.display()
                ),
            }
        }
    };
    let source = PwcSource::open(&pwc_dir)
        .with_context(|| format!("{} is not a usable PWC source", pwc_dir.display()))?;
    ctx.config.pwc_source = Some(pwc_dir.clone());

    if repositories.is_empty() {
        let first_party = tooling.join("mods");
        if first_party.is_dir()
            && !ctx
                .config
                .repositories
                .iter()
                .any(|r| r.path == first_party || r.name == FIRST_PARTY)
        {
            ctx.config.repositories.push(RepositoryConfig {
                name: FIRST_PARTY.to_owned(),
                path: first_party,
            });
        }
    } else {
        let mut configured: Vec<RepositoryConfig> = Vec::new();
        for dir in &repositories {
            let path = canonical(dir)?;
            if !path.is_dir() {
                bail!("repository {} is not a directory", path.display());
            }
            if configured.iter().any(|r| r.path == path) {
                continue;
            }
            let name = repository_name(&path, &configured);
            configured.push(RepositoryConfig { name, path });
        }
        ctx.config.repositories = configured;
    }
    ctx.save_config()?;

    println!("config      {}", ctx.dirs.config_file().display());
    println!(
        "pwc-source  {} (PWC {}, pwc-mod-api {})",
        pwc_dir.display(),
        source.version,
        source.api
    );
    if ctx.config.repositories.is_empty() {
        println!("repository  none (add one with `pwc repo add <dir>`)");
    }
    for repo in &ctx.config.repositories {
        println!("repository  {}  {}", repo.name, repo.path.display());
    }
    Ok(())
}

pub(crate) fn show(ctx: &Ctx) -> Result<()> {
    let config_file = ctx.dirs.config_file();
    let exists = if config_file.is_file() {
        ""
    } else {
        " (not written yet; run `pwc setup`)"
    };
    let wrapper = match &ctx.config.wrapper {
        Wrapper::Auto => "auto".to_owned(),
        Wrapper::Command(c) if c.is_empty() => "[] (cargo runs directly)".to_owned(),
        Wrapper::Command(c) => format!("{c:?}"),
    };
    let mut rows = vec![
        ("config file", format!("{}{exists}", config_file.display())),
        ("data", ctx.dirs.data.display().to_string()),
        ("cache", ctx.dirs.cache.display().to_string()),
        ("store", ctx.dirs.store().display().to_string()),
        ("instances", ctx.dirs.instances().display().to_string()),
        ("builds", ctx.dirs.builds().display().to_string()),
        ("target", ctx.dirs.target().display().to_string()),
        (
            "pwc-source",
            ctx.config.pwc_source.as_ref().map_or_else(
                || "not set (run `pwc setup`)".to_owned(),
                |p| p.display().to_string(),
            ),
        ),
        (
            "active-instance",
            ctx.config
                .active_instance
                .clone()
                .unwrap_or_else(|| "none".to_owned()),
        ),
        ("build.wrapper", wrapper),
    ];
    if let Some(pwc) = &ctx.config.pwc_source {
        let resolved = ctx.config.wrapper_command(pwc);
        let resolved = if resolved.is_empty() {
            "cargo (no wrapper)".to_owned()
        } else {
            resolved.join(" ")
        };
        rows.push(("build.wrapper resolved", resolved));
    }
    rows.push((
        "build.jobs",
        if ctx.config.jobs == 0 {
            "0 (Cargo's default)".to_owned()
        } else {
            ctx.config.jobs.to_string()
        },
    ));
    let width = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (key, value) in rows {
        println!("{key:<width$}  {value}");
    }
    if ctx.config.repositories.is_empty() {
        println!("{:<width$}  none", "repositories");
    }
    for repo in &ctx.config.repositories {
        println!(
            "{:<width$}  {}  {}",
            "repository",
            repo.name,
            repo.path.display()
        );
    }
    Ok(())
}

pub(crate) fn repo_add(mut ctx: Ctx, dir: &Path, name: Option<String>) -> Result<()> {
    let path = canonical(dir)?;
    if !path.is_dir() {
        bail!("{} is not a directory", path.display());
    }
    if let Some(existing) = ctx.config.repositories.iter().find(|r| r.path == path) {
        bail!(
            "{} is already configured as repository `{}`",
            path.display(),
            existing.name
        );
    }
    let name = match name {
        Some(name) => {
            if name.is_empty() || name.chars().any(char::is_whitespace) {
                bail!("invalid repository name {name:?} (must be non-empty, without whitespace)");
            }
            if ctx.config.repositories.iter().any(|r| r.name == name) {
                bail!("a repository named `{name}` already exists");
            }
            name
        }
        None => repository_name(&path, &ctx.config.repositories),
    };
    ctx.status(format!("Added repository `{name}` ({})", path.display()));
    ctx.config
        .repositories
        .push(RepositoryConfig { name, path });
    ctx.save_config()
}

pub(crate) fn repo_remove(mut ctx: Ctx, name: &str) -> Result<()> {
    let before = ctx.config.repositories.len();
    ctx.config.repositories.retain(|r| r.name != name);
    if ctx.config.repositories.len() == before {
        bail!("no repository named `{name}` (see `pwc repo list`)");
    }
    ctx.save_config()?;
    ctx.status(format!("Removed repository `{name}`"));
    Ok(())
}

pub(crate) fn repo_list(ctx: &Ctx) {
    if ctx.config.repositories.is_empty() {
        ctx.status("No repositories configured (add one with `pwc repo add <dir>`).");
        return;
    }
    let rows: Vec<Vec<String>> = ctx
        .config
        .repositories
        .iter()
        .map(|r| {
            let missing = if r.path.is_dir() { "" } else { "  (missing)" };
            vec![r.name.clone(), format!("{}{missing}", r.path.display())]
        })
        .collect();
    print_table(&["NAME", "PATH"], &rows);
}
