//! Resolver tests: the rules, the preference order, error messages, the registration order, and a
//! randomised comparison against a plain chronological backtracker and a brute-force search.

use std::collections::{BTreeMap, BTreeSet};

use super::*;

fn id(s: &str) -> PackageId {
    PackageId::parse(s).unwrap()
}

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

fn req(s: &str) -> VersionReq {
    VersionReq::parse(s).unwrap()
}

/// Builds a problem candidate by candidate; `source` tokens are assigned in insertion order.
#[derive(Default)]
struct P {
    problem: Problem,
}

impl P {
    fn new() -> Self {
        let mut p = Self::default();
        p.problem.api = Some(v("1.0.0"));
        p
    }

    /// Add `id version` with `deps` (`"id req"` pairs); returns its source token.
    fn add(&mut self, pkg: &str, version: &str, deps: &[(&str, &str)]) -> usize {
        self.add_full(pkg, version, deps, &[], Some("^1"))
    }

    fn add_full(
        &mut self,
        pkg: &str,
        version: &str,
        deps: &[(&str, &str)],
        conflicts: &[(&str, &str)],
        api: Option<&str>,
    ) -> usize {
        let token = self.problem.candidates.len();
        let mut digest = [0u8; 32];
        digest[..8].copy_from_slice(&(token as u64).to_le_bytes());
        self.problem.candidates.push(Candidate {
            id: id(pkg),
            version: v(version),
            kind: if api.is_some() {
                ModKind::Mod
            } else {
                ModKind::Bundle
            },
            hash: PackageHash::from_digest(digest),
            pwc_api: api.map(req),
            dependencies: deps.iter().map(|(d, r)| (id(d), req(r))).collect(),
            conflicts: conflicts.iter().map(|(d, r)| (id(d), req(r))).collect(),
            source: token,
        });
        token
    }

    fn root(&mut self, pkg: &str, r: &str) -> &mut Self {
        self.problem
            .roots
            .insert(id(pkg), Requirement::Version(req(r)));
        self
    }

    fn pin(&mut self, pkg: &str, token: usize) -> &mut Self {
        self.problem.roots.insert(id(pkg), Requirement::Pin(token));
        self
    }

    fn previous(&mut self, pkg: &str, version: &str) -> &mut Self {
        self.problem.previous.insert(id(pkg), v(version));
        self
    }

    fn resolve(&self) -> Result<Resolution, ResolveError> {
        resolve(&self.problem)
    }
}

/// `id version` of every selected package, sorted by id.
fn picked(r: &Resolution) -> Vec<String> {
    r.packages
        .iter()
        .map(|s| format!("{} {}", s.candidate.id, s.candidate.version))
        .collect()
}

fn order(r: &Resolution) -> Vec<String> {
    r.order.iter().map(ToString::to_string).collect()
}

fn err(p: &P) -> String {
    p.resolve().unwrap_err().to_string()
}

#[test]
fn empty_problem_resolves_to_nothing() {
    let r = P::new().resolve().unwrap();
    assert!(r.packages.is_empty());
    assert!(r.order.is_empty());
}

#[test]
fn picks_the_highest_matching_version() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[]);
    p.add("t.a", "1.2.0", &[]);
    p.add("t.a", "1.10.0", &[]);
    p.add("t.a", "2.0.0", &[]);
    p.root("t.a", "^1");
    assert_eq!(picked(&p.resolve().unwrap()), ["t.a 1.10.0"]);
}

#[test]
fn unneeded_candidates_are_not_selected() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[]);
    p.add("t.unused", "1.0.0", &[]);
    p.root("t.a", "*");
    assert_eq!(picked(&p.resolve().unwrap()), ["t.a 1.0.0"]);
}

#[test]
fn diamond_dependencies_share_one_version() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[("t.b", "^1"), ("t.c", "^1")]);
    p.add("t.b", "1.0.0", &[("t.d", "^1")]);
    p.add("t.c", "1.0.0", &[("t.d", ">=1.1")]);
    p.add("t.d", "1.0.0", &[]);
    p.add("t.d", "1.1.0", &[]);
    p.add("t.d", "1.2.0", &[]);
    p.add("t.d", "2.0.0", &[]);
    p.root("t.a", "*");
    let r = p.resolve().unwrap();
    assert_eq!(
        picked(&r),
        ["t.a 1.0.0", "t.b 1.0.0", "t.c 1.0.0", "t.d 1.2.0"]
    );
    assert_eq!(order(&r), ["t.d", "t.b", "t.c", "t.a"]);
    let a = r.get(&id("t.a")).unwrap();
    assert_eq!(a.dependencies, [id("t.b"), id("t.c")]);
    assert!(r.get(&id("t.zzz")).is_none());
}

#[test]
fn diamond_narrows_to_the_intersection() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[("t.d", ">=1.1")]);
    p.add("t.b", "1.0.0", &[("t.d", "<1.2")]);
    for d in ["1.0.0", "1.1.0", "1.1.5", "1.2.0"] {
        p.add("t.d", d, &[]);
    }
    p.root("t.a", "*").root("t.b", "*");
    assert_eq!(
        picked(&p.resolve().unwrap()),
        ["t.a 1.0.0", "t.b 1.0.0", "t.d 1.1.5"]
    );
}

#[test]
fn backtracks_to_an_older_version_when_the_newest_needs_the_impossible() {
    let mut p = P::new();
    p.add("t.a", "2.0.0", &[("t.b", "^2")]);
    p.add("t.a", "1.0.0", &[("t.b", "^1")]);
    p.add("t.b", "1.0.0", &[]);
    p.add("t.b", "1.5.0", &[]);
    p.root("t.a", "*");
    assert_eq!(picked(&p.resolve().unwrap()), ["t.a 1.0.0", "t.b 1.5.0"]);
}

#[test]
fn backtracks_through_a_deeper_dead_end() {
    let mut p = P::new();
    p.add("t.a", "2.0.0", &[("t.b", "^1")]);
    p.add("t.b", "1.1.0", &[("t.c", "^2")]);
    p.add("t.b", "1.0.0", &[("t.c", "^1")]);
    p.add("t.c", "1.0.0", &[]);
    p.root("t.a", "*");
    assert_eq!(
        picked(&p.resolve().unwrap()),
        ["t.a 2.0.0", "t.b 1.0.0", "t.c 1.0.0"]
    );
}

#[test]
fn backtracks_across_roots() {
    // t.a 2 needs t.c ^2, t.b needs t.c ^1: only t.a 1 works with t.b.
    let mut p = P::new();
    p.add("t.a", "2.0.0", &[("t.c", "^2")]);
    p.add("t.a", "1.0.0", &[("t.c", "^1")]);
    p.add("t.b", "1.0.0", &[("t.c", "^1")]);
    p.add("t.c", "1.0.0", &[]);
    p.add("t.c", "2.0.0", &[]);
    p.root("t.a", "*").root("t.b", "*");
    assert_eq!(
        picked(&p.resolve().unwrap()),
        ["t.a 1.0.0", "t.b 1.0.0", "t.c 1.0.0"]
    );
}

#[test]
fn a_dependency_already_chosen_must_match() {
    // t.b is decided (2.0) before t.a's dependency on it is known; t.a 2 needs t.b ^1.
    let mut p = P::new();
    p.add("t.a", "2.0.0", &[("t.b", "^1")]);
    p.add("t.a", "1.0.0", &[]);
    p.add("t.b", "1.0.0", &[]);
    p.add("t.b", "2.0.0", &[]);
    p.root("t.b", "*").root("t.c", "*");
    p.add("t.c", "1.0.0", &[("t.a", "*")]);
    assert_eq!(
        picked(&p.resolve().unwrap()),
        ["t.a 1.0.0", "t.b 2.0.0", "t.c 1.0.0"]
    );
}

#[test]
fn conflicts_are_rejected() {
    let mut p = P::new();
    p.add_full("t.a", "1.0.0", &[], &[("t.b", "*")], Some("^1"));
    p.add("t.b", "1.0.0", &[]);
    p.root("t.a", "*").root("t.b", "*");
    assert_eq!(err(&p), "`t.a 1.0.0` conflicts with `t.b 1.0.0`");
}

#[test]
fn conflicts_declared_by_the_later_package_are_rejected() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[]);
    p.add_full("t.b", "1.0.0", &[], &[("t.a", "^1")], Some("^1"));
    p.root("t.a", "*").root("t.b", "*");
    assert_eq!(
        p.resolve().unwrap_err(),
        ResolveError::Conflict {
            a: "`t.b 1.0.0`".into(),
            b: "`t.a 1.0.0`".into()
        }
    );
}

#[test]
fn conflicts_only_match_their_requirement_and_backtracking_avoids_them() {
    let mut p = P::new();
    p.add_full("t.a", "2.0.0", &[], &[("t.b", "^2")], Some("^1"));
    p.add("t.b", "2.0.0", &[]);
    p.add("t.b", "1.0.0", &[]);
    p.root("t.a", "*").root("t.b", "*");
    assert_eq!(picked(&p.resolve().unwrap()), ["t.a 2.0.0", "t.b 1.0.0"]);

    // The other direction: the later package declares the conflict.
    let mut p = P::new();
    p.add("t.a", "2.0.0", &[]);
    p.add("t.a", "1.0.0", &[]);
    p.add_full("t.b", "1.0.0", &[], &[("t.a", ">=2")], Some("^1"));
    p.root("t.a", "*").root("t.b", "*");
    assert_eq!(picked(&p.resolve().unwrap()), ["t.a 1.0.0", "t.b 1.0.0"]);
}

#[test]
fn conflicts_with_unselected_packages_are_irrelevant() {
    let mut p = P::new();
    p.add_full("t.a", "1.0.0", &[], &[("t.other", "*")], Some("^1"));
    p.add("t.other", "1.0.0", &[]);
    p.root("t.a", "*");
    assert_eq!(picked(&p.resolve().unwrap()), ["t.a 1.0.0"]);
}

#[test]
fn api_eligibility() {
    let mut p = P::new();
    p.add_full("t.a", "2.0.0", &[], &[], Some("^2"));
    p.add_full("t.a", "1.0.0", &[], &[], Some("^1"));
    p.add_full("t.bundle", "1.0.0", &[("t.a", "*")], &[], None);
    p.root("t.bundle", "*");
    assert_eq!(
        picked(&p.resolve().unwrap()),
        ["t.a 1.0.0", "t.bundle 1.0.0"]
    );

    p.problem.api = Some(v("2.3.0"));
    assert_eq!(
        picked(&p.resolve().unwrap()),
        ["t.a 2.0.0", "t.bundle 1.0.0"]
    );

    // No API given: no check, newest wins.
    p.problem.api = None;
    assert_eq!(
        picked(&p.resolve().unwrap()),
        ["t.a 2.0.0", "t.bundle 1.0.0"]
    );
}

#[test]
fn api_mismatch_is_reported() {
    let mut p = P::new();
    p.add_full("pwc.inventory", "1.0.0", &[], &[], Some("^2.0"));
    p.root("pwc.inventory", "*");
    assert_eq!(
        err(&p),
        "pwc.inventory 1.0.0 requires pwc-api ^2.0 but this PWC provides 1.0.0"
    );
}

#[test]
fn unknown_root() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[]);
    p.root("foo.x", "*");
    assert_eq!(
        err(&p),
        "unknown package `foo.x` (required by the instance)"
    );
}

#[test]
fn unknown_dependency() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[("foo.x", "^1")]);
    p.root("t.a", "*");
    assert_eq!(
        p.resolve().unwrap_err(),
        ResolveError::Unknown {
            id: id("foo.x"),
            required_by: "`t.a 1.0.0`".into()
        }
    );
}

#[test]
fn no_matching_dependency_version_names_the_available_ones() {
    let mut p = P::new();
    p.add("pwc.inventory", "1.0.0", &[("pwc.hotbar", "^2.0")]);
    p.add("pwc.hotbar", "1.1.0", &[]);
    p.add("pwc.hotbar", "1.0.0", &[]);
    p.root("pwc.inventory", "*");
    assert_eq!(
        err(&p),
        "no version of `pwc.hotbar` satisfies `^2.0` (required by `pwc.inventory 1.0.0`); available: 1.0.0, 1.1.0"
    );
}

#[test]
fn no_matching_root_version() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[]);
    p.root("t.a", "^3");
    assert_eq!(
        err(&p),
        "no version of `t.a` satisfies `^3` (required by the instance); available: 1.0.0"
    );
}

#[test]
fn incompatible_requirements_name_both_requirers() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[("t.c", "^1")]);
    p.add("t.b", "1.0.0", &[("t.c", "^2")]);
    p.add("t.c", "1.0.0", &[]);
    p.add("t.c", "2.0.0", &[]);
    p.root("t.a", "*").root("t.b", "*");
    assert_eq!(
        err(&p),
        "no compatible set of packages: `t.c` is required as `^1` by `t.a 1.0.0` and as `^2` by \
         `t.b 1.0.0`, and no version satisfies both; available: 1.0.0, 2.0.0"
    );
}

#[test]
fn reports_the_first_dead_end() {
    // Both versions of t.a fail; the newest's failure is reported.
    let mut p = P::new();
    p.add("t.a", "2.0.0", &[("t.b", "^2")]);
    p.add("t.a", "1.0.0", &[("t.missing", "*")]);
    p.add("t.b", "1.0.0", &[]);
    p.root("t.a", "*");
    assert_eq!(
        err(&p),
        "no version of `t.b` satisfies `^2` (required by `t.a 2.0.0`); available: 1.0.0"
    );
}

#[test]
fn chosen_dependency_mismatch_is_explained() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[]);
    p.add("t.b", "1.0.0", &[("t.a", "^2")]);
    p.root("t.a", "*").root("t.b", "*");
    assert_eq!(
        err(&p),
        "no compatible set of packages: `t.b 1.0.0` requires `t.a ^2`, but `t.a 1.0.0` is already chosen"
    );
}

#[test]
fn pins_select_exactly_that_candidate() {
    let mut p = P::new();
    p.add("t.a", "2.0.0", &[]);
    let local = p.add("t.a", "1.0.0", &[("t.b", "*")]);
    p.add("t.b", "1.0.0", &[]);
    p.pin("t.a", local);
    let r = p.resolve().unwrap();
    assert_eq!(picked(&r), ["t.a 1.0.0", "t.b 1.0.0"]);
    assert_eq!(r.get(&id("t.a")).unwrap().candidate.source, local);
}

#[test]
fn pins_win_over_a_same_version_candidate() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[]);
    let local = p.add("t.a", "1.0.0", &[]);
    p.pin("t.a", local);
    assert_eq!(p.resolve().unwrap().packages[0].candidate.source, local);
}

#[test]
fn pin_errors() {
    // A dependent needs another version than the pinned one.
    let mut p = P::new();
    let local = p.add("t.a", "1.0.0", &[]);
    p.add("t.a", "2.0.0", &[]);
    p.add("t.b", "1.0.0", &[("t.a", "^2")]);
    p.pin("t.a", local).root("t.b", "*");
    assert_eq!(
        err(&p),
        "no compatible set of packages: the instance pins `t.a` to 1.0.0, but `t.b 1.0.0` requires `t.a ^2`"
    );

    // A pinned package for the wrong API.
    let mut p = P::new();
    let local = p.add_full("t.a", "1.0.0", &[], &[], Some("^9"));
    p.pin("t.a", local);
    assert!(matches!(p.resolve().unwrap_err(), ResolveError::Api { .. }));

    // A token nobody has.
    let mut p = P::new();
    p.pin("t.a", 42);
    assert_eq!(err(&p), "unknown package `t.a` (required by the instance)");

    // A token for another id.
    let mut p = P::new();
    let other = p.add("t.b", "1.0.0", &[]);
    p.pin("t.a", other);
    assert_eq!(
        err(&p),
        "no compatible set of packages: the instance pins `t.a` to a package that is `t.b 1.0.0`"
    );
}

#[test]
fn previous_lock_is_preferred_until_updated() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[("t.b", "^1")]);
    p.add("t.a", "1.1.0", &[("t.b", "^1")]);
    p.add("t.b", "1.0.0", &[]);
    p.add("t.b", "1.3.0", &[]);
    p.root("t.a", "^1");
    p.previous("t.a", "1.0.0").previous("t.b", "1.0.0");
    assert_eq!(picked(&p.resolve().unwrap()), ["t.a 1.0.0", "t.b 1.0.0"]);

    p.problem.update.insert(id("t.a"));
    assert_eq!(picked(&p.resolve().unwrap()), ["t.a 1.1.0", "t.b 1.0.0"]);

    p.problem.update.clear();
    p.problem.update_all = true;
    assert_eq!(picked(&p.resolve().unwrap()), ["t.a 1.1.0", "t.b 1.3.0"]);
}

#[test]
fn previous_lock_is_dropped_when_no_longer_valid() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[]);
    p.add("t.a", "1.1.0", &[]);
    p.add("t.a", "1.2.0", &[]);
    p.root("t.a", ">=1.1");
    p.previous("t.a", "1.0.0");
    assert_eq!(picked(&p.resolve().unwrap()), ["t.a 1.2.0"]);

    // Locked version no longer available at all.
    let mut p = P::new();
    p.add("t.a", "1.1.0", &[]);
    p.root("t.a", "*").previous("t.a", "1.0.5");
    assert_eq!(picked(&p.resolve().unwrap()), ["t.a 1.1.0"]);

    // Locked version not eligible for the API any more.
    let mut p = P::new();
    p.add_full("t.a", "1.0.0", &[], &[], Some("^2"));
    p.add("t.a", "0.9.0", &[]);
    p.root("t.a", "*").previous("t.a", "1.0.0");
    assert_eq!(picked(&p.resolve().unwrap()), ["t.a 0.9.0"]);
}

#[test]
fn previous_lock_of_a_dependency_survives_updating_its_dependent() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[("t.b", "^1")]);
    p.add("t.a", "1.1.0", &[("t.b", "^1.2")]);
    p.add("t.b", "1.0.0", &[]);
    p.add("t.b", "1.2.0", &[]);
    p.add("t.b", "1.3.0", &[]);
    p.root("t.a", "*")
        .previous("t.a", "1.0.0")
        .previous("t.b", "1.0.0");
    p.problem.update.insert(id("t.a"));
    // t.b's locked 1.0.0 no longer fits t.a 1.1.0: the newest matching version is taken.
    assert_eq!(picked(&p.resolve().unwrap()), ["t.a 1.1.0", "t.b 1.3.0"]);
}

#[test]
fn pre_releases_need_an_explicit_requirement() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[]);
    p.add("t.a", "2.0.0-beta.1", &[]);
    p.root("t.a", "*");
    assert_eq!(picked(&p.resolve().unwrap()), ["t.a 1.0.0"]);

    p.root("t.a", ">=2.0.0-beta");
    assert_eq!(picked(&p.resolve().unwrap()), ["t.a 2.0.0-beta.1"]);

    // Through a dependency.
    let mut p = P::new();
    p.add("t.a", "2.0.0-beta.1", &[]);
    p.add("t.a", "1.5.0", &[]);
    p.add("t.b", "1.0.0", &[("t.a", "^2.0.0-beta.1")]);
    p.root("t.b", "*");
    assert_eq!(
        picked(&p.resolve().unwrap()),
        ["t.a 2.0.0-beta.1", "t.b 1.0.0"]
    );

    // Only a pre-release exists and nobody asked for one.
    let mut p = P::new();
    p.add("t.a", "2.0.0-beta.1", &[]);
    p.root("t.a", "^2");
    assert!(matches!(
        p.resolve().unwrap_err(),
        ResolveError::NoMatch { .. }
    ));
}

#[test]
fn equal_versions_keep_the_callers_order() {
    let mut p = P::new();
    let first = p.add("t.a", "1.0.0", &[]);
    p.add("t.a", "1.0.0", &[]);
    p.root("t.a", "*");
    assert_eq!(p.resolve().unwrap().packages[0].candidate.source, first);

    // ... but an equal-version candidate that does not fit is skipped.
    let mut p = P::new();
    p.add_full("t.a", "1.0.0", &[], &[], Some("^9"));
    let second = p.add("t.a", "1.0.0", &[]);
    p.root("t.a", "*");
    assert_eq!(p.resolve().unwrap().packages[0].candidate.source, second);
}

#[test]
fn cycles_are_errors() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[("t.b", "*")]);
    p.add("t.b", "1.0.0", &[("t.a", "*")]);
    p.root("t.a", "*");
    assert_eq!(err(&p), "dependency cycle: t.a → t.b → t.a");

    // A cycle reached through a non-member.
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[("t.b", "*")]);
    p.add("t.b", "1.0.0", &[("t.c", "*")]);
    p.add("t.c", "1.0.0", &[("t.d", "*")]);
    p.add("t.d", "1.0.0", &[("t.b", "*")]);
    p.root("t.a", "*");
    assert_eq!(err(&p), "dependency cycle: t.b → t.c → t.d → t.b");
}

#[test]
fn registration_order_is_topological_with_ties_by_id() {
    let mut p = P::new();
    p.add("t.z", "1.0.0", &[]);
    p.add("t.y", "1.0.0", &[("t.x", "*")]);
    p.add("t.x", "1.0.0", &[]);
    p.add("t.m", "1.0.0", &[("t.z", "*"), ("t.y", "*")]);
    p.add("t.b", "1.0.0", &[]);
    p.root("t.m", "*").root("t.b", "*");
    let r = p.resolve().unwrap();
    assert_eq!(
        picked(&r),
        [
            "t.b 1.0.0",
            "t.m 1.0.0",
            "t.x 1.0.0",
            "t.y 1.0.0",
            "t.z 1.0.0"
        ]
    );
    assert_eq!(order(&r), ["t.b", "t.x", "t.y", "t.z", "t.m"]);
}

#[test]
fn deterministic_regardless_of_candidate_order() {
    let mut p = P::new();
    p.add("t.a", "1.0.0", &[("t.b", "^1"), ("t.c", "*")]);
    p.add("t.a", "1.1.0", &[("t.b", "^1.1"), ("t.c", "^2")]);
    p.add("t.b", "1.0.0", &[]);
    p.add("t.b", "1.1.0", &[("t.d", "*")]);
    p.add("t.c", "1.0.0", &[]);
    p.add("t.c", "2.0.0", &[("t.d", "^1")]);
    p.add("t.d", "1.0.0", &[]);
    p.add("t.d", "1.4.0", &[]);
    p.root("t.a", "*");
    let expected = picked(&p.resolve().unwrap());
    assert_eq!(
        expected,
        ["t.a 1.1.0", "t.b 1.1.0", "t.c 2.0.0", "t.d 1.4.0"]
    );
    let mut rng = Rng(7);
    for _ in 0..50 {
        let mut shuffled = p.problem.clone();
        for i in (1..shuffled.candidates.len()).rev() {
            let j = rng.below(i + 1);
            shuffled.candidates.swap(i, j);
        }
        let r = resolve(&shuffled).unwrap();
        assert_eq!(picked(&r), expected);
        assert_eq!(resolve(&shuffled).unwrap(), r, "same input, same output");
    }
}

#[test]
fn backjumping_skips_unrelated_decisions() {
    // Five roots with 60 versions each, then one whose only version needs something impossible.
    // Chronological backtracking would retry 60^5 combinations; the failure does not depend on
    // them, so the search gives up at once.
    let mut p = P::new();
    for name in ["t.a", "t.b", "t.c", "t.d", "t.e"] {
        for minor in 0..60 {
            p.add(name, &format!("1.{minor}.0"), &[]);
        }
        p.root(name, "*");
    }
    p.add("t.z", "1.0.0", &[("t.dep", "^2")]);
    p.add("t.dep", "1.0.0", &[]);
    p.root("t.z", "*");
    assert_eq!(
        err(&p),
        "no version of `t.dep` satisfies `^2` (required by `t.z 1.0.0`); available: 1.0.0"
    );
}

#[test]
fn backjumping_still_retries_the_culprit() {
    // t.a (decided first) causes the failure deep down; unrelated t.b..t.e have many versions.
    let mut p = P::new();
    p.add("t.a", "2.0.0", &[("t.z", "^2")]);
    p.add("t.a", "1.0.0", &[("t.z", "^1")]);
    for name in ["t.b", "t.c", "t.d", "t.e"] {
        for minor in 0..40 {
            p.add(name, &format!("1.{minor}.0"), &[]);
        }
        p.root(name, "*");
    }
    p.add("t.z", "1.0.0", &[]);
    p.add("t.z", "2.0.0", &[("t.nothing", "*")]);
    p.root("t.a", "*");
    let r = p.resolve().unwrap();
    assert_eq!(
        picked(&r),
        [
            "t.a 1.0.0",
            "t.b 1.39.0",
            "t.c 1.39.0",
            "t.d 1.39.0",
            "t.e 1.39.0",
            "t.z 1.0.0"
        ]
    );
}

// ---------------------------------------------------------------------------------------------
// Randomised comparison.

/// xorshift64*.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

const IDS: [&str; 4] = ["r.a", "r.b", "r.c", "r.d"];
const VERSIONS: [&str; 5] = ["1.0.0", "1.1.0", "2.0.0", "2.1.0", "3.0.0-beta.1"];
const REQS: [&str; 8] = [
    "*",
    "^1",
    "^2",
    ">=1.1",
    "=1.0.0",
    "<2",
    "^1.1",
    ">=3.0.0-beta",
];

fn random_problem(rng: &mut Rng) -> Problem {
    let mut p = P::new();
    for name in IDS {
        let mut versions: Vec<&str> = VERSIONS
            .iter()
            .copied()
            .filter(|_| rng.chance(45))
            .collect();
        if versions.is_empty() && rng.chance(80) {
            versions.push(rng.pick(&VERSIONS));
        }
        for version in versions {
            let mut deps: Vec<(&str, &str)> = Vec::new();
            let mut conflicts: Vec<(&str, &str)> = Vec::new();
            for d in IDS {
                if d == name {
                    continue;
                }
                if rng.chance(25) {
                    deps.push((d, rng.pick(&REQS)));
                } else if rng.chance(10) {
                    conflicts.push((d, rng.pick(&REQS)));
                }
            }
            let api = match rng.below(10) {
                0 => None,
                1 => Some("^2"),
                _ => Some("^1"),
            };
            p.add_full(name, version, &deps, &conflicts, api);
            if rng.chance(10) {
                // A same-version duplicate with different dependencies.
                p.add_full(name, version, &[], &[], Some("^1"));
            }
        }
    }
    for name in IDS {
        if rng.chance(40) {
            if rng.chance(15) {
                let tokens: Vec<usize> = p
                    .problem
                    .candidates
                    .iter()
                    .filter(|c| c.id.as_str() == name)
                    .map(|c| c.source)
                    .collect();
                if !tokens.is_empty() {
                    let token = tokens[rng.below(tokens.len())];
                    p.pin(name, token);
                    continue;
                }
            }
            p.root(name, rng.pick(&REQS));
        }
        if rng.chance(30) {
            p.previous(name, rng.pick(&VERSIONS));
        }
    }
    if rng.chance(20) {
        p.problem.update.insert(id(rng.pick(&IDS)));
    }
    p.problem.update_all = rng.chance(10);
    p.problem
}

/// Whether `c` (for its id) is compatible with `selected` and the problem's fixed rules.
fn fits(problem: &Problem, selected: &BTreeMap<PackageId, usize>, c: usize) -> bool {
    let cand = &problem.candidates[c];
    if !cand.is_eligible(problem.api.as_ref()) {
        return false;
    }
    match problem.roots.get(&cand.id) {
        Some(Requirement::Version(r)) if !r.matches(&cand.version) => return false,
        Some(Requirement::Pin(token)) if *token != cand.source => return false,
        _ => {}
    }
    for &s in selected.values() {
        let other = &problem.candidates[s];
        if let Some(r) = other.dependencies.get(&cand.id)
            && !r.matches(&cand.version)
        {
            return false;
        }
        if let Some(r) = cand.dependencies.get(&other.id)
            && !r.matches(&other.version)
        {
            return false;
        }
        if let Some(r) = other.conflicts.get(&cand.id)
            && r.matches(&cand.version)
        {
            return false;
        }
        if let Some(r) = cand.conflicts.get(&other.id)
            && r.matches(&other.version)
        {
            return false;
        }
    }
    true
}

/// Plain chronological backtracking with the documented agenda and preference order.
fn reference(problem: &Problem) -> Option<BTreeMap<PackageId, usize>> {
    fn pool(problem: &Problem, id: &PackageId) -> Vec<usize> {
        let previous = if problem.update_all || problem.update.contains(id) {
            None
        } else {
            problem.previous.get(id)
        };
        let mut pool: Vec<usize> = (0..problem.candidates.len())
            .filter(|&c| &problem.candidates[c].id == id)
            .collect();
        pool.sort_by(|&a, &b| {
            let (ca, cb) = (&problem.candidates[a], &problem.candidates[b]);
            (previous == Some(&cb.version))
                .cmp(&(previous == Some(&ca.version)))
                .then(cb.version.cmp(&ca.version))
        });
        pool
    }
    fn go(
        problem: &Problem,
        agenda: &[PackageId],
        pos: usize,
        selected: &BTreeMap<PackageId, usize>,
    ) -> Option<BTreeMap<PackageId, usize>> {
        let Some(id) = agenda.get(pos) else {
            return Some(selected.clone());
        };
        for c in pool(problem, id) {
            if !fits(problem, selected, c) {
                continue;
            }
            let mut selected = selected.clone();
            selected.insert(id.clone(), c);
            let mut agenda = agenda.to_vec();
            for dep in problem.candidates[c].dependencies.keys() {
                if !agenda.contains(dep) {
                    agenda.push(dep.clone());
                }
            }
            if let Some(found) = go(problem, &agenda, pos + 1, &selected) {
                return Some(found);
            }
        }
        None
    }
    let agenda: Vec<PackageId> = problem.roots.keys().cloned().collect();
    go(problem, &agenda, 0, &BTreeMap::new())
}

/// Whether any valid selection exists (every subset of candidates, one per id).
fn brute_force_exists(problem: &Problem) -> bool {
    let ids: Vec<&str> = IDS.to_vec();
    fn go(problem: &Problem, ids: &[&str], selected: &mut BTreeMap<PackageId, usize>) -> bool {
        let Some((first, rest)) = ids.split_first() else {
            return is_valid(problem, selected);
        };
        if go(problem, rest, selected) {
            return true;
        }
        let pid = id(first);
        for c in 0..problem.candidates.len() {
            if problem.candidates[c].id == pid {
                selected.insert(pid.clone(), c);
                let ok = go(problem, rest, selected);
                selected.remove(&pid);
                if ok {
                    return true;
                }
            }
        }
        false
    }
    go(problem, &ids, &mut BTreeMap::new())
}

/// Every rule of the spec holds for `selected` (except cycles, checked separately).
fn is_valid(problem: &Problem, selected: &BTreeMap<PackageId, usize>) -> bool {
    for root in problem.roots.keys() {
        if !selected.contains_key(root) {
            return false;
        }
    }
    for (pid, &c) in selected {
        let cand = &problem.candidates[c];
        if &cand.id != pid {
            return false;
        }
        for dep in cand.dependencies.keys() {
            if !selected.contains_key(dep) {
                return false;
            }
        }
        let mut others = selected.clone();
        others.remove(pid);
        if !fits(problem, &others, c) {
            return false;
        }
    }
    true
}

fn has_cycle(problem: &Problem, selected: &BTreeMap<PackageId, usize>) -> bool {
    let packages: Vec<Selected> = selected
        .values()
        .map(|&c| Selected {
            candidate: problem.candidates[c].clone(),
            dependencies: problem.candidates[c].dependencies.keys().cloned().collect(),
        })
        .collect();
    registration_order(&packages).is_err()
}

#[test]
fn randomised_agreement_with_chronological_backtracking_and_brute_force() {
    let mut rng = Rng(0x5eed_1234_abcd_0001);
    let (mut solved, mut failed, mut cycles) = (0, 0, 0);
    for round in 0..3000 {
        let problem = random_problem(&mut rng);
        let expected = reference(&problem);
        match (resolve(&problem), expected) {
            (Ok(r), Some(sel)) => {
                let got: BTreeMap<PackageId, usize> = r
                    .packages
                    .iter()
                    .map(|s| (s.candidate.id.clone(), s.candidate.source))
                    .collect();
                let want: BTreeMap<PackageId, usize> = sel
                    .iter()
                    .map(|(k, &c)| (k.clone(), problem.candidates[c].source))
                    .collect();
                assert_eq!(got, want, "round {round}: {problem:#?}");
                assert!(is_valid(&problem, &sel), "round {round}");
                // The order is a topological order containing every package once.
                let mut seen = BTreeSet::new();
                for pid in &r.order {
                    for dep in &r.get(pid).unwrap().dependencies {
                        assert!(seen.contains(dep), "round {round}: {dep} before {pid}");
                    }
                    seen.insert(pid.clone());
                }
                assert_eq!(seen.len(), r.packages.len());
                solved += 1;
            }
            (Err(ResolveError::Cycle(_)), Some(sel)) => {
                assert!(has_cycle(&problem, &sel), "round {round}");
                cycles += 1;
            }
            (Err(e), None) => {
                assert!(
                    !brute_force_exists(&problem),
                    "round {round}: resolver said {e} but a solution exists"
                );
                failed += 1;
            }
            (got, want) => {
                panic!("round {round}: resolver {got:?}, reference {want:?}\n{problem:#?}")
            }
        }
    }
    // The generator exercises all outcomes.
    assert!(
        solved > 300 && failed > 300 && cycles > 10,
        "{solved} {failed} {cycles}"
    );
}
