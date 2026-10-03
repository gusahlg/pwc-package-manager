//! `pwc store list|verify|gc`.

use std::collections::BTreeSet;

use anyhow::{Context as _, Result, bail};
use pwc_instance::Instance;
use pwc_manifest::PackageHash;

use crate::context::Ctx;
use crate::output::print_table;

pub(crate) fn list(ctx: &Ctx) -> Result<()> {
    let entries = ctx.store()?.list()?;
    if entries.is_empty() {
        ctx.status("The store is empty.");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = entries
        .iter()
        .map(|e| {
            vec![
                e.manifest.id().to_string(),
                e.manifest.version().to_string(),
                e.hash.to_string(),
            ]
        })
        .collect();
    print_table(&["ID", "VERSION", "HASH"], &rows);
    Ok(())
}

pub(crate) fn verify(ctx: &Ctx) -> Result<()> {
    let store = ctx.store()?;
    let entries = store.list()?;
    let mut corrupt = 0;
    for entry in &entries {
        let ok = store
            .verify(&entry.hash)
            .with_context(|| format!("cannot verify {}", entry.hash))?;
        if ok {
            ctx.detail(format!(
                "ok       {} {} {}",
                entry.manifest.id(),
                entry.manifest.version(),
                entry.hash
            ));
        } else {
            corrupt += 1;
            println!(
                "corrupt  {} {} {}",
                entry.manifest.id(),
                entry.manifest.version(),
                entry.hash
            );
        }
    }
    if corrupt > 0 {
        bail!(
            "{corrupt} of {} store entries are corrupt (delete them and run `pwc lock` to reinstall)",
            entries.len()
        );
    }
    let n = entries.len();
    ctx.status(if n == 1 {
        "1 store entry verified".to_owned()
    } else {
        format!("{n} store entries verified")
    });
    Ok(())
}

pub(crate) fn gc(ctx: &Ctx, dry_run: bool) -> Result<()> {
    let mut keep: BTreeSet<PackageHash> = BTreeSet::new();
    for instance in Instance::list(&ctx.dirs)? {
        let lock = instance.lockfile().with_context(|| {
            format!(
                "cannot read the lock of instance `{}`; refusing to collect garbage",
                instance.manifest.name
            )
        })?;
        keep.extend(lock.into_iter().flat_map(|l| l.packages).map(|p| p.hash));
    }
    let removed = ctx.store()?.gc(&keep, dry_run)?;
    let verb = if dry_run { "would remove" } else { "removed" };
    for entry in &removed {
        println!(
            "{verb}  {} {} {}",
            entry.manifest.id(),
            entry.manifest.version(),
            entry.hash
        );
    }
    ctx.status(format!(
        "{} {} unreferenced store entries ({} kept)",
        capitalised(verb),
        removed.len(),
        keep.len()
    ));
    Ok(())
}

fn capitalised(s: &str) -> String {
    let mut chars = s.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}
