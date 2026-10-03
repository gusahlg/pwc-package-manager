//! `mod.toml` (docs/spec/mod-manifest.md).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use serde::Deserialize;

use crate::text::{
    check_format, check_package_path, has_control, push_array_kv, push_str_kv, quote, toml_error,
};
use crate::{ManifestError, PackageId, Version, VersionReq, check_package_license};

/// What a package is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ModKind {
    /// Registers mods into the game (`src/lib.rs` with `register`).
    #[default]
    Mod,
    /// Rust code for other packages; registers nothing.
    Library,
    /// Dependencies only, no code.
    Bundle,
}

impl ModKind {
    /// `"mod"`, `"library"` or `"bundle"`.
    pub fn as_str(self) -> &'static str {
        match self {
            ModKind::Mod => "mod",
            ModKind::Library => "library",
            ModKind::Bundle => "bundle",
        }
    }

    /// The kind named `s` (`"mod"`, `"library"`, `"bundle"`).
    pub(crate) fn from_name(s: &str) -> Option<Self> {
        match s {
            "mod" => Some(ModKind::Mod),
            "library" => Some(ModKind::Library),
            "bundle" => Some(ModKind::Bundle),
            _ => None,
        }
    }

    /// Whether packages of this kind contain Rust code (`src/lib.rs`) and declare `pwc-api`.
    pub fn has_code(self) -> bool {
        !matches!(self, ModKind::Bundle)
    }
}

impl std::fmt::Display for ModKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Rust edition of the package's code.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Edition {
    /// 2021.
    E2021,
    /// 2024 (default).
    #[default]
    E2024,
}

impl Edition {
    /// `"2021"` or `"2024"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Edition::E2021 => "2021",
            Edition::E2024 => "2024",
        }
    }

    fn from_name(s: &str) -> Option<Self> {
        match s {
            "2021" => Some(Edition::E2021),
            "2024" => Some(Edition::E2024),
            _ => None,
        }
    }
}

/// The fixed category list.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Category {
    /// `gameplay`
    Gameplay,
    /// `interface`
    Interface,
    /// `visuals`
    Visuals,
    /// `worldgen`
    Worldgen,
    /// `audio`
    Audio,
    /// `library`
    Library,
    /// `tools`
    Tools,
    /// `bundle`
    Bundle,
}

impl Category {
    /// Every category, in spec order.
    pub const ALL: [Category; 8] = [
        Category::Gameplay,
        Category::Interface,
        Category::Visuals,
        Category::Worldgen,
        Category::Audio,
        Category::Library,
        Category::Tools,
        Category::Bundle,
    ];

    /// The category's name in `mod.toml` (`"gameplay"`, `"interface"`, …).
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Gameplay => "gameplay",
            Category::Interface => "interface",
            Category::Visuals => "visuals",
            Category::Worldgen => "worldgen",
            Category::Audio => "audio",
            Category::Library => "library",
            Category::Tools => "tools",
            Category::Bundle => "bundle",
        }
    }

    fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == s)
    }
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `[package]`.
#[derive(Clone, PartialEq, Debug)]
pub struct PackageMetadata {
    /// Permanent id.
    pub id: PackageId,
    /// Display name.
    pub name: String,
    /// Exact version (no build metadata).
    pub version: Version,
    /// One sentence.
    pub description: String,
    /// `"Name"` or `"Name <email>"`.
    pub authors: Vec<String>,
    /// SPDX expression (policy-checked).
    pub license: String,
    /// Package-relative licence text paths.
    pub license_files: Vec<String>,
    /// What the package is.
    pub kind: ModKind,
    /// Requirement on `pwc-mod-api` (`None` only for bundles).
    pub pwc_api: Option<VersionReq>,
    /// Rust edition.
    pub edition: Edition,
    /// README path (default `README.md`).
    pub readme: String,
    /// Optional icon under `assets/`.
    pub icon: Option<String>,
    /// Optional `https://` URL.
    pub repository: Option<String>,
    /// Optional `https://` URL.
    pub homepage: Option<String>,
    /// Optional `https://` URL.
    pub documentation: Option<String>,
    /// ≤ 5 keywords.
    pub keywords: Vec<String>,
    /// Categories from the fixed list.
    pub categories: Vec<Category>,
}

/// A parsed, validated `mod.toml`.
#[derive(Clone, PartialEq, Debug)]
pub struct ModManifest {
    /// `[package]`.
    pub package: PackageMetadata,
    /// `[dependencies]`: id → requirement.
    pub dependencies: BTreeMap<PackageId, VersionReq>,
    /// `[conflicts]`: id → requirement.
    pub conflicts: BTreeMap<PackageId, VersionReq>,
    /// `[metadata]`, kept verbatim.
    pub metadata: Option<toml::Table>,
}

impl ModManifest {
    /// Parse and fully validate (including the licence policy) `mod.toml` text. Checks that need
    /// the package's files (licence files exist, …) are done by `pwc-package`.
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        check_format(text)?;
        let raw: RawManifest = toml::from_str(text).map_err(toml_error)?;
        raw.validate()
    }

    /// Read and parse a `mod.toml` file.
    pub fn from_file(path: &Path) -> Result<Self, ManifestError> {
        let text = std::fs::read_to_string(path).map_err(|source| ManifestError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&text)
    }

    /// Serialise back to canonical `mod.toml` text (keys in spec order).
    ///
    /// Defaults are written out (`kind`, `edition`, `readme`), so the text means the same thing even
    /// if a future format changes a default; empty optional lists and tables are omitted.
    pub fn to_toml_string(&self) -> String {
        let p = &self.package;
        let mut out = String::new();
        let _ = writeln!(out, "format = {}", crate::FORMAT);
        out.push_str("\n[package]\n");
        push_str_kv(&mut out, "id", p.id.as_str());
        push_str_kv(&mut out, "name", &p.name);
        push_str_kv(&mut out, "version", &p.version.to_string());
        push_str_kv(&mut out, "description", &p.description);
        push_array_kv(&mut out, "authors", p.authors.iter().map(String::as_str));
        push_str_kv(&mut out, "license", &p.license);
        push_array_kv(
            &mut out,
            "license-files",
            p.license_files.iter().map(String::as_str),
        );
        push_str_kv(&mut out, "kind", p.kind.as_str());
        if let Some(req) = &p.pwc_api {
            push_str_kv(&mut out, "pwc-api", &req.to_string());
        }
        push_str_kv(&mut out, "edition", p.edition.as_str());
        push_str_kv(&mut out, "readme", &p.readme);
        for (key, value) in [
            ("icon", &p.icon),
            ("repository", &p.repository),
            ("homepage", &p.homepage),
            ("documentation", &p.documentation),
        ] {
            if let Some(value) = value {
                push_str_kv(&mut out, key, value);
            }
        }
        if !p.keywords.is_empty() {
            push_array_kv(&mut out, "keywords", p.keywords.iter().map(String::as_str));
        }
        if !p.categories.is_empty() {
            push_array_kv(
                &mut out,
                "categories",
                p.categories.iter().map(|c| c.as_str()),
            );
        }
        for (section, table) in [
            ("dependencies", &self.dependencies),
            ("conflicts", &self.conflicts),
        ] {
            if !table.is_empty() {
                let _ = write!(out, "\n[{section}]\n");
                for (id, req) in table {
                    let _ = writeln!(out, "{} = {}", quote(id.as_str()), quote(&req.to_string()));
                }
            }
        }
        if let Some(metadata) = &self.metadata {
            let mut wrapper = toml::Table::new();
            wrapper.insert("metadata".to_string(), toml::Value::Table(metadata.clone()));
            out.push('\n');
            out.push_str(&toml::to_string(&wrapper).expect("a TOML table always serialises"));
        }
        out
    }

    /// Shorthands.
    pub fn id(&self) -> &PackageId {
        &self.package.id
    }

    /// The package version.
    pub fn version(&self) -> &Version {
        &self.package.version
    }

    /// The package kind.
    pub fn kind(&self) -> ModKind {
        self.package.kind
    }
}

/// Longest display name, in characters.
const MAX_NAME_CHARS: usize = 64;
/// Longest description, in characters.
const MAX_DESCRIPTION_CHARS: usize = 280;
/// Most keywords.
const MAX_KEYWORDS: usize = 5;
/// Longest keyword, in characters.
const MAX_KEYWORD_CHARS: usize = 24;

/// `mod.toml` as written, before validation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct RawManifest {
    /// Checked by [`check_format`] before deserialising.
    #[serde(rename = "format")]
    _format: serde::de::IgnoredAny,
    package: Option<RawPackage>,
    dependencies: Option<toml::Table>,
    conflicts: Option<toml::Table>,
    metadata: Option<toml::Table>,
}

/// `[package]` as written. Every key is optional here so a missing one gets a precise error.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct RawPackage {
    id: Option<String>,
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    authors: Option<Vec<String>>,
    license: Option<String>,
    license_files: Option<Vec<String>>,
    kind: Option<String>,
    pwc_api: Option<String>,
    edition: Option<String>,
    readme: Option<String>,
    icon: Option<String>,
    repository: Option<String>,
    homepage: Option<String>,
    documentation: Option<String>,
    keywords: Option<Vec<String>>,
    categories: Option<Vec<String>>,
}

impl RawManifest {
    fn validate(self) -> Result<ModManifest, ManifestError> {
        let package = self
            .package
            .ok_or(ManifestError::Missing("package"))?
            .validate()?;
        let dependencies = parse_requirements(self.dependencies, "dependencies", true)?;
        let conflicts = parse_requirements(self.conflicts, "conflicts", false)?;

        if dependencies.contains_key(&package.id) {
            return Err(ManifestError::invalid(
                dep_key("dependencies", package.id.as_str()),
                package.id.as_str(),
                "a package cannot depend on itself",
            ));
        }
        if conflicts.contains_key(&package.id) {
            return Err(ManifestError::invalid(
                dep_key("conflicts", package.id.as_str()),
                package.id.as_str(),
                "a package cannot conflict with itself",
            ));
        }
        if let Some(id) = dependencies.keys().find(|id| conflicts.contains_key(*id)) {
            return Err(ManifestError::invalid(
                dep_key("conflicts", id.as_str()),
                id.as_str(),
                "also listed in [dependencies]; an id may be in only one of the two tables",
            ));
        }
        if package.kind == ModKind::Bundle && dependencies.is_empty() {
            return Err(ManifestError::invalid(
                "dependencies",
                "",
                "a bundle must have at least one dependency",
            ));
        }
        Ok(ModManifest {
            package,
            dependencies,
            conflicts,
            metadata: self.metadata,
        })
    }
}

impl RawPackage {
    fn validate(self) -> Result<PackageMetadata, ManifestError> {
        let id = PackageId::parse_as(
            &self.id.ok_or(ManifestError::Missing("package.id"))?,
            "package.id",
        )?;

        let name = self.name.ok_or(ManifestError::Missing("package.name"))?;
        let name_chars = name.chars().count();
        if name_chars == 0 || name_chars > MAX_NAME_CHARS {
            return Err(ManifestError::invalid(
                "package.name",
                name,
                "must be 1–64 characters",
            ));
        }
        if has_control(&name) {
            return Err(ManifestError::invalid(
                "package.name",
                name,
                "must not contain control characters",
            ));
        }
        if name.trim().is_empty() {
            return Err(ManifestError::invalid(
                "package.name",
                name,
                "must not be blank",
            ));
        }

        let version = parse_version(
            "package.version",
            &self
                .version
                .ok_or(ManifestError::Missing("package.version"))?,
        )?;

        let description = self
            .description
            .ok_or(ManifestError::Missing("package.description"))?;
        let description_chars = description.chars().count();
        if description_chars == 0 || description_chars > MAX_DESCRIPTION_CHARS {
            return Err(ManifestError::invalid(
                "package.description",
                description,
                "must be one sentence of 1–280 characters",
            ));
        }
        if description.contains(['\n', '\r']) {
            return Err(ManifestError::invalid(
                "package.description",
                description,
                "must be one line (no newlines)",
            ));
        }
        if has_control(&description) {
            return Err(ManifestError::invalid(
                "package.description",
                description,
                "must not contain control characters",
            ));
        }
        if description.trim().is_empty() {
            return Err(ManifestError::invalid(
                "package.description",
                description,
                "must not be blank",
            ));
        }

        let authors = self
            .authors
            .ok_or(ManifestError::Missing("package.authors"))?;
        if authors.is_empty() {
            return Err(ManifestError::invalid(
                "package.authors",
                "[]",
                "must list at least one author",
            ));
        }
        for author in &authors {
            check_author(author).map_err(|reason| {
                ManifestError::invalid("package.authors", author.as_str(), reason)
            })?;
        }

        let kind = match self.kind {
            None => ModKind::default(),
            Some(kind) => ModKind::from_name(&kind).ok_or_else(|| {
                ManifestError::invalid(
                    "package.kind",
                    kind,
                    "must be \"mod\", \"library\" or \"bundle\"",
                )
            })?,
        };

        let license = self
            .license
            .ok_or(ManifestError::Missing("package.license"))?;
        check_package_license(&license, kind)?;

        let license_files = self
            .license_files
            .ok_or(ManifestError::Missing("package.license-files"))?;
        if license_files.is_empty() {
            return Err(ManifestError::invalid(
                "package.license-files",
                "[]",
                "must list at least one licence text",
            ));
        }
        check_path_list("package.license-files", &license_files)?;

        let pwc_api = match (kind, self.pwc_api) {
            (ModKind::Bundle, Some(req)) => {
                return Err(ManifestError::invalid(
                    "package.pwc-api",
                    req,
                    "a bundle contains no code and must omit `pwc-api`",
                ));
            }
            (ModKind::Bundle, None) => None,
            (_, None) => return Err(ManifestError::Missing("package.pwc-api")),
            (_, Some(req)) => Some(parse_req("package.pwc-api", &req)?),
        };

        let edition = match self.edition {
            None => Edition::default(),
            Some(edition) => Edition::from_name(&edition).ok_or_else(|| {
                ManifestError::invalid("package.edition", edition, "must be \"2021\" or \"2024\"")
            })?,
        };

        let readme = self.readme.unwrap_or_else(|| "README.md".to_string());
        check_package_path(&readme)
            .map_err(|reason| ManifestError::invalid("package.readme", readme.as_str(), reason))?;

        if let Some(icon) = &self.icon {
            check_package_path(icon)
                .map_err(|reason| ManifestError::invalid("package.icon", icon.as_str(), reason))?;
            if !icon.starts_with("assets/") {
                return Err(ManifestError::invalid(
                    "package.icon",
                    icon.as_str(),
                    "must be under `assets/`",
                ));
            }
            if !(icon.ends_with(".svg") || icon.ends_with(".png")) {
                return Err(ManifestError::invalid(
                    "package.icon",
                    icon.as_str(),
                    "must be a `.svg` or `.png` file",
                ));
            }
        }

        for (key, url) in [
            ("package.repository", &self.repository),
            ("package.homepage", &self.homepage),
            ("package.documentation", &self.documentation),
        ] {
            if let Some(url) = url {
                check_url(url)
                    .map_err(|reason| ManifestError::invalid(key, url.as_str(), reason))?;
            }
        }

        let keywords = self.keywords.unwrap_or_default();
        if keywords.len() > MAX_KEYWORDS {
            return Err(ManifestError::invalid(
                "package.keywords",
                keywords.join(", "),
                "at most 5 keywords",
            ));
        }
        for (i, keyword) in keywords.iter().enumerate() {
            let valid = (1..=MAX_KEYWORD_CHARS).contains(&keyword.len())
                && keyword
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
            if !valid {
                return Err(ManifestError::invalid(
                    "package.keywords",
                    keyword.as_str(),
                    "each keyword must be 1–24 characters of `a-z`, `0-9` and `-`",
                ));
            }
            if keywords[..i].contains(keyword) {
                return Err(ManifestError::invalid(
                    "package.keywords",
                    keyword.as_str(),
                    "listed twice",
                ));
            }
        }

        let mut categories = Vec::new();
        for category in self.categories.unwrap_or_default() {
            let parsed = Category::from_name(&category).ok_or_else(|| {
                let all: Vec<&str> = Category::ALL.iter().map(|c| c.as_str()).collect();
                ManifestError::invalid(
                    "package.categories",
                    category.as_str(),
                    format!("not a category; use one of {}", all.join(", ")),
                )
            })?;
            if categories.contains(&parsed) {
                return Err(ManifestError::invalid(
                    "package.categories",
                    category,
                    "listed twice",
                ));
            }
            categories.push(parsed);
        }

        Ok(PackageMetadata {
            id,
            name,
            version,
            description,
            authors,
            license,
            license_files,
            kind,
            pwc_api,
            edition,
            readme,
            icon: self.icon,
            repository: self.repository,
            homepage: self.homepage,
            documentation: self.documentation,
            keywords,
            categories,
        })
    }
}

/// The error key of a `[dependencies]`/`[conflicts]` entry: `dependencies."pwc.hotbar"`.
fn dep_key(section: &str, id: &str) -> String {
    format!("{section}.{}", quote(id))
}

/// Parse `[dependencies]` (string or `{ version = "…" }` values) or `[conflicts]` (strings only).
fn parse_requirements(
    table: Option<toml::Table>,
    section: &str,
    allow_table: bool,
) -> Result<BTreeMap<PackageId, VersionReq>, ManifestError> {
    let mut out = BTreeMap::new();
    for (key, value) in table.unwrap_or_default() {
        let entry_key = dep_key(section, &key);
        let id = PackageId::parse_as(&key, &entry_key).map_err(|err| match (&err, &value) {
            // `pwc.hotbar = "^1"` without quotes is a nested table `pwc = { hotbar = … }`.
            (
                ManifestError::Invalid {
                    key: k,
                    value: v,
                    reason,
                },
                toml::Value::Table(_),
            ) if !key.contains('.') => ManifestError::invalid(
                k.clone(),
                v.clone(),
                format!("{reason} (ids containing `.` must be quoted: `\"{key}.name\" = \"…\"`)"),
            ),
            _ => err,
        })?;
        let req = match value {
            toml::Value::String(req) => req,
            toml::Value::Table(mut t) if allow_table => {
                if let Some(unknown) = t.keys().find(|k| k.as_str() != "version") {
                    return Err(ManifestError::invalid(
                        format!("{entry_key}.{unknown}"),
                        "",
                        "unknown key (a dependency table only has `version`)",
                    ));
                }
                match t.remove("version") {
                    Some(toml::Value::String(req)) => req,
                    Some(other) => {
                        return Err(ManifestError::invalid(
                            format!("{entry_key}.version"),
                            other.to_string(),
                            "must be a version requirement string",
                        ));
                    }
                    None => {
                        return Err(ManifestError::invalid(
                            entry_key,
                            "{}",
                            "a dependency table needs `version`",
                        ));
                    }
                }
            }
            other => {
                let expected = if allow_table {
                    "must be a version requirement string or a table `{ version = \"…\" }`"
                } else {
                    "must be a version requirement string"
                };
                return Err(ManifestError::invalid(
                    entry_key,
                    other.to_string(),
                    expected,
                ));
            }
        };
        out.insert(id, parse_req(&entry_key, &req)?);
    }
    Ok(out)
}

/// A SemVer version without build metadata.
pub(crate) fn parse_version(key: &str, s: &str) -> Result<Version, ManifestError> {
    let version = Version::parse(s)
        .map_err(|e| ManifestError::invalid(key, s, format!("not a SemVer version ({e})")))?;
    if !version.build.is_empty() {
        return Err(ManifestError::invalid(
            key,
            s,
            "build metadata (`+…`) is not allowed",
        ));
    }
    Ok(version)
}

/// A SemVer requirement (`^1.0`, `>=2, <3`, `=1.4.2`, `*`).
pub(crate) fn parse_req(key: &str, s: &str) -> Result<VersionReq, ManifestError> {
    VersionReq::parse(s)
        .map_err(|e| ManifestError::invalid(key, s, format!("not a SemVer requirement ({e})")))
}

/// `"Name"` or `"Name <email>"`.
fn check_author(author: &str) -> Result<(), &'static str> {
    if author.trim().is_empty() {
        return Err("an author must not be blank");
    }
    if has_control(author) {
        return Err("an author must not contain control characters");
    }
    if author.trim() != author {
        return Err("an author must not have leading or trailing spaces");
    }
    match author.find('<') {
        None if author.contains('>') => Err("an author is `Name` or `Name <email>`"),
        None => Ok(()),
        Some(open) => {
            let name = author[..open].trim_end();
            let rest = &author[open + 1..];
            let email = rest
                .strip_suffix('>')
                .ok_or("an author is `Name` or `Name <email>` (the email must end the entry)")?;
            if name.is_empty() {
                return Err("an author is `Name` or `Name <email>` (the name is missing)");
            }
            if name.contains('>') || email.contains(['<', '>']) {
                return Err("an author is `Name` or `Name <email>`");
            }
            if email.is_empty() || email.contains(char::is_whitespace) || !email.contains('@') {
                return Err("the email of an author must look like `someone@example.org`");
            }
            Ok(())
        }
    }
}

/// An `https://` URL with a host and no whitespace.
fn check_url(url: &str) -> Result<(), &'static str> {
    let rest = url
        .strip_prefix("https://")
        .ok_or("must be an `https://` URL")?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() {
        return Err("must be an `https://` URL with a host");
    }
    if url.contains(char::is_whitespace) || has_control(url) {
        return Err("a URL must not contain spaces or control characters");
    }
    Ok(())
}

/// Every entry is a valid package path and none is listed twice.
fn check_path_list(key: &str, paths: &[String]) -> Result<(), ManifestError> {
    for (i, path) in paths.iter().enumerate() {
        check_package_path(path)
            .map_err(|reason| ManifestError::invalid(key, path.as_str(), reason))?;
        if paths[..i].contains(path) {
            return Err(ManifestError::invalid(key, path.as_str(), "listed twice"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The example from docs/spec/mod-manifest.md.
    const SPEC_EXAMPLE: &str = r#"format = 1

[package]
id = "pwc.inventory"
name = "Inventory"
version = "1.0.0"
description = "Your held materials as a list (press I); equip any of them into the hotbar."
authors = ["Project Watt Cubed contributors"]
license = "Apache-2.0 OR MIT"
license-files = ["LICENSES/Apache-2.0.txt", "LICENSES/MIT.txt"]
kind = "mod"
pwc-api = "^1.0"
readme = "README.md"
icon = "assets/icon.svg"
repository = "https://github.com/gusahlg/pwc-package-manager"
keywords = ["inventory", "ui"]
categories = ["interface"]

[dependencies]
"pwc.hotbar" = "^1.0"

[conflicts]
"someone.other-inventory" = "*"

[metadata]
anything = "tools may read this; pwc ignores it"
"#;

    const MINIMAL: &str = r#"format = 1
[package]
id = "foo.bar"
name = "Bar"
version = "0.1.0"
description = "A bar."
authors = ["Foo"]
license = "AGPL-3.0-or-later"
license-files = ["LICENSE"]
pwc-api = "^1"
"#;

    fn id(s: &str) -> PackageId {
        PackageId::parse(s).unwrap()
    }

    /// `MINIMAL` with `[package]` key `key` replaced by `line` (or removed when `line` is empty).
    fn with_package_line(key: &str, line: &str) -> String {
        let mut out = String::new();
        let mut replaced = false;
        for l in MINIMAL.lines() {
            if l.starts_with(&format!("{key} =")) {
                replaced = true;
                if !line.is_empty() {
                    out.push_str(line);
                    out.push('\n');
                }
            } else {
                out.push_str(l);
                out.push('\n');
            }
        }
        if !replaced {
            out.push_str(line);
            out.push('\n');
        }
        out
    }

    fn parse_err(text: &str) -> ManifestError {
        match ModManifest::parse(text) {
            Ok(m) => panic!("expected an error, parsed {m:?} from\n{text}"),
            Err(e) => e,
        }
    }

    /// Assert the error is `Invalid` for `key`, and that its message names the key.
    fn assert_invalid(text: &str, key: &str) {
        let err = parse_err(text);
        assert!(
            matches!(&err, ManifestError::Invalid { key: k, .. } if k == key),
            "expected invalid `{key}`, got: {err}\n{text}"
        );
        assert!(err.to_string().contains(&format!("`{key}`")), "{err}");
    }

    fn assert_round_trip(m: &ModManifest) {
        let text = m.to_toml_string();
        let back = ModManifest::parse(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
        assert_eq!(&back, m, "{text}");
        assert_eq!(
            back.to_toml_string(),
            text,
            "canonical text is a fixed point"
        );
    }

    #[test]
    fn spec_example_parses() {
        let m = ModManifest::parse(SPEC_EXAMPLE).unwrap();
        let p = &m.package;
        assert_eq!(m.id().as_str(), "pwc.inventory");
        assert_eq!(p.name, "Inventory");
        assert_eq!(m.version(), &Version::new(1, 0, 0));
        assert_eq!(p.authors, ["Project Watt Cubed contributors"]);
        assert_eq!(p.license, "Apache-2.0 OR MIT");
        assert_eq!(
            p.license_files,
            ["LICENSES/Apache-2.0.txt", "LICENSES/MIT.txt"]
        );
        assert_eq!(m.kind(), ModKind::Mod);
        assert_eq!(p.pwc_api, Some(VersionReq::parse("^1.0").unwrap()));
        assert_eq!(p.edition, Edition::E2024);
        assert_eq!(p.readme, "README.md");
        assert_eq!(p.icon.as_deref(), Some("assets/icon.svg"));
        assert_eq!(
            p.repository.as_deref(),
            Some("https://github.com/gusahlg/pwc-package-manager")
        );
        assert_eq!(p.homepage, None);
        assert_eq!(p.keywords, ["inventory", "ui"]);
        assert_eq!(p.categories, [Category::Interface]);
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(
            m.dependencies[&id("pwc.hotbar")],
            VersionReq::parse("^1.0").unwrap()
        );
        assert_eq!(
            m.conflicts[&id("someone.other-inventory")],
            VersionReq::STAR
        );
        let metadata = m.metadata.as_ref().unwrap();
        assert_eq!(
            metadata["anything"].as_str(),
            Some("tools may read this; pwc ignores it")
        );
        assert_round_trip(&m);
    }

    #[test]
    fn spec_example_canonical_text() {
        // The canonical form of the spec example: the example itself plus the explicit default edition.
        let m = ModManifest::parse(SPEC_EXAMPLE).unwrap();
        let expected = SPEC_EXAMPLE.replace(
            "pwc-api = \"^1.0\"\n",
            "pwc-api = \"^1.0\"\nedition = \"2024\"\n",
        );
        assert_eq!(m.to_toml_string(), expected);
    }

    #[test]
    fn minimal_defaults() {
        let m = ModManifest::parse(MINIMAL).unwrap();
        assert_eq!(m.kind(), ModKind::Mod);
        assert_eq!(m.package.edition, Edition::E2024);
        assert_eq!(m.package.readme, "README.md");
        assert!(
            m.package.icon.is_none()
                && m.package.keywords.is_empty()
                && m.package.categories.is_empty()
        );
        assert!(m.dependencies.is_empty() && m.conflicts.is_empty() && m.metadata.is_none());
        assert_round_trip(&m);
    }

    #[test]
    fn library_and_bundle() {
        let lib = ModManifest::parse(&with_package_line("kind", "kind = \"library\"")).unwrap();
        assert_eq!(lib.kind(), ModKind::Library);
        assert!(lib.kind().has_code());
        assert_round_trip(&lib);

        let bundle_text = r#"format = 1
[package]
id = "pwc.essentials"
name = "Essentials"
version = "1.0.0"
description = "The essential mods."
authors = ["Project Watt Cubed contributors"]
license = "CC0-1.0"
license-files = ["LICENSE"]
kind = "bundle"
categories = ["bundle"]
[dependencies]
"pwc.hotbar" = "^1.0"
"pwc.inventory" = { version = "^1.0" }
"#;
        let bundle = ModManifest::parse(bundle_text).unwrap();
        assert_eq!(bundle.kind(), ModKind::Bundle);
        assert!(!bundle.kind().has_code());
        assert_eq!(bundle.package.pwc_api, None);
        assert_eq!(bundle.dependencies.len(), 2);
        assert_round_trip(&bundle);

        // A bundle must not declare pwc-api and must have a dependency.
        assert_invalid(
            &bundle_text.replace("kind = \"bundle\"", "kind = \"bundle\"\npwc-api = \"^1\""),
            "package.pwc-api",
        );
        let no_deps = bundle_text.split("[dependencies]").next().unwrap();
        assert_invalid(no_deps, "dependencies");
    }

    #[test]
    fn format_is_checked_first() {
        assert!(matches!(
            parse_err(&MINIMAL.replace("format = 1\n", "")),
            ManifestError::Missing("format")
        ));
        // A newer format with keys this pwc does not know is "upgrade pwc", not "unknown key".
        let newer = MINIMAL.replace("format = 1", "format = 2\nfuture-key = 1");
        let err = parse_err(&newer);
        assert!(matches!(err, ManifestError::Format { found: 2 }), "{err}");
        assert!(err.to_string().contains("upgrade pwc"));
        assert!(matches!(
            parse_err("this is not toml"),
            ManifestError::Toml(_)
        ));
    }

    #[test]
    fn missing_required_keys() {
        for (key, missing) in [
            ("id", "package.id"),
            ("name", "package.name"),
            ("version", "package.version"),
            ("description", "package.description"),
            ("authors", "package.authors"),
            ("license", "package.license"),
            ("license-files", "package.license-files"),
            ("pwc-api", "package.pwc-api"),
        ] {
            let err = parse_err(&with_package_line(key, ""));
            assert!(
                matches!(err, ManifestError::Missing(k) if k == missing),
                "{key}: {err}"
            );
            assert!(err.to_string().contains(missing));
        }
        assert!(matches!(
            parse_err("format = 1\n"),
            ManifestError::Missing("package")
        ));
        // pwc-api is required for libraries too.
        let lib = with_package_line("kind", "kind = \"library\"");
        let lib = lib.replace("pwc-api = \"^1\"\n", "");
        assert!(matches!(
            parse_err(&lib),
            ManifestError::Missing("package.pwc-api")
        ));
    }

    #[test]
    fn unknown_keys_are_errors() {
        for text in [
            format!("{MINIMAL}typo = 1\n"),
            MINIMAL.replace("[package]", "unknown = true\n[package]"),
            with_package_line("licence", "licence = \"AGPL-3.0-or-later\""),
            with_package_line("pwc_api", "pwc_api = \"^1\""),
            format!("{MINIMAL}[dependencies]\n\"a.b\" = {{ version = \"1\", path = \"x\" }}\n"),
            format!("{MINIMAL}[build]\nprofile = \"dev\"\n"),
        ] {
            let err = parse_err(&text);
            let msg = err.to_string();
            assert!(
                matches!(err, ManifestError::Toml(_) | ManifestError::Invalid { .. }),
                "{msg}"
            );
            assert!(msg.contains("unknown"), "{msg}");
        }
        let msg = parse_err(&with_package_line(
            "licence",
            "licence = \"AGPL-3.0-or-later\"",
        ))
        .to_string();
        assert!(
            msg.contains("licence"),
            "the error names the unknown key: {msg}"
        );
    }

    #[test]
    fn wrong_types_are_errors() {
        for (key, line) in [
            ("id", "id = 5"),
            ("authors", "authors = \"Foo\""),
            ("license-files", "license-files = \"LICENSE\""),
            ("keywords", "keywords = [1]"),
            ("edition", "edition = 2024"),
        ] {
            let err = parse_err(&with_package_line(key, line));
            assert!(matches!(err, ManifestError::Toml(_)), "{line}: {err}");
            assert!(err.to_string().contains(key), "{line}: {err}");
        }
    }

    #[test]
    fn id_rules() {
        assert_invalid(&with_package_line("id", "id = \"Foo.Bar\""), "package.id");
        assert_invalid(&with_package_line("id", "id = \"foobar\""), "package.id");
        assert_invalid(&with_package_line("id", "id = \"foo.bar-\""), "package.id");
        // The pwc namespace parses (first-party packages are ordinary manifests).
        assert!(
            ModManifest::parse(&with_package_line("id", "id = \"pwc.thing\""))
                .unwrap()
                .id()
                .is_first_party()
        );
    }

    #[test]
    fn name_rules() {
        let name_64 = "n".repeat(64);
        let name_65 = "n".repeat(65);
        assert!(
            ModManifest::parse(&with_package_line("name", &format!("name = \"{name_64}\"")))
                .is_ok()
        );
        assert!(
            ModManifest::parse(&with_package_line("name", "name = \"Ünïcødé ✓ name\"")).is_ok()
        );
        for bad in [
            "\"\"",
            &format!("\"{name_65}\""),
            "\"tab\\there\"",
            "\"new\\nline\"",
            "\"   \"",
        ] {
            assert_invalid(
                &with_package_line("name", &format!("name = {bad}")),
                "package.name",
            );
        }
    }

    #[test]
    fn version_rules() {
        for ok in ["1.4.2", "2.0.0-beta.1", "0.0.1", "10.20.30-rc.1.x"] {
            let m = ModManifest::parse(&with_package_line(
                "version",
                &format!("version = \"{ok}\""),
            ))
            .unwrap();
            assert_eq!(m.version().to_string(), ok);
        }
        for bad in [
            "1.0",
            "1",
            "v1.0.0",
            "1.0.0+build.5",
            "1.0.0-beta+exp",
            "01.0.0",
            "",
            "latest",
        ] {
            assert_invalid(
                &with_package_line("version", &format!("version = \"{bad}\"")),
                "package.version",
            );
        }
    }

    #[test]
    fn description_rules() {
        let d280 = "d".repeat(280);
        assert!(
            ModManifest::parse(&with_package_line(
                "description",
                &format!("description = \"{d280}\"")
            ))
            .is_ok()
        );
        for bad in [
            "\"\"",
            &format!("\"{}\"", "d".repeat(281)),
            "\"two\\nlines\"",
            "\"cr\\rhere\"",
            "\"bell\\u0007\"",
            "\" \"",
        ] {
            assert_invalid(
                &with_package_line("description", &format!("description = {bad}")),
                "package.description",
            );
        }
    }

    #[test]
    fn author_rules() {
        for ok in [
            r#"["Foo"]"#,
            r#"["Foo Bar <foo@example.org>"]"#,
            r#"["A", "B <b@c.d>", "Project Watt Cubed contributors"]"#,
            r#"["Foo<foo@x.y>"]"#,
        ] {
            assert!(
                ModManifest::parse(&with_package_line("authors", &format!("authors = {ok}")))
                    .is_ok(),
                "{ok}"
            );
        }
        for bad in [
            "[]",
            r#"[""]"#,
            r#"["  "]"#,
            r#"[" Foo"]"#,
            r#"["Foo "]"#,
            r#"["<foo@example.org>"]"#,
            r#"["Foo <foo@example.org"]"#,
            r#"["Foo <foo@example.org> extra"]"#,
            r#"["Foo <>"]"#,
            r#"["Foo <not an email>"]"#,
            r#"["Foo <no-at-sign>"]"#,
            r#"["Foo > Bar"]"#,
            r#"["Foo <a@b> <c@d>"]"#,
            r#"["Foo\nBar"]"#,
        ] {
            assert_invalid(
                &with_package_line("authors", &format!("authors = {bad}")),
                "package.authors",
            );
        }
    }

    #[test]
    fn license_policy_is_enforced() {
        let with_license = |kind: &str, license: &str| {
            let text = with_package_line("license", &format!("license = \"{license}\""));
            match kind {
                "mod" => text,
                "library" => text.replace("[package]", "[package]\nkind = \"library\""),
                _ => {
                    text.replace("pwc-api = \"^1\"", "kind = \"bundle\"")
                        + "[dependencies]\n\"a.b\" = \"1\"\n"
                }
            }
        };
        for kind in ["mod", "library"] {
            for ok in [
                "Apache-2.0 OR MIT",
                "MIT",
                "MPL-2.0",
                "GPL-3.0-or-later",
                "AGPL-3.0-or-later",
                "AGPL-3.0-or-later AND CC-BY-SA-4.0",
                "(CC0-1.0 AND AGPL-3.0-or-later)",
                "Apache-2.0 WITH LLVM-exception",
                "(Apache-2.0 OR MIT) AND CC-BY-4.0",
            ] {
                let m = ModManifest::parse(&with_license(kind, ok))
                    .unwrap_or_else(|e| panic!("{kind} {ok}: {e}"));
                assert_eq!(m.package.license, ok);
            }
        }
        for ok in [
            "CC0-1.0",
            "Apache-2.0 OR MIT",
            "AGPL-3.0-or-later",
            "CC-BY-4.0 AND OFL-1.1",
            "MIT OR CC-BY-4.0",
        ] {
            assert!(
                ModManifest::parse(&with_license("bundle", ok)).is_ok(),
                "bundle {ok}"
            );
        }
        let no_software = |expr: &str| LicenseError::NoSoftwareLicence {
            expression: expr.into(),
        };
        for (kind, bad, expected) in [
            (
                "mod",
                "LicenseRef-Proprietary",
                LicenseError::NotAllowed("LicenseRef-Proprietary".into()),
            ),
            (
                "mod",
                "NOASSERTION",
                LicenseError::NotAllowed("NOASSERTION".into()),
            ),
            ("mod", "NONE", LicenseError::NotAllowed("NONE".into())),
            (
                "mod",
                "SSPL-1.0",
                LicenseError::NotAllowed("SSPL-1.0".into()),
            ),
            (
                "mod",
                "GPL-3.0+",
                LicenseError::NotAllowed("GPL-3.0+".into()),
            ),
            (
                "mod",
                "MIT AND CC-BY-NC-4.0",
                LicenseError::NotAllowed("CC-BY-NC-4.0".into()),
            ),
            (
                "mod",
                "Apache-2.0 WITH Foo-exception",
                LicenseError::ExceptionNotAllowed("Foo-exception".into()),
            ),
            ("mod", "MIT AND", LicenseError::Syntax("MIT AND".into())),
            ("mod", "CC-BY-SA-4.0", no_software("CC-BY-SA-4.0")),
            ("mod", "MIT OR CC-BY-4.0", no_software("MIT OR CC-BY-4.0")),
            (
                "library",
                "CC-BY-4.0 AND OFL-1.1",
                no_software("CC-BY-4.0 AND OFL-1.1"),
            ),
            (
                "library",
                "CC-BY-4.0 WITH LLVM-exception",
                no_software("CC-BY-4.0 WITH LLVM-exception"),
            ),
            (
                "bundle",
                "LicenseRef-Proprietary",
                LicenseError::NotAllowed("LicenseRef-Proprietary".into()),
            ),
        ] {
            let err = parse_err(&with_license(kind, bad));
            assert!(
                matches!(&err, ManifestError::License(e) if *e == expected),
                "{kind} {bad}: {err}"
            );
            let msg = err.to_string();
            assert!(msg.contains("POLICY.md"), "{msg}");
            assert!(
                msg.contains("proprietary licences are not accepted"),
                "{msg}"
            );
        }
    }

    use crate::LicenseError;

    #[test]
    fn license_file_rules() {
        let m = ModManifest::parse(&with_package_line(
            "license-files",
            r#"license-files = ["licenses/MIT.txt", "LICENSE-APACHE"]"#,
        ))
        .unwrap();
        assert_eq!(
            m.package.license_files,
            ["licenses/MIT.txt", "LICENSE-APACHE"]
        );
        for bad in [
            "[]",
            r#"["/abs/LICENSE"]"#,
            r#"["../LICENSE"]"#,
            r#"["a\\b"]"#,
            r#"["LICENSE", "LICENSE"]"#,
            r#"[""]"#,
            r#"["dir/"]"#,
        ] {
            assert_invalid(
                &with_package_line("license-files", &format!("license-files = {bad}")),
                "package.license-files",
            );
        }
    }

    #[test]
    fn kind_and_edition_rules() {
        assert_invalid(
            &with_package_line("kind", "kind = \"plugin\""),
            "package.kind",
        );
        assert_invalid(&with_package_line("kind", "kind = \"Mod\""), "package.kind");
        let m = ModManifest::parse(&with_package_line("edition", "edition = \"2021\"")).unwrap();
        assert_eq!(m.package.edition, Edition::E2021);
        assert_round_trip(&m);
        assert_invalid(
            &with_package_line("edition", "edition = \"2018\""),
            "package.edition",
        );
        assert_invalid(
            &with_package_line("pwc-api", "pwc-api = \"not a req\""),
            "package.pwc-api",
        );
        assert_invalid(
            &with_package_line("pwc-api", "pwc-api = \"\""),
            "package.pwc-api",
        );
    }

    #[test]
    fn readme_and_icon_rules() {
        let m = ModManifest::parse(&with_package_line("readme", "readme = \"docs/README.md\""))
            .unwrap();
        assert_eq!(m.package.readme, "docs/README.md");
        assert_invalid(
            &with_package_line("readme", "readme = \"../README.md\""),
            "package.readme",
        );
        assert_invalid(
            &with_package_line("readme", "readme = \"\""),
            "package.readme",
        );
        for ok in ["assets/icon.svg", "assets/img/icon.png"] {
            assert!(
                ModManifest::parse(&with_package_line("icon", &format!("icon = \"{ok}\""))).is_ok(),
                "{ok}"
            );
        }
        for bad in [
            "icon.svg",
            "src/icon.svg",
            "assets/icon.jpg",
            "assets/icon.SVG",
            "assets/../icon.svg",
            "assets/",
            "/assets/icon.svg",
        ] {
            assert_invalid(
                &with_package_line("icon", &format!("icon = \"{bad}\"")),
                "package.icon",
            );
        }
    }

    #[test]
    fn url_rules() {
        for key in ["repository", "homepage", "documentation"] {
            let ok = ModManifest::parse(&with_package_line(
                key,
                &format!("{key} = \"https://example.org/x?y#z\""),
            ))
            .unwrap();
            assert_round_trip(&ok);
            for bad in [
                "http://example.org",
                "https://",
                "https:///path",
                "ftp://x.y",
                "example.org",
                "https://exa mple.org",
                "",
            ] {
                assert_invalid(
                    &with_package_line(key, &format!("{key} = \"{bad}\"")),
                    &format!("package.{key}"),
                );
            }
        }
    }

    #[test]
    fn keyword_rules() {
        let five = r#"keywords = ["a", "b-c", "d0", "e", "x23456789012345678901234"]"#;
        assert!(ModManifest::parse(&with_package_line("keywords", five)).is_ok());
        for bad in [
            r#"["a", "b", "c", "d", "e", "f"]"#,
            r#"[""]"#,
            r#"["Upper"]"#,
            r#"["with space"]"#,
            r#"["under_score"]"#,
            r#"["x234567890123456789012345"]"#,
            r#"["dup", "dup"]"#,
        ] {
            assert_invalid(
                &with_package_line("keywords", &format!("keywords = {bad}")),
                "package.keywords",
            );
        }
    }

    #[test]
    fn category_rules() {
        let all = r#"categories = ["gameplay", "interface", "visuals", "worldgen", "audio", "library", "tools", "bundle"]"#;
        let m = ModManifest::parse(&with_package_line("categories", all)).unwrap();
        assert_eq!(m.package.categories, Category::ALL);
        assert_round_trip(&m);
        for bad in [r#"["ui"]"#, r#"["Gameplay"]"#, r#"["tools", "tools"]"#] {
            assert_invalid(
                &with_package_line("categories", &format!("categories = {bad}")),
                "package.categories",
            );
        }
    }

    #[test]
    fn dependency_rules() {
        let deps = |body: &str| format!("{MINIMAL}[dependencies]\n{body}\n");
        let m = ModManifest::parse(&deps(
            "\"a.b\" = \">=2, <3\"\n\"c.d\" = { version = \"=1.4.2\" }\n\"e.f\" = \"*\"",
        ))
        .unwrap();
        assert_eq!(m.dependencies.len(), 3);
        assert_eq!(
            m.dependencies[&id("c.d")],
            VersionReq::parse("=1.4.2").unwrap()
        );
        assert_round_trip(&m);

        assert_invalid(&deps("\"Bad.Id\" = \"1\""), "dependencies.\"Bad.Id\"");
        assert_invalid(&deps("\"a.b\" = \"not a version\""), "dependencies.\"a.b\"");
        assert_invalid(&deps("\"a.b\" = 1"), "dependencies.\"a.b\"");
        assert_invalid(&deps("\"a.b\" = {}"), "dependencies.\"a.b\"");
        assert_invalid(
            &deps("\"a.b\" = { version = 1 }"),
            "dependencies.\"a.b\".version",
        );
        assert_invalid(
            &deps("\"a.b\" = { version = \"1\", optional = true }"),
            "dependencies.\"a.b\".optional",
        );
        assert_invalid(&deps("\"foo.bar\" = \"1\""), "dependencies.\"foo.bar\"");

        // An unquoted dotted key is a nested table; the error explains the quoting.
        let err = parse_err(&deps("pwc.hotbar = \"^1.0\""));
        assert!(err.to_string().contains("quoted"), "{err}");

        // Conflicts are plain requirement strings.
        let conflicts = |body: &str| format!("{MINIMAL}[conflicts]\n{body}\n");
        assert!(ModManifest::parse(&conflicts("\"a.b\" = \"<2\"")).is_ok());
        assert_invalid(
            &conflicts("\"a.b\" = { version = \"1\" }"),
            "conflicts.\"a.b\"",
        );
        assert_invalid(&conflicts("\"foo.bar\" = \"*\""), "conflicts.\"foo.bar\"");

        // The same id may not be in both tables.
        assert_invalid(
            &format!("{MINIMAL}[dependencies]\n\"a.b\" = \"1\"\n[conflicts]\n\"a.b\" = \"2\"\n"),
            "conflicts.\"a.b\"",
        );
    }

    #[test]
    fn metadata_round_trips() {
        let text = format!(
            "{MINIMAL}[metadata]\nnumber = 3\nflag = true\nlist = [1, 2, 3]\nwhen = 2026-10-03\n[metadata.nested]\nkey = \"value\"\n[[metadata.items]]\nname = \"one\"\n[[metadata.items]]\nname = \"two\"\n"
        );
        let m = ModManifest::parse(&text).unwrap();
        let metadata = m.metadata.as_ref().unwrap();
        assert_eq!(metadata["nested"]["key"].as_str(), Some("value"));
        assert_eq!(metadata["items"].as_array().unwrap().len(), 2);
        assert_round_trip(&m);

        let empty = ModManifest::parse(&format!("{MINIMAL}[metadata]\n")).unwrap();
        assert_eq!(empty.metadata, Some(toml::Table::new()));
        assert_round_trip(&empty);
    }

    #[test]
    fn awkward_strings_round_trip() {
        let mut m = ModManifest::parse(SPEC_EXAMPLE).unwrap();
        m.package.name = "Quote \" back\\slash 'single' ✓".to_string();
        m.package.description = "Back\\slashes; \"quotes\" too — and ünïcode.".to_string();
        m.package.authors = vec![
            "Zoë O'Brien <zoe@example.org>".into(),
            "\"Nick\" Name".into(),
        ];
        m.package.keywords.clear();
        m.package.categories.clear();
        m.package.homepage = Some("https://example.org/\"quoted\"".into());
        m.conflicts.clear();
        m.metadata = None;
        assert_round_trip(&m);
    }

    #[test]
    fn edited_values_are_rechecked_on_parse() {
        // to_toml_string does not validate; a manifest edited into an invalid state is rejected
        // when read back.
        let mut m = ModManifest::parse(MINIMAL).unwrap();
        m.package.license = "CC-BY-SA-4.0".into();
        assert!(matches!(
            ModManifest::parse(&m.to_toml_string()),
            Err(ManifestError::License(
                LicenseError::NoSoftwareLicence { .. }
            ))
        ));
        m.package.license = "LicenseRef-Proprietary".into();
        assert!(matches!(
            ModManifest::parse(&m.to_toml_string()),
            Err(ManifestError::License(LicenseError::NotAllowed(_)))
        ));
    }

    #[test]
    fn from_file_reports_the_path() {
        let err = ModManifest::from_file(Path::new("/nonexistent/pwc/mod.toml")).unwrap_err();
        assert!(matches!(err, ManifestError::Io { .. }));
        assert!(err.to_string().contains("/nonexistent/pwc/mod.toml"));
    }
}
