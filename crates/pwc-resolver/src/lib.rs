//! Deterministic dependency resolution (docs/spec/resolver.md). Pure: candidates in, an exact
//! selection (or an explanation) out.
//!
//! # Algorithm
//!
//! Ids are decided one at a time in the order they are first encountered: the root requirements
//! sorted by id, then the dependencies of every chosen package (in sorted order) appended to the
//! end of that agenda — a breadth-first discovery order. For each id the candidates are tried in
//! preference order (the previously locked version first unless the id is being updated, then
//! newest first; candidates with equal id and version in the order the caller listed them). A
//! candidate is accepted when it is eligible for the mod API, satisfies every requirement on its
//! id, agrees with every already chosen package it depends on, and no conflict exists between it
//! and the chosen packages. On a dead end the search backtracks.
//!
//! Backtracking is *conflict-directed*: every failure carries the set of earlier decisions that
//! caused it, and the search jumps straight back to the most recent of them. The subtrees it skips
//! provably contain no solution, so the result is exactly the one plain chronological
//! backtracking would find (the first valid selection in preference order) — only much faster
//! when an unrelated, impossible requirement would otherwise make the search retry every
//! combination of the decisions in between.
//!
//! When no solution exists, the error explains the first dead end met: the one reached under the
//! most preferred choices.

use std::collections::{BTreeMap, BTreeSet};

use pwc_manifest::{ModKind, PackageHash, PackageId, Version, VersionReq};

/// One known version of one package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// Id.
    pub id: PackageId,
    /// Version.
    pub version: Version,
    /// Kind.
    pub kind: ModKind,
    /// Content hash.
    pub hash: PackageHash,
    /// `pwc-api` requirement (`None` for bundles).
    pub pwc_api: Option<VersionReq>,
    /// Dependencies.
    pub dependencies: BTreeMap<PackageId, VersionReq>,
    /// Conflicts.
    pub conflicts: BTreeMap<PackageId, VersionReq>,
    /// Opaque token the caller uses to find the package again (index into its own list).
    pub source: usize,
}

impl Candidate {
    /// Whether this candidate may be used with mod API `api` (`None` = no API check).
    pub fn is_eligible(&self, api: Option<&Version>) -> bool {
        match (&self.pwc_api, api) {
            (Some(req), Some(api)) => req.matches(api),
            _ => true,
        }
    }

    /// "`<id> <version>`", as used in error messages.
    pub fn label(&self) -> String {
        format!("`{} {}`", self.id, self.version)
    }
}

/// A root requirement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Requirement {
    /// Any version matching.
    Version(VersionReq),
    /// Exactly this candidate (by `source` token), from a `path`/`file` instance entry.
    Pin(usize),
}

/// Everything the resolver needs.
#[derive(Clone, Debug, Default)]
pub struct Problem {
    /// The `pwc-mod-api` version the build provides (`None` = do not check `pwc-api`).
    pub api: Option<Version>,
    /// Root requirements.
    pub roots: BTreeMap<PackageId, Requirement>,
    /// All candidates. The order only matters between candidates with the same id *and* version
    /// (e.g. two different trees both claiming `foo.a 1.0.0`): they are tried in this order, so
    /// callers list their preferred source first.
    pub candidates: Vec<Candidate>,
    /// Previously locked versions.
    pub previous: BTreeMap<PackageId, Version>,
    /// Ids to update (ignore `previous` for these). Empty + `update_all` false = conservative.
    pub update: BTreeSet<PackageId>,
    /// Ignore `previous` entirely.
    pub update_all: bool,
}

/// One selected package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selected {
    /// The candidate chosen.
    pub candidate: Candidate,
    /// Its dependency ids, sorted.
    pub dependencies: Vec<PackageId>,
}

/// The result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
    /// Selected packages sorted by id.
    pub packages: Vec<Selected>,
    /// Topological registration order (dependencies first, ties by id).
    pub order: Vec<PackageId>,
}

impl Resolution {
    /// The selected package with this id.
    pub fn get(&self, id: &PackageId) -> Option<&Selected> {
        self.packages
            .binary_search_by(|s| s.candidate.id.cmp(id))
            .ok()
            .map(|i| &self.packages[i])
    }
}

/// Resolve.
pub fn resolve(problem: &Problem) -> Result<Resolution, ResolveError> {
    let mut solver = Solver::new(problem)?;
    match solver.search(0) {
        Ok(()) => solver.into_resolution(),
        Err(_) => Err(solver.first_failure.take().unwrap_or_else(|| {
            ResolveError::Unsatisfiable("the requirements cannot all be met".to_string())
        })),
    }
}

/// Why resolution failed (messages per docs/spec/resolver.md).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    /// Nobody provides the id.
    #[error("unknown package `{id}` (required by {required_by})")]
    Unknown {
        /// Id.
        id: PackageId,
        /// "the instance" or "`<id> <version>`".
        required_by: String,
    },
    /// No version satisfies the requirement.
    #[error(
        "no version of `{id}` satisfies `{req}` (required by {required_by}); available: {available}"
    )]
    NoMatch {
        /// Id.
        id: PackageId,
        /// Requirement.
        req: String,
        /// Who required it.
        required_by: String,
        /// Comma-separated versions, or "none".
        available: String,
    },
    /// The package needs another mod API.
    #[error("{id} {version} requires pwc-api {req} but this PWC provides {api}")]
    Api {
        /// Id.
        id: PackageId,
        /// Version.
        version: Version,
        /// Its requirement.
        req: String,
        /// Provided API.
        api: String,
    },
    /// Two selected packages conflict.
    #[error("{a} conflicts with {b}")]
    Conflict {
        /// "`<id> <version>`".
        a: String,
        /// "`<id> <version>`".
        b: String,
    },
    /// A cycle.
    #[error("dependency cycle: {0}")]
    Cycle(String),
    /// Constraints cannot all hold (after backtracking); the message names the closest failure.
    #[error("no compatible set of packages: {0}")]
    Unsatisfiable(String),
}

/// Who imposed a root requirement, for messages.
const BY_INSTANCE: &str = "the instance";

/// The ids of the decisions that caused a failure (conflict-directed backjumping).
type Culprits<'a> = BTreeSet<&'a PackageId>;

/// Search state. Candidates are referred to by their index in `problem.candidates`.
struct Solver<'a> {
    problem: &'a Problem,
    /// Per id: candidate indices in preference order.
    pools: BTreeMap<&'a PackageId, Vec<usize>>,
    /// Root version requirements.
    root_reqs: BTreeMap<&'a PackageId, &'a VersionReq>,
    /// Pinned roots: id → candidate index.
    pins: BTreeMap<&'a PackageId, usize>,
    /// Current decisions: id → candidate index.
    selected: BTreeMap<&'a PackageId, usize>,
    /// Per id: chosen candidates that depend on it, in choice order.
    requirers: BTreeMap<&'a PackageId, Vec<usize>>,
    /// Per id: chosen candidates that declare a conflict with it, in choice order.
    conflicters: BTreeMap<&'a PackageId, Vec<usize>>,
    /// Ids in the order they are decided.
    agenda: Vec<&'a PackageId>,
    /// Membership of `agenda`.
    on_agenda: BTreeSet<&'a PackageId>,
    /// The explanation of the first dead end, reported if no solution exists.
    first_failure: Option<ResolveError>,
}

impl<'a> Solver<'a> {
    fn new(problem: &'a Problem) -> Result<Self, ResolveError> {
        let mut pools: BTreeMap<&PackageId, Vec<usize>> = BTreeMap::new();
        for (i, c) in problem.candidates.iter().enumerate() {
            pools.entry(&c.id).or_default().push(i);
        }
        for (id, pool) in pools.iter_mut() {
            let previous = if problem.update_all || problem.update.contains(*id) {
                None
            } else {
                problem.previous.get(*id)
            };
            // Stable: equal (id, version) keep the caller's order.
            pool.sort_by(|&a, &b| {
                let (ca, cb) = (&problem.candidates[a], &problem.candidates[b]);
                let pa = previous == Some(&ca.version);
                let pb = previous == Some(&cb.version);
                pb.cmp(&pa).then_with(|| cb.version.cmp(&ca.version))
            });
        }

        let mut root_reqs = BTreeMap::new();
        let mut pins = BTreeMap::new();
        for (id, req) in &problem.roots {
            match req {
                Requirement::Version(req) => {
                    if !pools.contains_key(id) {
                        return Err(ResolveError::Unknown {
                            id: id.clone(),
                            required_by: BY_INSTANCE.to_string(),
                        });
                    }
                    root_reqs.insert(id, req);
                }
                Requirement::Pin(token) => {
                    let Some(index) = problem
                        .candidates
                        .iter()
                        .position(|c| c.source == *token && &c.id == id)
                    else {
                        if let Some(other) = problem.candidates.iter().find(|c| c.source == *token)
                        {
                            return Err(ResolveError::Unsatisfiable(format!(
                                "the instance pins `{id}` to a package that is {}",
                                other.label()
                            )));
                        }
                        return Err(ResolveError::Unknown {
                            id: id.clone(),
                            required_by: BY_INSTANCE.to_string(),
                        });
                    };
                    pins.insert(id, index);
                }
            }
        }

        let agenda: Vec<&PackageId> = problem.roots.keys().collect();
        let on_agenda = agenda.iter().copied().collect();
        Ok(Self {
            problem,
            pools,
            root_reqs,
            pins,
            selected: BTreeMap::new(),
            requirers: BTreeMap::new(),
            conflicters: BTreeMap::new(),
            agenda,
            on_agenda,
            first_failure: None,
        })
    }

    fn cand(&self, index: usize) -> &'a Candidate {
        &self.problem.candidates[index]
    }

    /// The candidates for `id` in preference order (just the pinned one for a pinned root).
    fn pool(&self, id: &PackageId) -> Vec<usize> {
        match self.pins.get(id) {
            Some(&pin) => vec![pin],
            None => self.pools.get(id).cloned().unwrap_or_default(),
        }
    }

    /// Decide `agenda[pos..]`. `Ok` leaves the solution in `selected`; `Err` carries the culprits:
    /// earlier decisions which, kept as they are, make every continuation fail.
    fn search(&mut self, pos: usize) -> Result<(), Culprits<'a>> {
        let Some(&id) = self.agenda.get(pos) else {
            return Ok(());
        };
        // `id` is on the agenda because of these choices (or because it is a root).
        let mut culprits: Culprits<'a> = self
            .requirers
            .get(id)
            .into_iter()
            .flatten()
            .map(|&r| &self.cand(r).id)
            .collect();
        let mut any_accepted = false;
        for c in self.pool(id) {
            if let Err(culprit) = self.check(id, c) {
                culprits.extend(culprit);
                continue;
            }
            any_accepted = true;
            let agenda_len = self.select(c);
            match self.search(pos + 1) {
                Ok(()) => return Ok(()),
                Err(deeper) => {
                    self.unselect(c, agenda_len);
                    if !deeper.contains(id) {
                        // This choice is irrelevant to the failure: no alternative can help.
                        return Err(deeper);
                    }
                    culprits.extend(deeper.into_iter().filter(|&d| d != id));
                }
            }
        }
        if !any_accepted && self.first_failure.is_none() {
            self.first_failure = Some(self.explain(id));
        }
        culprits.remove(id);
        Err(culprits)
    }

    /// Whether candidate `c` for `id` fits the current decisions. On rejection, the decision that
    /// rules it out (`None` when nothing decided could change that: API, root requirement).
    fn check(&self, id: &PackageId, c: usize) -> Result<(), Option<&'a PackageId>> {
        let cand = self.cand(c);
        if !cand.is_eligible(self.problem.api.as_ref()) {
            return Err(None);
        }
        if let Some(req) = self.root_reqs.get(id)
            && !req.matches(&cand.version)
        {
            return Err(None);
        }
        for &r in self.requirers.get(id).into_iter().flatten() {
            let requirer = self.cand(r);
            if !requirer.dependencies[id].matches(&cand.version) {
                return Err(Some(&requirer.id));
            }
        }
        for (dep, req) in &cand.dependencies {
            if let Some(&s) = self.selected.get(dep)
                && !req.matches(&self.cand(s).version)
            {
                return Err(Some(&self.cand(s).id));
            }
        }
        for (other, req) in &cand.conflicts {
            if let Some(&s) = self.selected.get(other)
                && req.matches(&self.cand(s).version)
            {
                return Err(Some(&self.cand(s).id));
            }
        }
        for &r in self.conflicters.get(id).into_iter().flatten() {
            let conflicter = self.cand(r);
            if conflicter.conflicts[id].matches(&cand.version) {
                return Err(Some(&conflicter.id));
            }
        }
        Ok(())
    }

    /// Choose `c`; returns the agenda length before, for [`Self::unselect`].
    fn select(&mut self, c: usize) -> usize {
        let cand = self.cand(c);
        let agenda_len = self.agenda.len();
        self.selected.insert(&cand.id, c);
        for dep in cand.dependencies.keys() {
            self.requirers.entry(dep).or_default().push(c);
            if self.on_agenda.insert(dep) {
                self.agenda.push(dep);
            }
        }
        for other in cand.conflicts.keys() {
            self.conflicters.entry(other).or_default().push(c);
        }
        agenda_len
    }

    /// Undo [`Self::select`] (choices are undone in reverse order, so `c` is last everywhere).
    fn unselect(&mut self, c: usize, agenda_len: usize) {
        let cand = self.cand(c);
        self.selected.remove(&cand.id);
        for dep in cand.dependencies.keys() {
            if let Some(list) = self.requirers.get_mut(dep) {
                list.pop();
            }
        }
        for other in cand.conflicts.keys() {
            if let Some(list) = self.conflicters.get_mut(other) {
                list.pop();
            }
        }
        for id in self.agenda.drain(agenda_len..) {
            self.on_agenda.remove(id);
        }
    }

    /// Explain why no candidate for `id` fits the current decisions.
    fn explain(&self, id: &PackageId) -> ResolveError {
        let pool = self.pool(id);
        // Every requirement on `id`, with who imposed it.
        let mut reqs: Vec<(String, &VersionReq)> = Vec::new();
        if let Some(req) = self.root_reqs.get(id) {
            reqs.push((BY_INSTANCE.to_string(), req));
        }
        for &r in self.requirers.get(id).into_iter().flatten() {
            let requirer = self.cand(r);
            reqs.push((requirer.label(), &requirer.dependencies[id]));
        }
        let first_by = reqs
            .first()
            .map_or_else(|| BY_INSTANCE.to_string(), |(by, _)| by.clone());
        if pool.is_empty() {
            return ResolveError::Unknown {
                id: id.clone(),
                required_by: first_by,
            };
        }
        let versions: BTreeSet<&Version> = pool.iter().map(|&c| &self.cand(c).version).collect();
        let available = versions
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let matches = |c: usize, req: &VersionReq| req.matches(&self.cand(c).version);

        if let Some(&pin) = self.pins.get(id) {
            if let Some((by, req)) = reqs.iter().find(|(_, req)| !matches(pin, req)) {
                return ResolveError::Unsatisfiable(format!(
                    "the instance pins `{id}` to {}, but {by} requires `{id} {req}`",
                    self.cand(pin).version
                ));
            }
        } else {
            if let Some((by, req)) = reqs
                .iter()
                .find(|(_, req)| !pool.iter().any(|&c| matches(c, req)))
            {
                return ResolveError::NoMatch {
                    id: id.clone(),
                    req: req.to_string(),
                    required_by: by.clone(),
                    available,
                };
            }
            if !pool
                .iter()
                .any(|&c| reqs.iter().all(|(_, req)| matches(c, req)))
            {
                for (i, (by_a, req_a)) in reqs.iter().enumerate() {
                    for (by_b, req_b) in &reqs[i + 1..] {
                        if !pool.iter().any(|&c| matches(c, req_a) && matches(c, req_b)) {
                            return ResolveError::Unsatisfiable(format!(
                                "`{id}` is required as `{req_a}` by {by_a} and as `{req_b}` by {by_b}, \
                                 and no version satisfies both; available: {available}"
                            ));
                        }
                    }
                }
                let all = reqs
                    .iter()
                    .map(|(by, req)| format!("`{req}` by {by}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                return ResolveError::Unsatisfiable(format!(
                    "no version of `{id}` satisfies every requirement ({all}); available: {available}"
                ));
            }
        }

        let matching: Vec<usize> = pool
            .iter()
            .copied()
            .filter(|&c| reqs.iter().all(|(_, req)| matches(c, req)))
            .collect();
        let api = self.problem.api.as_ref();
        let Some(&first) = matching.iter().find(|&&c| self.cand(c).is_eligible(api)) else {
            let cand = self.cand(matching[0]);
            return ResolveError::Api {
                id: cand.id.clone(),
                version: cand.version.clone(),
                req: cand
                    .pwc_api
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                api: api.map(ToString::to_string).unwrap_or_default(),
            };
        };

        // Eligible and acceptable to every requirer: ruled out by what is already chosen.
        let cand = self.cand(first);
        for (dep, req) in &cand.dependencies {
            if let Some(&s) = self.selected.get(dep)
                && !req.matches(&self.cand(s).version)
            {
                if self.pins.contains_key(dep) {
                    return ResolveError::Unsatisfiable(format!(
                        "the instance pins `{dep}` to {}, but {} requires `{dep} {req}`",
                        self.cand(s).version,
                        cand.label()
                    ));
                }
                return ResolveError::Unsatisfiable(format!(
                    "{} requires `{dep} {req}`, but {} is already chosen",
                    cand.label(),
                    self.cand(s).label()
                ));
            }
        }
        for (other, req) in &cand.conflicts {
            if let Some(&s) = self.selected.get(other)
                && req.matches(&self.cand(s).version)
            {
                return ResolveError::Conflict {
                    a: cand.label(),
                    b: self.cand(s).label(),
                };
            }
        }
        for &r in self.conflicters.get(id).into_iter().flatten() {
            let conflicter = self.cand(r);
            if conflicter.conflicts[id].matches(&cand.version) {
                return ResolveError::Conflict {
                    a: conflicter.label(),
                    b: cand.label(),
                };
            }
        }
        ResolveError::Unsatisfiable(format!("no version of `{id}` fits; available: {available}"))
    }

    /// Package the decisions: sorted by id, plus the registration order.
    fn into_resolution(self) -> Result<Resolution, ResolveError> {
        let packages: Vec<Selected> = self
            .selected
            .values()
            .map(|&c| {
                let candidate = self.cand(c).clone();
                let dependencies = candidate.dependencies.keys().cloned().collect();
                Selected {
                    candidate,
                    dependencies,
                }
            })
            .collect();
        let order = registration_order(&packages)?;
        Ok(Resolution { packages, order })
    }
}

/// Topological order of `packages` (sorted by id, dependencies all selected): dependencies first,
/// ties broken by id.
fn registration_order(packages: &[Selected]) -> Result<Vec<PackageId>, ResolveError> {
    let deps: BTreeMap<&PackageId, &[PackageId]> = packages
        .iter()
        .map(|s| (&s.candidate.id, s.dependencies.as_slice()))
        .collect();
    let mut waiting: BTreeMap<&PackageId, usize> = BTreeMap::new();
    let mut dependents: BTreeMap<&PackageId, Vec<&PackageId>> = BTreeMap::new();
    for (&id, ds) in &deps {
        let ds: Vec<&PackageId> = ds.iter().filter(|d| deps.contains_key(d)).collect();
        waiting.insert(id, ds.len());
        for d in ds {
            dependents.entry(d).or_default().push(id);
        }
    }
    let mut ready: BTreeSet<&PackageId> = waiting
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut order = Vec::with_capacity(packages.len());
    while let Some(id) = ready.pop_first() {
        waiting.remove(id);
        order.push(id.clone());
        for &dependent in dependents.get(id).into_iter().flatten() {
            if let Some(n) = waiting.get_mut(dependent) {
                *n -= 1;
                if *n == 0 {
                    ready.insert(dependent);
                }
            }
        }
    }
    let Some((&start, _)) = waiting.first_key_value() else {
        return Ok(order);
    };
    // Every package left waits on another one left: walk from the smallest until a repeat.
    let mut path = vec![start];
    loop {
        let current = path[path.len() - 1];
        let Some(next) = deps[current].iter().find(|d| waiting.contains_key(d)) else {
            return Err(ResolveError::Cycle(current.to_string())); // unreachable
        };
        if let Some(at) = path.iter().position(|p| *p == next) {
            let mut cycle: Vec<String> = path[at..].iter().map(ToString::to_string).collect();
            cycle.push(next.to_string());
            return Err(ResolveError::Cycle(cycle.join(" → ")));
        }
        path.push(next);
    }
}

#[cfg(test)]
mod tests;
