//! Human-friendly output: aligned tables and short descriptions of sources and requirements.

use std::fmt::Write as _;

use pwc_instance::AvailableSource;
use pwc_manifest::{ModRequirement, ModSource, VersionReq};

/// Print rows under a header, columns padded to their widest cell (the last column is not padded).
pub(crate) fn print_table(header: &[&str], rows: &[Vec<String>]) {
    print!("{}", table(header, rows));
}

/// The text [`print_table`] prints.
pub(crate) fn table(header: &[&str], rows: &[Vec<String>]) -> String {
    let columns = header.len();
    let mut widths: Vec<usize> = header.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate().take(columns) {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let mut out = String::new();
    let mut line = |cells: &mut dyn Iterator<Item = &str>| {
        let mut text = String::new();
        for (i, cell) in cells.enumerate() {
            if i + 1 < columns {
                let _ = write!(text, "{cell:<width$}  ", width = widths[i]);
            } else {
                text.push_str(cell);
            }
        }
        out.push_str(text.trim_end());
        out.push('\n');
    };
    line(&mut header.iter().copied());
    for row in rows {
        line(&mut row.iter().map(String::as_str));
    }
    out
}

/// Where an available package is.
pub(crate) fn available_source(source: &AvailableSource) -> String {
    match source {
        AvailableSource::RepositoryDir(p) | AvailableSource::RepositoryFile(p) => {
            format!("repository:{}", p.display())
        }
        AvailableSource::Path(p) => format!("path:{}", p.display()),
        AvailableSource::File(p) => format!("file:{}", p.display()),
        AvailableSource::Store => "store".to_owned(),
    }
}

/// An instance `[mods]` entry: `^1.0`, `path ../x`, `file x.pwcmod (=1.2.0)`.
pub(crate) fn requirement(req: &ModRequirement) -> String {
    let version = if req.version == VersionReq::STAR {
        String::new()
    } else {
        format!(" ({})", req.version)
    };
    match &req.source {
        ModSource::Registry => req.version.to_string(),
        ModSource::Path(p) => format!("path {}{version}", p.display()),
        ModSource::File(p) => format!("file {}{version}", p.display()),
    }
}

/// `a ^1.0, b *` or `none`.
pub(crate) fn requirements<'a>(
    entries: impl IntoIterator<Item = (&'a pwc_manifest::PackageId, &'a VersionReq)>,
) -> String {
    let parts: Vec<String> = entries
        .into_iter()
        .map(|(id, req)| format!("{id} {req}"))
        .collect();
    if parts.is_empty() {
        "none".to_owned()
    } else {
        parts.join(", ")
    }
}

/// `1 package`, `2 packages`.
pub(crate) fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// A byte count for humans.
pub(crate) fn bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_pads_all_but_the_last_column() {
        let text = table(
            &["ID", "VERSION", "DESCRIPTION"],
            &[
                vec!["pwc.hotbar".into(), "1.0.0".into(), "A hotbar.".into()],
                vec!["x.y".into(), "10.20.30".into(), String::new()],
            ],
        );
        assert_eq!(
            text,
            "ID          VERSION   DESCRIPTION\npwc.hotbar  1.0.0     A hotbar.\nx.y         10.20.30\n"
        );
    }

    #[test]
    fn counts() {
        assert_eq!(count(1, "package"), "1 package");
        assert_eq!(count(0, "package"), "0 packages");
    }

    #[test]
    fn byte_counts() {
        assert_eq!(bytes(12), "12 B");
        assert_eq!(bytes(2048), "2.0 KiB");
        assert_eq!(bytes(5 * 1024 * 1024 + 1), "5.0 MiB");
    }
}
