//! The licence policy (POLICY.md §1, policy version 1): a package's `license` is an SPDX
//! expression over free licences on the allowlists below. Code must be under a free-software
//! licence compatible with the game's `AGPL-3.0-or-later` (mods are compiled into the game), for
//! example `Apache-2.0 OR MIT` (the recommended choice), `MPL-2.0` or `GPL-3.0-or-later`; assets
//! and documentation may use a free-content licence (POLICY.md says which suit assets compiled into
//! the game). Proprietary licences are not accepted.

use crate::ModKind;

/// Free-software licences accepted for code: each is compatible with the game's
/// `AGPL-3.0-or-later`, so a package under it can be compiled into the game. Keep in sync with
/// POLICY.md.
pub const SOFTWARE_LICENSES: &[&str] = &[
    "0BSD",
    "AGPL-3.0-only",
    "AGPL-3.0-or-later",
    "Apache-2.0",
    "BlueOak-1.0.0",
    "BSD-1-Clause",
    "BSD-2-Clause",
    "BSD-2-Clause-Patent",
    "BSD-3-Clause",
    "BSL-1.0",
    "CC0-1.0",
    "GPL-3.0-only",
    "GPL-3.0-or-later",
    "ISC",
    "LGPL-2.1-or-later",
    "LGPL-3.0-only",
    "LGPL-3.0-or-later",
    "MIT",
    "MIT-0",
    "MPL-2.0",
    "NCSA",
    "PostgreSQL",
    "UPL-1.0",
    "Unicode-3.0",
    "Unlicense",
    "Zlib",
];

/// Free-content licences accepted for assets and documentation. Keep in sync with POLICY.md.
///
/// `LAL-1.3` is the Free Art License 1.3, which the game's MOD_POLICY.md originally wrote as
/// `FAL-1.3`; SPDX lists it under its French name, Licence Art Libre. `CC0-1.0` is on both lists.
pub const CONTENT_LICENSES: &[&str] =
    &["CC-BY-SA-4.0", "CC-BY-4.0", "CC0-1.0", "OFL-1.1", "LAL-1.3"];

/// SPDX exception ids accepted after `WITH`. Keep in sync with POLICY.md.
pub const ALLOWED_EXCEPTIONS: &[&str] = &[
    "LLVM-exception",
    "Classpath-exception-2.0",
    "GCC-exception-3.1",
];

/// The rule every policy rejection message points at.
const POLICY: &str = "packages must use free licences from the allowlists in POLICY.md \
                      (AGPL-compatible free-software licences for code, such as `Apache-2.0 OR MIT`; \
                      free-content licences for assets and documentation); \
                      proprietary licences are not accepted";

/// Why an expression was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LicenseError {
    /// Not a well-formed SPDX expression (ids, `AND`, `OR`, `WITH`, parentheses).
    #[error(
        "`{0}` is not a valid SPDX licence expression (case-sensitive licence ids joined with \
         `AND` and `OR`, optional `WITH <exception>` and parentheses): {POLICY}"
    )]
    Syntax(String),
    /// A licence id on neither allowlist: proprietary, unknown, `LicenseRef-*`, `DocumentRef-*`,
    /// `NOASSERTION`, `NONE`, a trailing `+`, wrong case, or a free licence that is not approved.
    #[error("licence `{0}` is not allowed: {POLICY}")]
    NotAllowed(String),
    /// An exception id after `WITH` that is not on [`ALLOWED_EXCEPTIONS`].
    #[error("licence exception `{0}` is not allowed: {POLICY}")]
    ExceptionNotAllowed(String),
    /// A `mod` or `library` package whose expression has an `OR` alternative without a
    /// free-software licence, which would leave the package's code under content licences only.
    #[error(
        "`{expression}` does not license the package's code: every `OR` alternative of a `mod` or \
         `library` package must include a free-software licence (content licences such as \
         `CC-BY-SA-4.0` do not cover code); {POLICY}"
    )]
    NoSoftwareLicence {
        /// The rejected expression.
        expression: String,
    },
}

/// Check an SPDX expression against the policy's syntax and allowlists, whatever the package
/// kind: every licence id is in [`SOFTWARE_LICENSES`] or [`CONTENT_LICENSES`] and every `WITH`
/// exception in [`ALLOWED_EXCEPTIONS`]. Ids are case-sensitive; a trailing `+` is rejected (use
/// the `-or-later` ids), as are `LicenseRef-*`, `DocumentRef-*`, `NOASSERTION` and `NONE`.
///
/// This does not know what the package contains; [`check_package_license`] adds the rule that
/// code-carrying packages license their code under a free-software licence.
///
/// The SPDX grammar (precedence `WITH` > `AND` > `OR`; ASCII whitespace between tokens) is
/// parsed in full first, so a malformed expression is always a [`LicenseError::Syntax`]:
///
/// ```text
/// expression := and-expr ("OR" and-expr)*
/// and-expr   := primary ("AND" primary)*
/// primary    := "(" expression ")" | license-id ["WITH" exception-id]
/// ```
///
/// Then every id is checked left to right ([`LicenseError::NotAllowed`] for a licence,
/// [`LicenseError::ExceptionNotAllowed`] for an exception).
pub fn check_license_expression(expr: &str) -> Result<(), LicenseError> {
    check(expr).map(|_| ())
}

/// The kind-specific licence rule (POLICY.md §1): the expression passes
/// [`check_license_expression`], and for `mod` and `library` packages every `OR` alternative
/// includes a free-software licence from [`SOFTWARE_LICENSES`], so whichever alternative a
/// licensee picks, the code is under a free-software licence. Evaluated recursively: an id counts
/// if it is on the software list, `X WITH e` if `X` does, `A AND B` if either side does, `A OR B`
/// if both sides do. So `Apache-2.0 OR MIT`, `MPL-2.0` and `AGPL-3.0-or-later AND CC-BY-SA-4.0`
/// pass, while `CC-BY-SA-4.0` or `MIT OR CC-BY-4.0` fail with
/// [`LicenseError::NoSoftwareLicence`].
///
/// A `bundle` contains no code, so any expression passing the allowlist check is fine.
pub fn check_package_license(expr: &str, kind: ModKind) -> Result<(), LicenseError> {
    let licenses_code = check(expr)?;
    if kind.has_code() && !licenses_code {
        return Err(LicenseError::NoSoftwareLicence {
            expression: expr.to_string(),
        });
    }
    Ok(())
}

/// Parse `expr` and check every id against the allowlists. Returns whether every `OR`
/// alternative includes a software licence (the code-package rule of [`check_package_license`]).
fn check(expr: &str) -> Result<bool, LicenseError> {
    let syntax = || LicenseError::Syntax(expr.to_string());
    let tokens = tokenize(expr).ok_or_else(syntax)?;
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
        depth: 0,
        terms: Vec::new(),
    };
    let licenses_code = parser.expression().ok_or_else(syntax)?;
    if parser.pos != tokens.len() {
        return Err(syntax());
    }
    for &(license, exception) in &parser.terms {
        if !(SOFTWARE_LICENSES.contains(&license) || CONTENT_LICENSES.contains(&license)) {
            return Err(LicenseError::NotAllowed(license.to_string()));
        }
        if let Some(exception) = exception
            && !ALLOWED_EXCEPTIONS.contains(&exception)
        {
            return Err(LicenseError::ExceptionNotAllowed(exception.to_string()));
        }
    }
    Ok(licenses_code)
}

/// Deepest parenthesis nesting accepted (keeps the recursive parser's stack bounded).
const MAX_DEPTH: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Token<'a> {
    Open,
    Close,
    And,
    Or,
    With,
    /// A licence or exception id (`[A-Za-z0-9.+:-]+`; whether it is acceptable is decided later).
    Id(&'a str),
}

/// Split into tokens; `None` on a character no SPDX expression can contain.
fn tokenize(expr: &str) -> Option<Vec<Token<'_>>> {
    let mut tokens = Vec::new();
    let mut rest = expr;
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace());
        let Some(c) = rest.chars().next() else { break };
        match c {
            '(' => {
                tokens.push(Token::Open);
                rest = &rest[1..];
            }
            ')' => {
                tokens.push(Token::Close);
                rest = &rest[1..];
            }
            _ => {
                let end = rest
                    .find(|c: char| c.is_ascii_whitespace() || c == '(' || c == ')')
                    .unwrap_or(rest.len());
                let word = &rest[..end];
                if !word
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+' | b':'))
                {
                    return None;
                }
                tokens.push(match word {
                    "AND" => Token::And,
                    "OR" => Token::Or,
                    "WITH" => Token::With,
                    _ => Token::Id(word),
                });
                rest = &rest[end..];
            }
        }
    }
    Some(tokens)
}

/// Recursive-descent parser collecting `(licence, exception)` terms. Every method returns `None`
/// on a syntax error, otherwise whether the parsed subexpression licenses code (see
/// [`check_package_license`]). Every operand is parsed even when that result is already known,
/// so no syntax error is skipped.
struct Parser<'t, 'a> {
    tokens: &'t [Token<'a>],
    pos: usize,
    depth: usize,
    terms: Vec<(&'a str, Option<&'a str>)>,
}

impl<'a> Parser<'_, 'a> {
    fn peek(&self) -> Option<Token<'a>> {
        self.tokens.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<Token<'a>> {
        let token = self.peek()?;
        self.pos += 1;
        Some(token)
    }

    /// `A OR B` licenses code if both alternatives do.
    fn expression(&mut self) -> Option<bool> {
        let mut licenses_code = self.and_expression()?;
        while self.peek() == Some(Token::Or) {
            self.pos += 1;
            let alternative = self.and_expression()?;
            licenses_code &= alternative;
        }
        Some(licenses_code)
    }

    /// `A AND B` licenses code if either operand does.
    fn and_expression(&mut self) -> Option<bool> {
        let mut licenses_code = self.primary()?;
        while self.peek() == Some(Token::And) {
            self.pos += 1;
            let operand = self.primary()?;
            licenses_code |= operand;
        }
        Some(licenses_code)
    }

    /// An id licenses code if it is a software licence; `X WITH e` if `X` does.
    fn primary(&mut self) -> Option<bool> {
        match self.next()? {
            Token::Open => {
                self.depth += 1;
                if self.depth > MAX_DEPTH {
                    return None;
                }
                let licenses_code = self.expression()?;
                if self.next()? != Token::Close {
                    return None;
                }
                self.depth -= 1;
                Some(licenses_code)
            }
            Token::Id(license) => {
                let exception = if self.peek() == Some(Token::With) {
                    self.pos += 1;
                    match self.next()? {
                        Token::Id(exception) => Some(exception),
                        _ => return None,
                    }
                } else {
                    None
                };
                self.terms.push((license, exception));
                Some(SOFTWARE_LICENSES.contains(&license))
            }
            Token::Close | Token::And | Token::Or | Token::With => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODE_KINDS: [ModKind; 2] = [ModKind::Mod, ModKind::Library];
    const ALL_KINDS: [ModKind; 3] = [ModKind::Mod, ModKind::Library, ModKind::Bundle];

    fn no_software(expr: &str) -> LicenseError {
        LicenseError::NoSoftwareLicence {
            expression: expr.to_string(),
        }
    }

    #[test]
    fn lists_match_policy() {
        // Every allowed id is named (in backticks) in POLICY.md.
        let policy = include_str!("../../../POLICY.md");
        for id in SOFTWARE_LICENSES
            .iter()
            .chain(CONTENT_LICENSES)
            .chain(ALLOWED_EXCEPTIONS)
        {
            assert!(
                policy.contains(&format!("`{id}`")),
                "POLICY.md does not mention `{id}`"
            );
        }
    }

    #[test]
    fn lists_are_exactly_policy_version_1() {
        let sorted = |list: &[&'static str]| {
            let mut list = list.to_vec();
            list.sort_unstable();
            list
        };
        assert_eq!(
            sorted(SOFTWARE_LICENSES),
            sorted(&[
                "0BSD",
                "AGPL-3.0-only",
                "AGPL-3.0-or-later",
                "Apache-2.0",
                "BlueOak-1.0.0",
                "BSD-1-Clause",
                "BSD-2-Clause",
                "BSD-2-Clause-Patent",
                "BSD-3-Clause",
                "BSL-1.0",
                "CC0-1.0",
                "GPL-3.0-only",
                "GPL-3.0-or-later",
                "ISC",
                "LGPL-2.1-or-later",
                "LGPL-3.0-only",
                "LGPL-3.0-or-later",
                "MIT",
                "MIT-0",
                "MPL-2.0",
                "NCSA",
                "PostgreSQL",
                "UPL-1.0",
                "Unicode-3.0",
                "Unlicense",
                "Zlib",
            ])
        );
        assert_eq!(
            sorted(CONTENT_LICENSES),
            ["CC-BY-4.0", "CC-BY-SA-4.0", "CC0-1.0", "LAL-1.3", "OFL-1.1"]
        );
        assert_eq!(
            sorted(ALLOWED_EXCEPTIONS),
            [
                "Classpath-exception-2.0",
                "GCC-exception-3.1",
                "LLVM-exception"
            ]
        );
        for list in [SOFTWARE_LICENSES, CONTENT_LICENSES, ALLOWED_EXCEPTIONS] {
            let mut deduped = sorted(list);
            deduped.dedup();
            assert_eq!(deduped.len(), list.len(), "duplicate id in {list:?}");
        }
    }

    #[test]
    fn every_allowed_id_is_accepted_alone() {
        for id in SOFTWARE_LICENSES.iter().chain(CONTENT_LICENSES) {
            check_license_expression(id).unwrap_or_else(|e| panic!("{id}: {e}"));
            check_package_license(id, ModKind::Bundle).unwrap_or_else(|e| panic!("{id}: {e}"));
        }
        for id in SOFTWARE_LICENSES {
            for kind in CODE_KINDS {
                check_package_license(id, kind).unwrap_or_else(|e| panic!("{kind:?} {id}: {e}"));
            }
        }
        // Content-only ids do not license code (CC0-1.0 is on both lists).
        for id in CONTENT_LICENSES
            .iter()
            .filter(|id| !SOFTWARE_LICENSES.contains(*id))
        {
            for kind in CODE_KINDS {
                assert_eq!(check_package_license(id, kind), Err(no_software(id)));
            }
        }
        for exception in ALLOWED_EXCEPTIONS {
            let expr = format!("Apache-2.0 WITH {exception}");
            for kind in ALL_KINDS {
                check_package_license(&expr, kind).unwrap_or_else(|e| panic!("{expr}: {e}"));
            }
        }
    }

    #[test]
    fn accepted_expressions() {
        for expr in [
            "Apache-2.0 OR MIT",
            "MIT OR Apache-2.0",
            "MIT",
            "MPL-2.0",
            "AGPL-3.0-or-later",
            "AGPL-3.0-or-later AND CC-BY-SA-4.0",
            "(AGPL-3.0-or-later AND CC-BY-SA-4.0)",
            "Apache-2.0 WITH LLVM-exception",
            "GPL-3.0-or-later WITH GCC-exception-3.1",
            "GPL-3.0-only WITH Classpath-exception-2.0",
            "(Apache-2.0 OR MIT) AND CC-BY-4.0",
            "(Apache-2.0 WITH LLVM-exception OR MIT) AND (CC-BY-SA-4.0 OR CC0-1.0)",
            "Apache-2.0 OR MIT AND CC-BY-4.0",
            "MIT AND CC-BY-4.0 AND CC0-1.0 AND OFL-1.1 AND LAL-1.3",
            "CC-BY-SA-4.0 AND AGPL-3.0-or-later",
            "((MIT))",
            "  MIT  ",
            "Apache-2.0\tOR\nMIT",
            "Apache-2.0\r\nOR MIT",
            "(Apache-2.0)OR(MIT)",
            "CC0-1.0",
            "CC-BY-SA-4.0",
            "CC-BY-4.0 OR OFL-1.1",
        ] {
            check_license_expression(expr).unwrap_or_else(|e| panic!("{expr:?}: {e}"));
        }
    }

    #[test]
    fn code_packages_need_software_in_every_alternative() {
        for kind in CODE_KINDS {
            for ok in [
                "Apache-2.0 OR MIT",
                "MIT",
                "MPL-2.0",
                "GPL-3.0-or-later",
                "AGPL-3.0-or-later",
                "AGPL-3.0-or-later AND CC-BY-SA-4.0",
                "CC-BY-4.0 AND AGPL-3.0-or-later AND OFL-1.1",
                "Apache-2.0 WITH LLVM-exception",
                "(Apache-2.0 OR MIT) AND CC-BY-4.0",
                "CC-BY-4.0 AND (Apache-2.0 OR MIT)",
                // AND binds tighter than OR: (MIT AND CC-BY-4.0) OR Apache-2.0.
                "MIT AND CC-BY-4.0 OR Apache-2.0",
                "(MIT AND CC-BY-4.0) OR (Apache-2.0 AND CC-BY-SA-4.0)",
                "(MIT OR CC-BY-4.0) AND Zlib",
                "(MIT OR CC-BY-4.0) AND (Zlib OR OFL-1.1) AND ISC",
                // CC0-1.0 is a software licence too.
                "CC0-1.0",
                "CC0-1.0 OR MIT",
                "CC0-1.0 WITH LLVM-exception",
            ] {
                check_package_license(ok, kind).unwrap_or_else(|e| panic!("{kind:?} {ok}: {e}"));
            }
            for bad in [
                "CC-BY-SA-4.0",
                "CC-BY-SA-4.0 AND OFL-1.1",
                "MIT OR CC-BY-4.0",
                "CC-BY-4.0 OR MIT",
                "Apache-2.0 OR MIT OR LAL-1.3",
                // AND binds tighter than OR: MIT OR (CC-BY-4.0 AND OFL-1.1).
                "MIT OR CC-BY-4.0 AND OFL-1.1",
                "(MIT OR CC-BY-4.0) AND (OFL-1.1 OR CC-BY-SA-4.0)",
                "((MIT AND CC-BY-4.0) OR (OFL-1.1 AND LAL-1.3))",
                // WITH on a content licence does not make it a software licence.
                "CC-BY-4.0 WITH LLVM-exception",
                "CC-BY-SA-4.0 WITH GCC-exception-3.1 OR MIT",
            ] {
                assert_eq!(
                    check_package_license(bad, kind),
                    Err(no_software(bad)),
                    "{kind:?} {bad}"
                );
                // A bundle takes any expression passing the allowlist check.
                check_package_license(bad, ModKind::Bundle)
                    .unwrap_or_else(|e| panic!("bundle {bad}: {e}"));
                check_license_expression(bad).unwrap_or_else(|e| panic!("{bad}: {e}"));
            }
            // The syntax and allowlist checks come first.
            for (expr, expected) in [
                (
                    "LicenseRef-Proprietary",
                    LicenseError::NotAllowed("LicenseRef-Proprietary".into()),
                ),
                (
                    "CC-BY-4.0 OR LicenseRef-Mine",
                    LicenseError::NotAllowed("LicenseRef-Mine".into()),
                ),
                (
                    "CC-BY-4.0 WITH Foo-exception",
                    LicenseError::ExceptionNotAllowed("Foo-exception".into()),
                ),
                ("CC-BY-4.0 OR", LicenseError::Syntax("CC-BY-4.0 OR".into())),
            ] {
                assert_eq!(check_package_license(expr, kind), Err(expected));
            }
        }
        for (expr, expected) in [
            (
                "LicenseRef-Proprietary",
                LicenseError::NotAllowed("LicenseRef-Proprietary".into()),
            ),
            (
                "MIT WITH Foo-exception",
                LicenseError::ExceptionNotAllowed("Foo-exception".into()),
            ),
            ("MIT AND", LicenseError::Syntax("MIT AND".into())),
        ] {
            assert_eq!(check_package_license(expr, ModKind::Bundle), Err(expected));
        }
    }

    #[test]
    fn syntax_errors() {
        for expr in [
            "",
            "   ",
            "AND",
            "OR",
            "WITH",
            "MIT AND",
            "OR MIT",
            "MIT OR",
            "MIT AND AND CC0-1.0",
            "MIT OR OR Apache-2.0",
            "MIT AND OR Apache-2.0",
            "MIT CC0-1.0",
            "MIT and CC0-1.0",
            "MIT or Apache-2.0",
            "MIT with LLVM-exception",
            "(MIT",
            "MIT)",
            "()",
            ")(",
            "(MIT OR) Apache-2.0",
            "MIT WITH",
            "WITH LLVM-exception",
            "MIT WITH (LLVM-exception)",
            "(Apache-2.0) WITH LLVM-exception",
            "(Apache-2.0 OR MIT) WITH LLVM-exception",
            "Apache-2.0 WITH LLVM-exception WITH LLVM-exception",
            "Apache-2.0 WITH AND",
            "Apache-2.0 WITH LLVM-exception MIT",
            "MIT/Apache-2.0",
            "MIT, Apache-2.0",
            "MIT & Apache-2.0",
            "MIT | Apache-2.0",
            "\"MIT\"",
            "MIT;",
            "MIT\u{a0}OR Apache-2.0",
            "MIT OR\u{2003}Apache-2.0",
            "Apache-2.0 OR MÏT",
            "MIT\0",
            // Syntax is reported before policy.
            "LicenseRef-Proprietary AND",
            "Proprietary OR OR MIT",
            "MIT WITH Foo AND",
        ] {
            for kind in ALL_KINDS {
                assert_eq!(
                    check_package_license(expr, kind),
                    Err(LicenseError::Syntax(expr.to_string())),
                    "{kind:?} {expr:?}"
                );
            }
            assert_eq!(
                check_license_expression(expr),
                Err(LicenseError::Syntax(expr.to_string())),
                "{expr:?}"
            );
        }
    }

    #[test]
    fn deep_nesting_is_a_syntax_error_not_a_stack_overflow() {
        let ok = format!("{}MIT{}", "(".repeat(MAX_DEPTH), ")".repeat(MAX_DEPTH));
        check_package_license(&ok, ModKind::Mod).unwrap();
        let too_deep = format!("({ok})");
        assert_eq!(
            check_license_expression(&too_deep),
            Err(LicenseError::Syntax(too_deep.clone()))
        );
        let deep = format!("{}MIT{}", "(".repeat(100_000), ")".repeat(100_000));
        assert!(matches!(
            check_license_expression(&deep),
            Err(LicenseError::Syntax(_))
        ));
        let unbalanced = "(".repeat(100_000);
        assert!(matches!(
            check_license_expression(&unbalanced),
            Err(LicenseError::Syntax(_))
        ));
        // Depth is nesting, not the number of groups: many sibling groups are fine.
        let siblings = vec!["(MIT OR Apache-2.0)"; 1_000].join(" AND ");
        check_package_license(&siblings, ModKind::Mod).unwrap();
        // Long flat chains do not recurse.
        let chain = vec!["CC-BY-4.0"; 100_000].join(" OR ") + " OR MIT";
        assert_eq!(
            check_package_license(&chain, ModKind::Mod),
            Err(no_software(&chain))
        );
        check_package_license(&chain, ModKind::Bundle).unwrap();
    }

    #[test]
    fn proprietary_unknown_and_unapproved_licences_are_rejected() {
        for (expr, bad) in [
            ("LicenseRef-Proprietary", "LicenseRef-Proprietary"),
            ("MIT AND LicenseRef-Mine", "LicenseRef-Mine"),
            (
                "Apache-2.0 OR LicenseRef-Commercial",
                "LicenseRef-Commercial",
            ),
            ("DocumentRef-x:LicenseRef-y", "DocumentRef-x:LicenseRef-y"),
            ("NOASSERTION", "NOASSERTION"),
            ("NONE", "NONE"),
            ("MIT OR NONE", "NONE"),
            ("Proprietary", "Proprietary"),
            ("Commercial", "Commercial"),
            ("SSPL-1.0", "SSPL-1.0"),
            ("BUSL-1.1", "BUSL-1.1"),
            ("Elastic-2.0", "Elastic-2.0"),
            (
                "PolyForm-Noncommercial-1.0.0",
                "PolyForm-Noncommercial-1.0.0",
            ),
            ("(CC0-1.0 AND (MIT AND SSPL-1.0))", "SSPL-1.0"),
            // A trailing `+` is rejected: use the `-or-later` ids.
            ("GPL-3.0+", "GPL-3.0+"),
            ("Apache-2.0+", "Apache-2.0+"),
            ("MIT OR MPL-2.0+", "MPL-2.0+"),
            ("AGPL-3.0-or-later+", "AGPL-3.0-or-later+"),
            ("+", "+"),
            // Deprecated SPDX ids and wrong case.
            ("GPL-3.0", "GPL-3.0"),
            ("AGPL-3.0", "AGPL-3.0"),
            (
                "GPL-2.0-with-classpath-exception",
                "GPL-2.0-with-classpath-exception",
            ),
            ("mit", "mit"),
            ("apache-2.0 OR MIT", "apache-2.0"),
            ("cc0-1.0", "cc0-1.0"),
            // Free licences that are not AGPL-compatible or not on the list.
            ("GPL-2.0-only", "GPL-2.0-only"),
            ("GPL-2.0-or-later", "GPL-2.0-or-later"),
            ("LGPL-2.1-only", "LGPL-2.1-only"),
            ("EPL-2.0", "EPL-2.0"),
            ("EUPL-1.2", "EUPL-1.2"),
            ("CDDL-1.0", "CDDL-1.0"),
            ("MPL-1.1", "MPL-1.1"),
            ("BSD-4-Clause", "BSD-4-Clause"),
            ("WTFPL", "WTFPL"),
            ("JSON", "JSON"),
            // Non-free or outdated content licences.
            ("CC-BY-NC-4.0", "CC-BY-NC-4.0"),
            ("CC-BY-ND-4.0", "CC-BY-ND-4.0"),
            ("MIT AND CC-BY-NC-SA-4.0", "CC-BY-NC-SA-4.0"),
            ("CC-BY-SA-3.0", "CC-BY-SA-3.0"),
            // MOD_POLICY.md's original name for the Free Art License is not an SPDX id; `LAL-1.3` is.
            ("FAL-1.3", "FAL-1.3"),
            ("LAL-1.2", "LAL-1.2"),
            // An exception is not a licence.
            ("LLVM-exception", "LLVM-exception"),
            // Reported left to right, a licence before its exception.
            ("Foo WITH Bar", "Foo"),
            ("MIT OR Foo OR Bar", "Foo"),
        ] {
            for kind in ALL_KINDS {
                assert_eq!(
                    check_package_license(expr, kind),
                    Err(LicenseError::NotAllowed(bad.to_string())),
                    "{kind:?} {expr:?}"
                );
            }
            assert_eq!(
                check_license_expression(expr),
                Err(LicenseError::NotAllowed(bad.to_string())),
                "{expr:?}"
            );
        }
    }

    #[test]
    fn only_listed_exceptions_are_accepted() {
        for (expr, bad) in [
            ("Apache-2.0 WITH Foo", "Foo"),
            (
                "GPL-3.0-or-later WITH Autoconf-exception-3.0",
                "Autoconf-exception-3.0",
            ),
            (
                "GPL-3.0-or-later WITH Classpath-exception-2.0+",
                "Classpath-exception-2.0+",
            ),
            ("Apache-2.0 WITH llvm-exception", "llvm-exception"),
            ("MIT WITH MIT", "MIT"),
            ("MIT WITH LicenseRef-Mine", "LicenseRef-Mine"),
            (
                "MIT OR Apache-2.0 WITH LLVM-exception AND CC0-1.0 WITH X",
                "X",
            ),
        ] {
            for kind in ALL_KINDS {
                assert_eq!(
                    check_package_license(expr, kind),
                    Err(LicenseError::ExceptionNotAllowed(bad.to_string())),
                    "{kind:?} {expr:?}"
                );
            }
        }
        // The licence id is checked before its exception.
        assert_eq!(
            check_license_expression("Proprietary WITH Foo"),
            Err(LicenseError::NotAllowed("Proprietary".into()))
        );
        // An allowed exception on a content licence passes the allowlist check, but does not make
        // the term a software licence.
        check_license_expression("CC-BY-4.0 WITH LLVM-exception").unwrap();
        assert_eq!(
            check_package_license("CC-BY-4.0 WITH LLVM-exception", ModKind::Mod),
            Err(no_software("CC-BY-4.0 WITH LLVM-exception"))
        );
    }

    #[test]
    fn messages_point_at_the_policy() {
        for err in [
            check_license_expression("SSPL-1.0").unwrap_err(),
            check_license_expression("LicenseRef-Proprietary").unwrap_err(),
            check_license_expression("MIT WITH Foo").unwrap_err(),
            check_package_license("CC-BY-SA-4.0", ModKind::Mod).unwrap_err(),
            check_package_license("MIT OR CC-BY-4.0", ModKind::Library).unwrap_err(),
        ] {
            let msg = err.to_string();
            assert!(msg.contains("POLICY.md"), "{msg}");
            assert!(
                msg.contains("proprietary licences are not accepted"),
                "{msg}"
            );
            assert!(!msg.contains("must be AGPL"), "{msg}");
        }
        let syntax = check_license_expression("MIT AND").unwrap_err().to_string();
        assert!(syntax.contains("POLICY.md"), "{syntax}");
        assert!(
            syntax.contains("proprietary licences are not accepted"),
            "{syntax}"
        );
        assert!(syntax.contains("`MIT AND`"), "{syntax}");
        for (err, needle) in [
            (
                check_license_expression("LicenseRef-Proprietary").unwrap_err(),
                "`LicenseRef-Proprietary`",
            ),
            (
                check_license_expression("MIT WITH Foo").unwrap_err(),
                "`Foo`",
            ),
            (
                check_package_license("MIT OR CC-BY-4.0", ModKind::Mod).unwrap_err(),
                "`MIT OR CC-BY-4.0`",
            ),
        ] {
            let msg = err.to_string();
            assert!(msg.contains(needle), "{msg}");
        }
    }

    /// No input panics, and the kind rules are consistent with the allowlist check: a bundle
    /// accepts exactly what [`check_license_expression`] accepts, a code package a subset of it,
    /// rejecting the rest only with [`LicenseError::NoSoftwareLicence`].
    #[test]
    fn exhaustive_short_token_sequences() {
        const TOKENS: [&str; 10] = [
            "(",
            ")",
            "AND",
            "OR",
            "WITH",
            "MIT",
            "CC-BY-4.0",
            "LLVM-exception",
            "Foo",
            "+",
        ];
        let mut words = Vec::new();
        let (mut accepted, mut code_accepted) = (0, 0);
        for len in 0..=5u32 {
            for n in 0..TOKENS.len().pow(len) {
                words.clear();
                let mut n = n;
                for _ in 0..len {
                    words.push(TOKENS[n % TOKENS.len()]);
                    n /= TOKENS.len();
                }
                let expr = words.join(" ");
                let any = check_license_expression(&expr);
                assert_eq!(
                    check_package_license(&expr, ModKind::Bundle),
                    any,
                    "{expr:?}"
                );
                let code = check_package_license(&expr, ModKind::Mod);
                assert_eq!(check_package_license(&expr, ModKind::Library), code);
                match (&any, &code) {
                    (Ok(()), Ok(())) => {
                        accepted += 1;
                        code_accepted += 1;
                    }
                    (Ok(()), Err(e)) => {
                        accepted += 1;
                        assert_eq!(*e, no_software(&expr), "{expr:?}");
                    }
                    (Err(_), _) => assert_eq!(code, any, "{expr:?}"),
                }
            }
        }
        assert!(accepted > code_accepted && code_accepted > 0);
    }

    /// Random expression trees over allowed ids, rendered with only the parentheses precedence
    /// needs (plus some redundant ones), against a direct evaluation of the code-package rule.
    #[test]
    fn random_trees_match_the_reference_rule() {
        enum Tree {
            Id(&'static str, Option<&'static str>),
            And(Box<Tree>, Box<Tree>),
            Or(Box<Tree>, Box<Tree>),
        }

        fn software(tree: &Tree) -> bool {
            match tree {
                Tree::Id(id, _) => SOFTWARE_LICENSES.contains(id),
                Tree::And(a, b) => software(a) || software(b),
                Tree::Or(a, b) => software(a) && software(b),
            }
        }

        /// SplitMix64.
        struct Rng(u64);
        impl Rng {
            fn below(&mut self, n: usize) -> usize {
                self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
                let mut z = self.0;
                z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
                ((z ^ (z >> 31)) % n as u64) as usize
            }
        }

        fn generate(rng: &mut Rng, depth: u32) -> Tree {
            if depth == 0 || rng.below(3) == 0 {
                // Content licences are over-weighted so that both outcomes are common.
                let id = if rng.below(2) == 0 {
                    SOFTWARE_LICENSES[rng.below(SOFTWARE_LICENSES.len())]
                } else {
                    CONTENT_LICENSES[rng.below(CONTENT_LICENSES.len())]
                };
                let exception = (rng.below(5) == 0)
                    .then(|| ALLOWED_EXCEPTIONS[rng.below(ALLOWED_EXCEPTIONS.len())]);
                return Tree::Id(id, exception);
            }
            let a = Box::new(generate(rng, depth - 1));
            let b = Box::new(generate(rng, depth - 1));
            if rng.below(2) == 0 {
                Tree::And(a, b)
            } else {
                Tree::Or(a, b)
            }
        }

        /// `prec` is 0 at the top and in an `OR` operand, 1 in an `AND` operand.
        fn render(rng: &mut Rng, tree: &Tree, prec: u8, out: &mut String) {
            let (op, own, a, b) = match tree {
                Tree::Id(id, exception) => {
                    let redundant = rng.below(8) == 0;
                    if redundant {
                        out.push('(');
                    }
                    out.push_str(id);
                    if let Some(e) = exception {
                        out.push_str(" WITH ");
                        out.push_str(e);
                    }
                    if redundant {
                        out.push(')');
                    }
                    return;
                }
                Tree::And(a, b) => ("AND", 1, a, b),
                Tree::Or(a, b) => ("OR", 0, a, b),
            };
            let parens = own < prec || rng.below(6) == 0;
            if parens {
                out.push('(');
            }
            render(rng, a, own, out);
            out.push(' ');
            out.push_str(op);
            out.push(' ');
            render(rng, b, own, out);
            if parens {
                out.push(')');
            }
        }

        let mut rng = Rng(0x5eed);
        let (mut passed, mut failed) = (0, 0);
        for _ in 0..5_000 {
            let tree = generate(&mut rng, 5);
            let mut expr = String::new();
            render(&mut rng, &tree, 0, &mut expr);
            check_license_expression(&expr).unwrap_or_else(|e| panic!("{expr}: {e}"));
            check_package_license(&expr, ModKind::Bundle)
                .unwrap_or_else(|e| panic!("bundle {expr}: {e}"));
            let result = check_package_license(&expr, ModKind::Mod);
            if software(&tree) {
                result.unwrap_or_else(|e| panic!("{expr}: {e}"));
                passed += 1;
            } else {
                assert_eq!(result, Err(no_software(&expr)));
                failed += 1;
            }
        }
        assert!(
            passed > 500 && failed > 500,
            "{passed} passed, {failed} failed"
        );
    }
}
