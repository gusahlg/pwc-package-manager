//! Shared helpers for reading and writing the TOML formats: the `format` check, TOML error
//! conversion and a small deterministic writer for canonical output.

use std::fmt::Write as _;

use crate::{FORMAT, ManifestError};

/// Convert a `toml` deserialisation error (syntax, unknown key, wrong type). Its message carries
/// the line, column and the offending key.
pub(crate) fn toml_error(err: toml::de::Error) -> ManifestError {
    ManifestError::Toml(err.to_string().trim_end().to_string())
}

/// Parse `text` as a TOML table and check its top-level `format` before anything else, so a file
/// from a newer pwc is reported as "upgrade pwc" rather than as unknown keys.
pub(crate) fn check_format(text: &str) -> Result<(), ManifestError> {
    let table: toml::Table = toml::from_str(text).map_err(toml_error)?;
    match table.get("format") {
        None => Err(ManifestError::Missing("format")),
        Some(toml::Value::Integer(n)) if *n == i64::from(FORMAT) => Ok(()),
        Some(toml::Value::Integer(n)) => match u32::try_from(*n) {
            Ok(found) => Err(ManifestError::Format { found }),
            Err(_) => Err(ManifestError::invalid(
                "format",
                n.to_string(),
                format!("must be {FORMAT}"),
            )),
        },
        Some(other) => Err(ManifestError::invalid(
            "format",
            other.to_string(),
            format!("must be the integer {FORMAT}"),
        )),
    }
}

/// A TOML basic string (`"…"`) with every character that needs it escaped. Deterministic: the
/// same text always gives the same output.
pub(crate) fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() && (c as u32) < 0x80 => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A TOML array of strings on one line: `["a", "b"]`.
pub(crate) fn quote_array<'a>(items: impl IntoIterator<Item = &'a str>) -> String {
    let items: Vec<String> = items.into_iter().map(quote).collect();
    format!("[{}]", items.join(", "))
}

/// Append `key = "value"\n`.
pub(crate) fn push_str_kv(out: &mut String, key: &str, value: &str) {
    let _ = writeln!(out, "{key} = {}", quote(value));
}

/// Append `key = ["a", "b"]\n`.
pub(crate) fn push_array_kv<'a>(
    out: &mut String,
    key: &str,
    items: impl IntoIterator<Item = &'a str>,
) {
    let _ = writeln!(out, "{key} = {}", quote_array(items));
}

/// Whether `s` contains a control character (C0, DEL or C1).
pub(crate) fn has_control(s: &str) -> bool {
    s.chars().any(char::is_control)
}

/// Check a package-relative path (`src/lib.rs`, `assets/icon.svg`, `LICENSE-MIT`): non-empty UTF-8,
/// relative, `/`-separated, without empty, `.` or `..` components, without `\`, NUL or other
/// control characters, at most [`MAX_PATH_BYTES`] bytes. This is the path rule of both `mod.toml`
/// path keys and `.pwcmod` entries (docs/spec/package-format.md); it does not decide whether the
/// path is *part of* a package (that is `pwc-package`'s contents rule).
pub fn check_package_path(path: &str) -> Result<(), &'static str> {
    if path.is_empty() {
        return Err("a package path must not be empty");
    }
    if path.len() > MAX_PATH_BYTES {
        return Err("a package path must be at most 1024 bytes");
    }
    if path.starts_with('/') {
        return Err("a package path must be relative (no leading `/`)");
    }
    if path.contains('\\') {
        return Err("a package path must use `/` separators (no `\\`)");
    }
    if has_control(path) {
        return Err("a package path must not contain control characters");
    }
    for component in path.split('/') {
        match component {
            "" => {
                return Err(
                    "a package path must not contain empty components (`//` or a trailing `/`)",
                );
            }
            "." | ".." => return Err("a package path must not contain `.` or `..` components"),
            _ => {}
        }
    }
    Ok(())
}

/// The longest package-relative path accepted, in bytes.
pub const MAX_PATH_BYTES: usize = 1024;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_round_trips_through_toml() {
        for s in [
            "",
            "plain",
            "with \"quotes\" and \\backslashes\\",
            "tab\tnewline\ncr\r",
            "bell\u{7} del\u{7f} nul\u{0} esc\u{1b}",
            "unicode: å ä ö — ✓ 🦀",
            "'single' '''triple'''",
            "c1 control \u{85}",
        ] {
            let doc = format!("k = {}\n", quote(s));
            let table: toml::Table =
                toml::from_str(&doc).unwrap_or_else(|e| panic!("{doc:?}: {e}"));
            assert_eq!(table["k"].as_str(), Some(s), "{doc:?}");
        }
    }

    #[test]
    fn arrays() {
        assert_eq!(quote_array([]), "[]");
        assert_eq!(quote_array(["a", "b\"c"]), r#"["a", "b\"c"]"#);
    }

    #[test]
    fn format_check() {
        assert!(check_format("format = 1").is_ok());
        assert!(matches!(
            check_format(""),
            Err(ManifestError::Missing("format"))
        ));
        assert!(matches!(
            check_format("format = 2"),
            Err(ManifestError::Format { found: 2 })
        ));
        assert!(matches!(
            check_format("format = 0"),
            Err(ManifestError::Format { found: 0 })
        ));
        assert!(matches!(
            check_format("format = -1"),
            Err(ManifestError::Invalid { .. })
        ));
        assert!(matches!(
            check_format("format = \"1\""),
            Err(ManifestError::Invalid { .. })
        ));
        assert!(matches!(
            check_format("format = 1.0"),
            Err(ManifestError::Invalid { .. })
        ));
        assert!(matches!(
            check_format("format = "),
            Err(ManifestError::Toml(_))
        ));
        let upgrade = check_format("format = 7\nnew-key = true")
            .unwrap_err()
            .to_string();
        assert!(upgrade.contains("upgrade pwc"), "{upgrade}");
    }

    #[test]
    fn package_paths() {
        for ok in [
            "mod.toml",
            "src/lib.rs",
            "assets/a b/ü.png",
            "LICENSE-MIT",
            "a/.b",
            "a..b/c",
            "x".repeat(1024).as_str(),
        ] {
            check_package_path(ok).unwrap_or_else(|e| panic!("{ok:?}: {e}"));
        }
        for bad in [
            "",
            "/etc/passwd",
            "src\\lib.rs",
            "src//lib.rs",
            "src/",
            "./src/lib.rs",
            "src/./lib.rs",
            "../escape",
            "src/../../escape",
            "..",
            ".",
            "nul\u{0}byte",
            "new\nline",
            "x".repeat(1025).as_str(),
        ] {
            assert!(check_package_path(bad).is_err(), "{bad:?}");
        }
    }
}
