//! Package ids and instance names.

use crate::ManifestError;

/// A permanent, namespaced package id (`pwc.hotbar`, `gusahlg.better-caves`). See
/// `docs/spec/mod-manifest.md` for the grammar.
#[derive(
    Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, serde::Serialize, serde::Deserialize,
)]
#[serde(try_from = "String", into = "String")]
pub struct PackageId(String);

impl PackageId {
    /// Parse and validate an id.
    pub fn parse(s: &str) -> Result<Self, ManifestError> {
        Self::parse_as(s, "id")
    }

    /// Parse an id that appears as the value or key `key` (named in the error).
    pub(crate) fn parse_as(s: &str, key: &str) -> Result<Self, ManifestError> {
        check_id(s).map_err(|reason| ManifestError::invalid(key, s, reason))?;
        Ok(Self(s.to_string()))
    }

    /// The id text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The namespace (first segment).
    pub fn namespace(&self) -> &str {
        self.0.split('.').next().unwrap_or("")
    }

    /// Whether this id is in the reserved first-party `pwc` namespace.
    pub fn is_first_party(&self) -> bool {
        self.namespace() == "pwc"
    }

    /// The Rust crate name: `.` and `-` replaced by `_` (`pwc.material-names` → `pwc_material_names`).
    pub fn crate_name(&self) -> String {
        self.0.replace(['.', '-'], "_")
    }
}

impl std::fmt::Display for PackageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for PackageId {
    type Err = ManifestError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<String> for PackageId {
    type Error = ManifestError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<PackageId> for String {
    fn from(id: PackageId) -> String {
        id.0
    }
}

/// An instance name: `[a-z0-9][a-z0-9-]{0,63}`.
#[derive(
    Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, serde::Serialize, serde::Deserialize,
)]
#[serde(try_from = "String", into = "String")]
pub struct InstanceName(String);

impl InstanceName {
    /// Parse and validate a name.
    pub fn parse(s: &str) -> Result<Self, ManifestError> {
        Self::parse_as(s, "name")
    }

    /// Parse a name that appears as the value of `key` (named in the error).
    pub(crate) fn parse_as(s: &str, key: &str) -> Result<Self, ManifestError> {
        check_instance_name(s).map_err(|reason| ManifestError::invalid(key, s, reason))?;
        Ok(Self(s.to_string()))
    }

    /// The name text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for InstanceName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for InstanceName {
    type Err = ManifestError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<String> for InstanceName {
    type Error = ManifestError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<InstanceName> for String {
    fn from(n: InstanceName) -> String {
        n.0
    }
}

/// Longest package id, in bytes (ids are ASCII).
const MAX_ID_LEN: usize = 64;

/// Longest instance name, in bytes (names are ASCII).
const MAX_INSTANCE_NAME_LEN: usize = 64;

/// The package id grammar: two or more dot-separated segments, each `[a-z0-9][a-z0-9-]*` without
/// `--` or a trailing `-`, at most 64 characters in total.
fn check_id(s: &str) -> Result<(), &'static str> {
    if s.is_empty() {
        return Err("a package id must not be empty");
    }
    if s.len() > MAX_ID_LEN {
        return Err("a package id must be at most 64 characters");
    }
    if !s.contains('.') {
        return Err("a package id needs at least two dot-separated segments (`namespace.name`)");
    }
    for segment in s.split('.') {
        let bytes = segment.as_bytes();
        let Some(&first) = bytes.first() else {
            return Err(
                "a package id must not contain empty segments (leading, trailing or doubled `.`)",
            );
        };
        if !bytes
            .iter()
            .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(
                "a package id may only contain lowercase ASCII letters, digits, `-` and `.`",
            );
        }
        if first == b'-' {
            return Err("each segment of a package id must start with a lowercase letter or digit");
        }
        if bytes.last() == Some(&b'-') {
            return Err("a segment of a package id must not end with `-`");
        }
        if segment.contains("--") {
            return Err("a package id must not contain `--`");
        }
    }
    Ok(())
}

/// The instance name grammar: `[a-z0-9][a-z0-9-]{0,63}`.
fn check_instance_name(s: &str) -> Result<(), &'static str> {
    let bytes = s.as_bytes();
    let Some(&first) = bytes.first() else {
        return Err("an instance name must not be empty");
    };
    if bytes.len() > MAX_INSTANCE_NAME_LEN {
        return Err("an instance name must be at most 64 characters");
    }
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return Err("an instance name must start with a lowercase letter or digit");
    }
    if !bytes
        .iter()
        .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("an instance name may only contain lowercase ASCII letters, digits and `-`");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_ids() {
        for id in [
            "pwc.hotbar",
            "pwc.material-names",
            "gusahlg.better-caves",
            "a.b",
            "0.1",
            "a1.b2.c3",
            "foo.bar-baz.qux-2",
            "x.y-z-w",
            // exactly 64 characters
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        ] {
            let parsed = PackageId::parse(id).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(parsed.as_str(), id);
            assert_eq!(parsed.to_string(), id);
        }
    }

    #[test]
    fn invalid_ids() {
        let too_long = format!("a.{}", "b".repeat(63));
        assert_eq!(too_long.len(), 65);
        for id in [
            "",
            "pwc",
            "hotbar",
            ".pwc",
            "pwc.",
            "pwc..hotbar",
            "Pwc.hotbar",
            "pwc.Hotbar",
            "pwc.hot_bar",
            "pwc.hot bar",
            "pwc.-hotbar",
            "pwc.hotbar-",
            "pwc-.hotbar",
            "pwc.hot--bar",
            "pwc/hotbar",
            "pwc.hötbar",
            "pwc.hotbar\n",
            too_long.as_str(),
        ] {
            let err = PackageId::parse(id).expect_err(id);
            assert!(
                matches!(err, ManifestError::Invalid { ref key, .. } if key == "id"),
                "{id}: {err}"
            );
        }
    }

    #[test]
    fn id_parts() {
        let id = PackageId::parse("pwc.material-names").unwrap();
        assert_eq!(id.namespace(), "pwc");
        assert!(id.is_first_party());
        assert_eq!(id.crate_name(), "pwc_material_names");
        let id: PackageId = "gusahlg.better-caves.extra".parse().unwrap();
        assert_eq!(id.namespace(), "gusahlg");
        assert!(!id.is_first_party());
        assert_eq!(id.crate_name(), "gusahlg_better_caves_extra");
        // A namespace that merely starts with `pwc` is not first-party.
        assert!(!PackageId::parse("pwcx.a").unwrap().is_first_party());
    }

    #[test]
    fn id_error_names_the_key() {
        let err = PackageId::parse_as("Bad", "package.id").unwrap_err();
        assert!(err.to_string().contains("`package.id`"), "{err}");
    }

    #[test]
    fn id_serde() {
        let id: PackageId = toml::Value::String("pwc.hotbar".into()).try_into().unwrap();
        assert_eq!(id.as_str(), "pwc.hotbar");
        assert!(
            toml::Value::String("nope".into())
                .try_into::<PackageId>()
                .is_err()
        );
        assert_eq!(String::from(id), "pwc.hotbar");
    }

    #[test]
    fn valid_instance_names() {
        let max = "a".repeat(64);
        for name in [
            "survival",
            "a",
            "0",
            "my-pack",
            "dev-",
            "a--b",
            "123",
            max.as_str(),
        ] {
            let parsed = InstanceName::parse(name).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(parsed.as_str(), name);
        }
    }

    #[test]
    fn invalid_instance_names() {
        let long = "a".repeat(65);
        for name in [
            "",
            "-a",
            "Survival",
            "my_pack",
            "my pack",
            "a.b",
            "é",
            "a/b",
            long.as_str(),
        ] {
            let err = InstanceName::parse(name).expect_err(name);
            assert!(
                matches!(err, ManifestError::Invalid { ref key, .. } if key == "name"),
                "{name}: {err}"
            );
        }
    }
}
