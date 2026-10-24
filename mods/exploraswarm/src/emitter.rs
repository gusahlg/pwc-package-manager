// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Emitters propose the next genome. `Random` fills the archive, then `Gaussian` and `IsoLine`
//! mutate elites. A scheduler picks between those two with a bandit on recent insertion rate.
//!
//! Parent selection is curiosity-biased: draw 4 elites, and with probability 0.75 take the one
//! with the highest `rarity + bonus_norm`, otherwise a uniform draw among the 4. Rarity is the
//! mean, over the elite's signatures, of `1 / (1 + elites holding that signature)`.
//!
//! `ask` returns `None` when an emitter has nothing to offer (a future critic still training).
//! An emitter may own a background thread later; the ones here do their work inline.

use std::collections::VecDeque;

use crate::archive::{Archive, Elite, Genome, Settle};
use crate::experience::Evaluation;
use crate::nn::{self, NetSpec};
use crate::rng::Rng;

pub const ID_RANDOM: u8 = 0;
pub const ID_GAUSSIAN: u8 = 1;
pub const ID_ISOLINE: u8 = 2;
pub const WARMUP_ELITES: usize = 16;

/// Stable name for an emitter id. Unknown ids share one label.
pub fn name_of(id: u8) -> &'static str {
    match id {
        ID_RANDOM => "random",
        ID_GAUSSIAN => "gaussian",
        ID_ISOLINE => "isoline",
        _ => "emitter",
    }
}
const TOURNAMENT: usize = 4;
const CURIOSITY: f64 = 0.75;
const WINDOW: usize = 100;
const EPSILON: f64 = 0.1;
const SIGMA_ISO: f32 = 0.01;
const SIGMA_LINE: f32 = 0.2;

pub trait Emitter: Send {
    fn name(&self) -> &'static str;
    fn id(&self) -> u8;
    fn ask(&mut self, archive: &Archive, rng: &mut Rng) -> Option<Genome>;
    fn tell(&mut self, eval: &Evaluation, outcome: Settle);
}

pub struct Random {
    spec: NetSpec,
}

impl Random {
    pub fn new(spec: NetSpec) -> Self {
        Self { spec }
    }
}

impl Emitter for Random {
    fn name(&self) -> &'static str {
        "random"
    }
    fn id(&self) -> u8 {
        ID_RANDOM
    }
    fn ask(&mut self, _archive: &Archive, rng: &mut Rng) -> Option<Genome> {
        Some(Genome {
            id: 0,
            parent: None,
            emitter: ID_RANDOM,
            params: nn::init(&self.spec, rng),
        })
    }
    fn tell(&mut self, _eval: &Evaluation, _outcome: Settle) {}
}

pub struct Gaussian {
    spec: NetSpec,
    pub sigma: f32,
}

impl Gaussian {
    pub fn new(spec: NetSpec, sigma: f32) -> Self {
        Self { spec, sigma }
    }
}

impl Emitter for Gaussian {
    fn name(&self) -> &'static str {
        "gaussian"
    }
    fn id(&self) -> u8 {
        ID_GAUSSIAN
    }
    fn ask(&mut self, archive: &Archive, rng: &mut Rng) -> Option<Genome> {
        let (parent_id, mut params) = {
            let parent = select_parent(archive, rng)?;
            (
                parent.genome.id,
                fit_params(&parent.genome.params, &self.spec),
            )
        };
        let sigma = self.sigma;
        for p in &mut params {
            *p += sigma * rng.normal() as f32;
        }
        Some(Genome {
            id: 0,
            parent: Some(parent_id),
            emitter: ID_GAUSSIAN,
            params,
        })
    }
    fn tell(&mut self, _eval: &Evaluation, _outcome: Settle) {}
}

pub struct IsoLine {
    spec: NetSpec,
}

impl IsoLine {
    pub fn new(spec: NetSpec) -> Self {
        Self { spec }
    }
}

impl Emitter for IsoLine {
    fn name(&self) -> &'static str {
        "isoline"
    }
    fn id(&self) -> u8 {
        ID_ISOLINE
    }
    fn ask(&mut self, archive: &Archive, rng: &mut Rng) -> Option<Genome> {
        let x = {
            let parent = select_parent(archive, rng)?;
            (
                parent.genome.id,
                fit_params(&parent.genome.params, &self.spec),
            )
        };
        let mut y = {
            let parent = select_parent(archive, rng)?;
            (
                parent.genome.id,
                fit_params(&parent.genome.params, &self.spec),
            )
        };
        if y.0 == x.0
            && let Some(other) = other_parent(archive, x.0)
        {
            y = other;
        }
        let line = rng.normal() as f32;
        let mut params = vec![0.0; x.1.len()];
        for (i, param) in params.iter_mut().enumerate() {
            let iso = rng.normal() as f32;
            let yi = y.1.get(i).copied().unwrap_or(x.1[i]);
            *param = x.1[i] + SIGMA_ISO * iso + SIGMA_LINE * (yi - x.1[i]) * line;
        }
        Some(Genome {
            id: 0,
            parent: Some(x.0),
            emitter: ID_ISOLINE,
            params,
        })
    }
    fn tell(&mut self, _eval: &Evaluation, _outcome: Settle) {}
}

fn fit_params(params: &[f32], spec: &NetSpec) -> Vec<f32> {
    let n = nn::param_count(spec);
    let mut out = vec![0.0; n];
    let copy = params.len().min(n);
    out[..copy].copy_from_slice(&params[..copy]);
    out
}

fn other_parent(archive: &Archive, not: u64) -> Option<(u64, Vec<f32>)> {
    let cell = archive
        .occupied()
        .into_iter()
        .find(|&c| archive.elite(c).is_some_and(|e| e.genome.id != not))?;
    let elite = archive.elite(cell)?;
    Some((elite.genome.id, elite.genome.params.clone()))
}

/// `rarity + bonus/(1+|bonus|)`. Higher is more interesting as a parent.
pub fn curiosity(archive: &Archive, elite: &Elite) -> f32 {
    rarity(archive, elite) + bonus_norm(elite.bonus)
}

fn rarity(archive: &Archive, elite: &Elite) -> f32 {
    if elite.signatures.is_empty() {
        return 0.0;
    }
    let sum: f32 = elite
        .signatures
        .iter()
        .map(|s| 1.0 / (1.0 + archive.signature_count(*s) as f32))
        .sum();
    sum / elite.signatures.len() as f32
}

fn bonus_norm(bonus: f32) -> f32 {
    if bonus.is_finite() {
        bonus / (1.0 + bonus.abs())
    } else {
        0.0
    }
}

/// Draw up to 4 elites. With probability 0.75 return the most curious, else a uniform one of them.
pub fn select_parent<'a>(archive: &'a Archive, rng: &mut Rng) -> Option<&'a Elite> {
    let mut idx = archive.occupied();
    if idx.is_empty() {
        return None;
    }
    let k = idx.len().min(TOURNAMENT);
    for i in 0..k {
        let j = i + rng.index(idx.len() - i);
        idx.swap(i, j);
    }
    let chosen = if rng.chance(CURIOSITY) {
        let mut best = idx[0];
        let mut best_s = f32::NEG_INFINITY;
        for &cell in &idx[..k] {
            let Some(elite) = archive.elite(cell) else {
                continue;
            };
            let score = curiosity(archive, elite);
            if score > best_s {
                best_s = score;
                best = cell;
            }
        }
        best
    } else {
        idx[rng.index(k)]
    };
    archive.elite(chosen)
}

/// Random until the archive holds [`WARMUP_ELITES`], then a bandit over Gaussian and IsoLine.
pub struct Scheduler {
    random: Random,
    evolvers: Vec<Box<dyn Emitter>>,
    windows: Vec<VecDeque<bool>>,
}

impl Scheduler {
    pub fn new(spec: NetSpec, sigma: f32) -> Self {
        let evolvers: Vec<Box<dyn Emitter>> = vec![
            Box::new(Gaussian::new(spec.clone(), sigma)),
            Box::new(IsoLine::new(spec.clone())),
        ];
        let windows = evolvers.iter().map(|_| VecDeque::new()).collect();
        Self {
            random: Random::new(spec),
            evolvers,
            windows,
        }
    }

    pub fn ask(&mut self, archive: &Archive, rng: &mut Rng) -> Option<Genome> {
        if archive.elite_count() < WARMUP_ELITES {
            return self.random.ask(archive, rng);
        }
        let i = self.pick(rng);
        if let Some(g) = self.evolvers[i].ask(archive, rng) {
            return Some(g);
        }
        for evolver in &mut self.evolvers {
            if let Some(g) = evolver.ask(archive, rng) {
                return Some(g);
            }
        }
        self.random.ask(archive, rng)
    }

    /// Tell the emitter that produced `eval`. Re-evaluations and the first half of a challenge
    /// do not move the bandit; the challenge's second result does.
    pub fn tell(&mut self, eval: &Evaluation, outcome: Settle) {
        let id = eval.candidate.emitter;
        if id == ID_RANDOM {
            self.random.tell(eval, outcome);
        } else if let Some(em) = self.evolvers.iter_mut().find(|e| e.id() == id) {
            em.tell(eval, outcome);
        }
        if matches!(
            eval.candidate.kind,
            crate::archive::CandidateKind::Reevaluate(_)
        ) {
            return;
        }
        if matches!(outcome, Settle::Challenged | Settle::Discarded) {
            return;
        }
        self.record(id, outcome.bandit_success());
    }

    pub fn emitter_name(&self, id: u8) -> &'static str {
        name_of(id)
    }

    fn pick(&self, rng: &mut Rng) -> usize {
        let n = self.evolvers.len();
        if n == 0 || rng.chance(EPSILON) {
            return rng.index(n.max(1));
        }
        let mut best = 0usize;
        let mut best_r = -1.0f32;
        for i in 0..n {
            let r = self.rate(i);
            if r > best_r {
                best_r = r;
                best = i;
            }
        }
        best
    }

    fn rate(&self, i: usize) -> f32 {
        let w = &self.windows[i];
        if w.is_empty() {
            0.0
        } else {
            w.iter().filter(|s| **s).count() as f32 / w.len() as f32
        }
    }

    fn record(&mut self, id: u8, success: bool) {
        let Some(i) = self.evolvers.iter().position(|e| e.id() == id) else {
            return;
        };
        let w = &mut self.windows[i];
        w.push_back(success);
        if w.len() > WINDOW {
            w.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{Candidate, CandidateKind, Genome};
    use crate::experience::{Evaluation, Trajectory};
    use crate::facts::EndReason;
    use crate::nn::NetSpec;
    use crate::rules::{Axis, Verdict};

    fn spec() -> NetSpec {
        NetSpec {
            version: 1,
            obs_len: 2,
            actions: 2,
            memory: 1,
            hidden: vec![2],
            norm_version: 1,
        }
    }

    fn filled(n: u64) -> Archive {
        let axes = [Axis {
            name: "x",
            min: 0.0,
            max: 20.0,
            bins: 20,
        }];
        let mut archive = Archive::new("test", &axes, spec(), 1);
        let mut rng = Rng::from_seed(5);
        for i in 0..n {
            let params = nn::init(&spec(), &mut rng);
            let g = Genome {
                id: i + 1,
                parent: None,
                emitter: ID_RANDOM,
                params,
            };
            let settle = archive.offer_new(g, 1.0, vec![i as f32 + 0.5], vec![100 + i], 0.0, i);
            assert_eq!(settle, Settle::Inserted);
        }
        archive
    }

    fn evaluation(emitter: u8, _inserted: bool) -> Evaluation {
        Evaluation {
            candidate: Candidate {
                genome: Genome {
                    id: 1,
                    parent: None,
                    emitter,
                    params: vec![0.0],
                },
                emitter,
                kind: CandidateKind::New,
            },
            verdict: Verdict::default(),
            trajectory: Trajectory::empty(0, 2, 1),
            end_reason: EndReason::TimeLimit,
            decisions: 1,
            episode: 1,
            agent: "swarm01".to_string(),
        }
    }

    #[test]
    fn random_warms_up_then_gaussian_and_isoline_take_over() {
        let mut sched = Scheduler::new(spec(), 0.03);
        let mut rng = Rng::from_seed(8);
        let early = filled(15);
        let g = sched.ask(&early, &mut rng).unwrap();
        assert_eq!(g.emitter, ID_RANDOM);
        assert!(g.parent.is_none());
        assert_eq!(g.params.len(), nn::param_count(&spec()));

        let ready = filled(16);
        let mut saw_gauss = false;
        let mut saw_iso = false;
        for _ in 0..30 {
            let g = sched.ask(&ready, &mut rng).unwrap();
            assert_ne!(g.emitter, ID_RANDOM);
            assert!(g.parent.is_some());
            assert_eq!(g.params.len(), nn::param_count(&spec()));
            let parent = ready.occupied().iter().find_map(|&c| {
                let e = ready.elite(c).unwrap();
                (e.genome.id == g.parent.unwrap()).then_some(e.genome.params.clone())
            });
            let parent = parent.expect("parent id");
            assert_ne!(g.params, parent, "a mutation changes something");
            match g.emitter {
                ID_GAUSSIAN => saw_gauss = true,
                ID_ISOLINE => saw_iso = true,
                _ => {}
            }
        }
        assert!(saw_gauss || saw_iso);
    }

    #[test]
    fn the_bandit_keeps_a_floor_under_the_weaker_emitter() {
        let mut sched = Scheduler::new(spec(), 0.03);
        let archive = filled(16);
        let mut rng = Rng::from_seed(3);
        for _ in 0..100 {
            sched.tell(&evaluation(ID_GAUSSIAN, true), Settle::Inserted);
            sched.tell(&evaluation(ID_ISOLINE, false), Settle::Rejected);
        }
        let mut gauss = 0;
        let mut iso = 0;
        for _ in 0..300 {
            match sched.ask(&archive, &mut rng).unwrap().emitter {
                ID_GAUSSIAN => gauss += 1,
                ID_ISOLINE => iso += 1,
                other => panic!("unexpected emitter {other}"),
            }
        }
        assert!(iso >= 1, "epsilon floor, iso was {iso}");
        assert!(gauss > iso, "gaussian {gauss} should beat isoline {iso}");
    }

    #[test]
    fn curiosity_prefers_the_rare_signature() {
        let axes = [Axis {
            name: "x",
            min: 0.0,
            max: 4.0,
            bins: 4,
        }];
        let mut archive = Archive::new("test", &axes, spec(), 1);
        let params = vec![0.0; nn::param_count(&spec())];
        // Three elites share signature 2; one holds signature 1 alone.
        for (i, sig) in [1u64, 2, 2, 2].into_iter().enumerate() {
            let g = Genome {
                id: (i as u64) + 1,
                parent: None,
                emitter: 0,
                params: params.clone(),
            };
            assert_eq!(
                archive.offer_new(g, 1.0, vec![i as f32 + 0.1], vec![sig], 0.0, 1),
                Settle::Inserted
            );
        }
        assert_eq!(archive.signature_count(1), 1);
        assert_eq!(archive.signature_count(2), 3);
        let mut rng = Rng::from_seed(12);
        let mut rare = 0;
        for _ in 0..200 {
            let parent = select_parent(&archive, &mut rng).unwrap();
            if parent.genome.id == 1 {
                rare += 1;
            }
        }
        assert!(rare >= 120, "rare parent chosen {rare}/200");
    }
}
