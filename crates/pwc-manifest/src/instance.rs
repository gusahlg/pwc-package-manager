//! `instance.toml` (docs/spec/instances.md).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::manifest::parse_req;
use crate::text::{check_format, push_str_kv, quote, toml_error};
use crate::{FORMAT, InstanceName, ManifestError, PackageId, VersionReq};

/// Where a requested module comes from.
#[derive(Clone, PartialEq, Debug)]
pub enum ModuleSource {
    /// Any repository or the store.
    Registry,
    /// A local source tree (mutable, for development). Relative to the instance directory if relative.
    Path(PathBuf),
    /// A `.pwcmodule` file.
    File(PathBuf),
}

/// The name `ModuleSource` had before the rename.
#[doc(hidden)]
pub type ModSource = ModuleSource;

/// The name `ModuleRequirement` had before the rename.
#[doc(hidden)]
pub type ModRequirement = ModuleRequirement;

/// One `[modules]` entry.
#[derive(Clone, PartialEq, Debug)]
pub struct ModuleRequirement {
    /// Version requirement (`*` when a path/file entry gave none).
    pub version: VersionReq,
    /// Source.
    pub source: ModuleSource,
}

/// Build profile.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BuildProfile {
    /// Optimised (default).
    #[default]
    Release,
    /// Debug.
    Dev,
}

impl BuildProfile {
    /// `"release"` or `"dev"`.
    pub fn as_str(self) -> &'static str {
        match self {
            BuildProfile::Release => "release",
            BuildProfile::Dev => "dev",
        }
    }

    fn from_name(s: &str) -> Option<Self> {
        match s {
            "release" => Some(BuildProfile::Release),
            "dev" => Some(BuildProfile::Dev),
            _ => None,
        }
    }
}

/// A parsed `instance.toml`.
#[derive(Clone, PartialEq, Debug)]
pub struct InstanceManifest {
    /// Instance name.
    pub name: InstanceName,
    /// Optional description.
    pub description: Option<String>,
    /// Requirement on the PWC game version.
    pub pwc: VersionReq,
    /// Requested modules.
    pub modules: BTreeMap<PackageId, ModuleRequirement>,
    /// `[build] profile`.
    pub profile: BuildProfile,
}

impl InstanceManifest {
    /// A new instance with no modules.
    pub fn new(name: InstanceName, pwc: VersionReq) -> Self {
        Self {
            name,
            description: None,
            pwc,
            modules: BTreeMap::new(),
            profile: BuildProfile::Release,
        }
    }

    /// Parse and validate.
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        check_format(text)?;
        let raw: RawInstance = toml::from_str(text).map_err(toml_error)?;
        raw.validate()
    }

    /// Read a file.
    pub fn from_file(path: &Path) -> Result<Self, ManifestError> {
        let text = std::fs::read_to_string(path).map_err(|source| ManifestError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&text)
    }

    /// Canonical text: `format`, `name`, `description` (if any), `pwc`, then `[modules]` (sorted by id)
    /// and `[build]`. Registry requirements are written as plain strings, `path`/`file` entries as
    /// inline tables carrying `version` only when it is not `*`. Paths that are not valid UTF-8 are
    /// written lossily (TOML text cannot hold them).
    pub fn to_toml_string(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "format = {FORMAT}");
        push_str_kv(&mut out, "name", self.name.as_str());
        if let Some(description) = &self.description {
            push_str_kv(&mut out, "description", description);
        }
        push_str_kv(&mut out, "pwc", &self.pwc.to_string());
        out.push_str("\n[modules]\n");
        for (id, req) in &self.modules {
            let value = match &req.source {
                ModuleSource::Registry => quote(&req.version.to_string()),
                ModuleSource::Path(path) | ModuleSource::File(path) => {
                    let key = if matches!(req.source, ModuleSource::Path(_)) {
                        "path"
                    } else {
                        "file"
                    };
                    let mut table = format!("{{ {key} = {}", quote(&path.to_string_lossy()));
                    if req.version != VersionReq::STAR {
                        let _ = write!(table, ", version = {}", quote(&req.version.to_string()));
                    }
                    table.push_str(" }");
                    table
                }
            };
            let _ = writeln!(out, "{} = {value}", quote(id.as_str()));
        }
        out.push_str("\n[build]\n");
        push_str_kv(&mut out, "profile", self.profile.as_str());
        out
    }
}

/// `instance.toml` as written, before validation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct RawInstance {
    /// Checked by [`check_format`] before deserialising.
    #[serde(rename = "format")]
    _format: serde::de::IgnoredAny,
    name: Option<String>,
    description: Option<String>,
    pwc: Option<String>,
    modules: Option<toml::Table>,
    /// The older name of `modules`.
    mods: Option<toml::Table>,
    build: Option<RawBuild>,
}

/// `[build]` as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct RawBuild {
    profile: Option<String>,
}

impl RawInstance {
    fn validate(self) -> Result<InstanceManifest, ManifestError> {
        let name =
            InstanceName::parse_as(&self.name.ok_or(ManifestError::Missing("name"))?, "name")?;
        let pwc = match self.pwc {
            None => VersionReq::STAR,
            Some(req) => parse_req("pwc", &req)?,
        };
        let (table_name, table) = match (self.modules, self.mods) {
            (Some(_), Some(_)) => {
                return Err(ManifestError::invalid(
                    "modules",
                    "",
                    "both `[modules]` and its older name `[mods]` are present; keep only `[modules]`",
                ));
            }
            (Some(table), None) => ("modules", table),
            (None, Some(table)) => ("mods", table),
            (None, None) => ("modules", toml::Table::new()),
        };
        let mut modules = BTreeMap::new();
        for (key, value) in table {
            let entry_key = format!("{table_name}.{}", quote(&key));
            let id = PackageId::parse_as(&key, &entry_key)?;
            modules.insert(id, parse_module_requirement(&entry_key, value)?);
        }
        let profile = match self.build.and_then(|b| b.profile) {
            None => BuildProfile::default(),
            Some(profile) => BuildProfile::from_name(&profile).ok_or_else(|| {
                ManifestError::invalid("build.profile", profile, "must be \"release\" or \"dev\"")
            })?,
        };
        Ok(InstanceManifest {
            name,
            description: self.description,
            pwc,
            modules,
            profile,
        })
    }
}

/// One `[modules]` value: a requirement string, or a table with exactly one source (`version`,
/// `path` or `file`); a `path`/`file` table may also carry `version`.
fn parse_module_requirement(
    key: &str,
    value: toml::Value,
) -> Result<ModuleRequirement, ManifestError> {
    let mut table = match value {
        toml::Value::String(req) => {
            return Ok(ModuleRequirement {
                version: parse_req(key, &req)?,
                source: ModuleSource::Registry,
            });
        }
        toml::Value::Table(table) => table,
        other => {
            return Err(ManifestError::invalid(
                key,
                other.to_string(),
                "must be a version requirement string or a table with `version`, `path` or `file`",
            ));
        }
    };
    if let Some(unknown) = table
        .keys()
        .find(|k| !matches!(k.as_str(), "version" | "path" | "file"))
    {
        return Err(ManifestError::invalid(
            format!("{key}.{unknown}"),
            "",
            "unknown key (a module entry has `version`, `path` or `file`)",
        ));
    }
    let mut string = |field: &str| -> Result<Option<String>, ManifestError> {
        match table.remove(field) {
            None => Ok(None),
            Some(toml::Value::String(s)) if s.is_empty() => Err(ManifestError::invalid(
                format!("{key}.{field}"),
                s,
                "must not be empty",
            )),
            Some(toml::Value::String(s)) => Ok(Some(s)),
            Some(other) => Err(ManifestError::invalid(
                format!("{key}.{field}"),
                other.to_string(),
                "must be a string",
            )),
        }
    };
    let version = string("version")?;
    let path = string("path")?;
    let file = string("file")?;
    let source = match (path, file) {
        (Some(_), Some(_)) => {
            return Err(ManifestError::invalid(
                key,
                "",
                "has both `path` and `file`; give exactly one source",
            ));
        }
        (Some(path), None) => ModuleSource::Path(PathBuf::from(path)),
        (None, Some(file)) => ModuleSource::File(PathBuf::from(file)),
        (None, None) if version.is_none() => {
            return Err(ManifestError::invalid(
                key,
                "{}",
                "needs a source: `version`, `path` or `file`",
            ));
        }
        (None, None) => ModuleSource::Registry,
    };
    let version = match version {
        None => VersionReq::STAR,
        Some(req) => parse_req(&format!("{key}.version"), &req)?,
    };
    Ok(ModuleRequirement { version, source })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC_EXAMPLE: &str = r#"format = 1
name = "survival"
description = "PWC with the essentials and better caves"   # optional
pwc = "^2.0"                                                # requirement on the PWC version

[modules]
"pwc.essentials" = "^1.0"                                   # from a repository or the store
"gusahlg.better-caves" = { path = "../../dev/better-caves" } # local mutable source tree
"foo.seasons" = { file = "/downloads/foo.seasons-2.3.1.pwcmodule" }
"foo.noise" = { version = "^2.0" }

[build]
profile = "release"                                         # "release" (default) or "dev"
"#;

    fn id(s: &str) -> PackageId {
        PackageId::parse(s).unwrap()
    }

    fn req(s: &str) -> VersionReq {
        VersionReq::parse(s).unwrap()
    }

    fn assert_round_trip(m: &InstanceManifest) {
        let text = m.to_toml_string();
        let back = InstanceManifest::parse(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
        assert_eq!(&back, m, "{text}");
        assert_eq!(back.to_toml_string(), text);
    }

    fn assert_invalid(text: &str, key: &str) {
        match InstanceManifest::parse(text) {
            Err(ManifestError::Invalid { key: k, .. }) if k == key => {}
            other => panic!("expected invalid `{key}`, got {other:?}\n{text}"),
        }
    }

    #[test]
    fn spec_example() {
        let m = InstanceManifest::parse(SPEC_EXAMPLE).unwrap();
        assert_eq!(m.name.as_str(), "survival");
        assert_eq!(
            m.description.as_deref(),
            Some("PWC with the essentials and better caves")
        );
        assert_eq!(m.pwc, req("^2.0"));
        assert_eq!(m.profile, BuildProfile::Release);
        assert_eq!(m.modules.len(), 4);
        assert_eq!(
            m.modules[&id("pwc.essentials")],
            ModuleRequirement {
                version: req("^1.0"),
                source: ModuleSource::Registry
            }
        );
        assert_eq!(
            m.modules[&id("gusahlg.better-caves")],
            ModuleRequirement {
                version: VersionReq::STAR,
                source: ModuleSource::Path("../../dev/better-caves".into())
            }
        );
        assert_eq!(
            m.modules[&id("foo.seasons")],
            ModuleRequirement {
                version: VersionReq::STAR,
                source: ModuleSource::File("/downloads/foo.seasons-2.3.1.pwcmodule".into())
            }
        );
        assert_eq!(
            m.modules[&id("foo.noise")],
            ModuleRequirement {
                version: req("^2.0"),
                source: ModuleSource::Registry
            }
        );
        assert_round_trip(&m);
    }

    #[test]
    fn canonical_text() {
        let m = InstanceManifest::parse(SPEC_EXAMPLE).unwrap();
        let expected = r#"format = 1
name = "survival"
description = "PWC with the essentials and better caves"
pwc = "^2.0"

[modules]
"foo.noise" = "^2.0"
"foo.seasons" = { file = "/downloads/foo.seasons-2.3.1.pwcmodule" }
"gusahlg.better-caves" = { path = "../../dev/better-caves" }
"pwc.essentials" = "^1.0"

[build]
profile = "release"
"#;
        assert_eq!(m.to_toml_string(), expected);
    }

    #[test]
    fn defaults_and_new() {
        let m = InstanceManifest::parse("format = 1\nname = \"vanilla\"\n").unwrap();
        assert_eq!(
            m,
            InstanceManifest::new(InstanceName::parse("vanilla").unwrap(), VersionReq::STAR)
        );
        assert_eq!(m.pwc, VersionReq::STAR);
        assert_eq!(m.profile, BuildProfile::Release);
        assert!(m.modules.is_empty() && m.description.is_none());
        assert_round_trip(&m);

        let mut dev = InstanceManifest::new(InstanceName::parse("dev").unwrap(), req(">=2.0, <3"));
        dev.profile = BuildProfile::Dev;
        dev.description = Some("Line \"one\"\nline two".into());
        dev.modules.insert(
            id("a.b"),
            ModuleRequirement {
                version: req("=1.2.3"),
                source: ModuleSource::Path("/x/y z".into()),
            },
        );
        dev.modules.insert(
            id("c.d"),
            ModuleRequirement {
                version: req("^0.3"),
                source: ModuleSource::File("rel/c.d-0.3.0.pwcmodule".into()),
            },
        );
        dev.modules.insert(
            id("e.f"),
            ModuleRequirement {
                version: VersionReq::STAR,
                source: ModuleSource::Registry,
            },
        );
        assert_round_trip(&dev);
        assert!(
            dev.to_toml_string()
                .contains("\"a.b\" = { path = \"/x/y z\", version = \"=1.2.3\" }")
        );
        assert!(dev.to_toml_string().contains("profile = \"dev\""));
    }

    #[test]
    fn mod_entries() {
        let doc = |entry: &str| format!("format = 1\nname = \"x\"\n[modules]\n{entry}\n");
        let parse = |entry: &str| {
            InstanceManifest::parse(&doc(entry)).map(|m| m.modules.into_values().next().unwrap())
        };
        assert_eq!(
            parse("\"a.b\" = { path = \"p\", version = \"^1\" }").unwrap(),
            ModuleRequirement {
                version: req("^1"),
                source: ModuleSource::Path("p".into())
            }
        );
        assert_eq!(
            parse("\"a.b\" = { file = \"f.pwcmodule\", version = \"=2.0.0\" }").unwrap(),
            ModuleRequirement {
                version: req("=2.0.0"),
                source: ModuleSource::File("f.pwcmodule".into())
            }
        );
        assert_invalid(
            &doc("\"a.b\" = { path = \"p\", file = \"f\" }"),
            "modules.\"a.b\"",
        );
        assert_invalid(&doc("\"a.b\" = {}"), "modules.\"a.b\"");
        assert_invalid(
            &doc("\"a.b\" = { git = \"https://x\" }"),
            "modules.\"a.b\".git",
        );
        assert_invalid(&doc("\"a.b\" = { path = \"\" }"), "modules.\"a.b\".path");
        assert_invalid(&doc("\"a.b\" = { path = 3 }"), "modules.\"a.b\".path");
        assert_invalid(
            &doc("\"a.b\" = { version = \"nope\" }"),
            "modules.\"a.b\".version",
        );
        assert_invalid(
            &doc("\"a.b\" = { path = \"p\", version = \"nope\" }"),
            "modules.\"a.b\".version",
        );
        assert_invalid(&doc("\"a.b\" = \"nope\""), "modules.\"a.b\"");
        assert_invalid(&doc("\"a.b\" = 1"), "modules.\"a.b\"");
        assert_invalid(&doc("\"ab\" = \"1\""), "modules.\"ab\"");
    }

    #[test]
    fn top_level_rules() {
        assert!(matches!(
            InstanceManifest::parse("format = 1\n"),
            Err(ManifestError::Missing("name"))
        ));
        assert!(matches!(
            InstanceManifest::parse("name = \"x\"\n"),
            Err(ManifestError::Missing("format"))
        ));
        assert!(matches!(
            InstanceManifest::parse("format = 3\nname = \"x\"\n"),
            Err(ManifestError::Format { found: 3 })
        ));
        assert_invalid("format = 1\nname = \"Not Valid\"\n", "name");
        assert_invalid("format = 1\nname = \"x\"\npwc = \"two\"\n", "pwc");
        assert_invalid(
            "format = 1\nname = \"x\"\n[build]\nprofile = \"debug\"\n",
            "build.profile",
        );
        for unknown in [
            "format = 1\nname = \"x\"\nmod = {}\n",
            "format = 1\nname = \"x\"\n[build]\nlto = true\n",
        ] {
            let err = InstanceManifest::parse(unknown).unwrap_err();
            assert!(
                matches!(err, ManifestError::Toml(_)) && err.to_string().contains("unknown"),
                "{err}"
            );
        }
    }

    #[test]
    fn the_older_mods_table_is_still_read_and_never_written() {
        let old = "format = 1\nname = \"x\"\n[mods]\n\"a.b\" = \"^1\"\n";
        let m = InstanceManifest::parse(old).unwrap();
        assert_eq!(m.modules.len(), 1);
        let written = m.to_toml_string();
        assert!(written.contains("[modules]") && !written.contains("[mods]"));
        assert_eq!(InstanceManifest::parse(&written).unwrap(), m);
        assert_invalid(
            "format = 1\nname = \"x\"\n[mods]\n\"ab\" = \"1\"\n",
            "mods.\"ab\"",
        );
    }

    #[test]
    fn both_module_tables_are_an_error_naming_both() {
        let both = "format = 1\nname = \"x\"\n[mods]\n[modules]\n";
        let err = InstanceManifest::parse(both).unwrap_err().to_string();
        assert!(err.contains("[modules]") && err.contains("[mods]"), "{err}");
    }
}
