// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! MAP-Elites archive. A `Partition` maps a behaviour descriptor to a cell; `Grid` is the one
//! shipped now (edge values fall in the edge bins). Each cell holds at most one elite.
//!
//! A new candidate replaces nobody on its first look. It enters an empty cell, or, when it beats
//! the incumbent by a noise margin, waits for one more evaluation and replaces only if the mean of
//! those two still wins. Re-evaluating an elite updates its mean in place. Its cell stays the cell
//! of its first evaluation unless two re-evaluations in a row land in the same other cell.

use std::collections::BTreeMap;

use crate::nn::NetSpec;
use crate::rules::Axis;

/// One network and where it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct Genome {
    pub id: u64,
    pub parent: Option<u64>,
    pub emitter: u8,
    pub params: Vec<f32>,
}

/// Why the swarm asked for this genome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateKind {
    New,
    /// Re-evaluate the elite that lives in this cell.
    Reevaluate(usize),
    /// Second evaluation of a challenger aimed at this cell.
    Challenge(usize),
}

/// A genome handed to one agent for one episode.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub genome: Genome,
    pub emitter: u8,
    pub kind: CandidateKind,
}

impl Candidate {
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            CandidateKind::New => "new",
            CandidateKind::Reevaluate(_) => "reeval",
            CandidateKind::Challenge(_) => "challenge",
        }
    }
}

/// What the archive did with an evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Settle {
    Inserted,
    Rejected,
    Challenged,
    Replaced,
    /// Re-evaluation updated the mean; the elite stayed.
    Updated,
    /// Re-evaluation moved the elite into another cell.
    Moved,
    /// Two votes agreed on a new cell, but the elite lost the comparison and stayed.
    Stayed,
    /// The episode was cut short by the swarm or the network, not by the agent; nothing changed.
    Discarded,
}

impl Settle {
    pub fn as_str(self) -> &'static str {
        match self {
            Settle::Inserted => "inserted",
            Settle::Rejected => "rejected",
            Settle::Challenged => "challenged",
            Settle::Replaced => "replaced",
            Settle::Updated => "updated",
            Settle::Moved => "moved",
            Settle::Stayed => "stayed",
            Settle::Discarded => "discarded",
        }
    }

    /// An emitter's bandit counts a new elite or a replacement as a success.
    pub fn bandit_success(self) -> bool {
        matches!(self, Settle::Inserted | Settle::Replaced | Settle::Moved)
    }
}

/// Running totals per emitter id.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EmitterStats {
    pub insertions: u64,
    pub challenges: u64,
    pub replacements: u64,
    pub rejected: u64,
    pub reevals: u64,
}

/// The occupant of one cell.
#[derive(Clone, Debug)]
pub struct Elite {
    pub genome: Genome,
    pub fitness_mean: f32,
    pub fitness_m2: f32,
    pub evals: u32,
    /// Descriptor of the first evaluation. The cell can later disagree with it, after a move.
    pub descriptor: Vec<f32>,
    pub signatures: Vec<u64>,
    pub bonus: f32,
    pub born: u64,
    pub last_eval: u64,
    /// `(cell, votes)` when recent re-evaluations landed elsewhere.
    pub moved_votes: Option<(usize, u8)>,
}

impl Elite {
    fn fresh(
        genome: Genome,
        fitness: f32,
        descriptor: Vec<f32>,
        signatures: Vec<u64>,
        bonus: f32,
        episode: u64,
    ) -> Self {
        Self {
            genome,
            fitness_mean: fitness,
            fitness_m2: 0.0,
            evals: 1,
            descriptor,
            signatures,
            bonus,
            born: episode,
            last_eval: episode,
            moved_votes: None,
        }
    }

    pub fn std(&self) -> f32 {
        sample_std(self.fitness_m2, self.evals)
    }
}

/// A candidate that beat the margin once and is waiting for its second evaluation.
#[derive(Clone, Debug)]
pub struct Challenger {
    pub genome: Genome,
    pub cell: usize,
    pub first_fitness: f32,
    pub descriptor: Vec<f32>,
    pub signatures: Vec<u64>,
    pub bonus: f32,
    pub in_flight: bool,
}

/// Maps descriptors to cells. A CVT archive can replace `Grid` later.
pub trait Partition {
    fn cells(&self) -> usize;
    fn cell(&self, descriptor: &[f32]) -> usize;
    fn coords(&self, cell: usize) -> Vec<u16>;
}

/// Even bins over the rules' axes. Axis 0 is the fastest-moving coordinate.
#[derive(Clone, Debug)]
pub struct Grid {
    axes: Vec<Axis>,
    strides: Vec<usize>,
    n: usize,
}

impl Grid {
    pub fn new(axes: &[Axis]) -> Self {
        let mut strides = vec![1usize; axes.len()];
        let mut n = 1usize;
        for (i, ax) in axes.iter().enumerate() {
            strides[i] = n;
            let bins = ax.bins.max(1) as usize;
            n = n.saturating_mul(bins);
        }
        if axes.is_empty() {
            n = 1;
        }
        Self {
            axes: axes.to_vec(),
            strides,
            n: n.max(1),
        }
    }

    fn bin(ax: &Axis, v: f32) -> usize {
        let bins = ax.bins.max(1) as usize;
        if bins == 1 || !v.is_finite() || v <= ax.min {
            return 0;
        }
        if v >= ax.max {
            return bins - 1;
        }
        let span = ax.max - ax.min;
        if span <= 0.0 {
            return 0;
        }
        let scaled = (v - ax.min) / span * bins as f32;
        let mut b = scaled.floor() as usize;
        // An exact internal edge stays in the lower bin. The axis max is already the last bin.
        if b > 0 && scaled == b as f32 {
            b -= 1;
        }
        b.min(bins - 1)
    }
}

impl Partition for Grid {
    fn cells(&self) -> usize {
        self.n
    }

    fn cell(&self, descriptor: &[f32]) -> usize {
        if self.axes.is_empty() {
            return 0;
        }
        let mut idx = 0usize;
        for (i, ax) in self.axes.iter().enumerate() {
            let v = descriptor.get(i).copied().unwrap_or(ax.min);
            idx += Self::bin(ax, v) * self.strides[i];
        }
        idx.min(self.n - 1)
    }

    fn coords(&self, cell: usize) -> Vec<u16> {
        self.axes
            .iter()
            .enumerate()
            .map(|(i, ax)| {
                let bins = ax.bins.max(1) as usize;
                ((cell / self.strides[i]) % bins) as u16
            })
            .collect()
    }
}

/// The whole archive, including challengers waiting for a second look.
#[derive(Clone, Debug)]
pub struct Archive {
    pub rules: String,
    pub axes: Vec<Axis>,
    pub spec: NetSpec,
    pub grid: Grid,
    pub cells: Vec<Option<Elite>>,
    pub challengers: Vec<Challenger>,
    pub min_seen: Option<f32>,
    pub episodes: u64,
    pub next_genome_id: u64,
    pub run_seed: u64,
    pub stats: Vec<EmitterStats>,
    pub sig_index: BTreeMap<u64, u32>,
}

impl Archive {
    pub fn new(rules: &str, axes: &[Axis], spec: NetSpec, run_seed: u64) -> Self {
        let grid = Grid::new(axes);
        let n = grid.cells();
        Self {
            rules: rules.to_string(),
            axes: axes.to_vec(),
            spec,
            grid,
            cells: (0..n).map(|_| None).collect(),
            challengers: Vec::new(),
            min_seen: None,
            episodes: 0,
            next_genome_id: 0,
            run_seed,
            stats: vec![EmitterStats::default(); 3],
            sig_index: BTreeMap::new(),
        }
    }

    pub fn assign_id(&mut self, genome: &mut Genome) {
        if genome.id == 0 {
            self.next_genome_id += 1;
            genome.id = self.next_genome_id;
        } else if genome.id > self.next_genome_id {
            self.next_genome_id = genome.id;
        }
    }

    pub fn elite_count(&self) -> usize {
        self.cells.iter().filter(|c| c.is_some()).count()
    }

    pub fn elite(&self, cell: usize) -> Option<&Elite> {
        self.cells.get(cell).and_then(|c| c.as_ref())
    }

    pub fn occupied(&self) -> Vec<usize> {
        self.cells
            .iter()
            .enumerate()
            .filter_map(|(i, c)| c.as_ref().map(|_| i))
            .collect()
    }

    pub fn signature_count(&self, sig: u64) -> u32 {
        self.sig_index.get(&sig).copied().unwrap_or(0)
    }

    pub fn coverage(&self) -> f32 {
        if self.cells.is_empty() {
            0.0
        } else {
            self.elite_count() as f32 / self.cells.len() as f32
        }
    }

    /// Sum of `(mean − min fitness seen)`, so the score stays non-negative.
    pub fn qd_score(&self) -> f32 {
        let Some(min) = self.min_seen else { return 0.0 };
        self.cells
            .iter()
            .flatten()
            .map(|e| e.fitness_mean - min)
            .filter(|d| d.is_finite())
            .sum()
    }

    pub fn best(&self) -> Option<f32> {
        self.cells
            .iter()
            .flatten()
            .map(|e| e.fitness_mean)
            .max_by(|a, b| a.total_cmp(b))
    }

    /// Max fitness on the plane of the first two axes. Rows are axis 1, columns axis 0.
    pub fn projection(&self) -> Vec<Vec<Option<f32>>> {
        let x_bins = self
            .axes
            .first()
            .map(|a| a.bins.max(1) as usize)
            .unwrap_or(1);
        let y_bins = self
            .axes
            .get(1)
            .map(|a| a.bins.max(1) as usize)
            .unwrap_or(1);
        let mut grid = vec![vec![None; x_bins]; y_bins];
        for (i, slot) in self.cells.iter().enumerate() {
            let Some(e) = slot else { continue };
            let coords = self.grid.coords(i);
            let x = coords.first().copied().unwrap_or(0) as usize;
            let y = coords.get(1).copied().unwrap_or(0) as usize;
            if x < x_bins && y < y_bins {
                let cell = &mut grid[y][x];
                *cell = Some(match *cell {
                    Some(v) if v >= e.fitness_mean => v,
                    _ => e.fitness_mean,
                });
            }
        }
        grid
    }

    pub fn axes_compatible(&self, axes: &[Axis]) -> bool {
        self.axes.len() == axes.len()
            && self.axes.iter().zip(axes).all(|(a, b)| {
                a.name == b.name
                    && a.min.to_bits() == b.min.to_bits()
                    && a.max.to_bits() == b.max.to_bits()
                    && a.bins == b.bins
            })
    }

    pub fn rebuild_signatures(&mut self) {
        self.sig_index.clear();
        let sigs: Vec<Vec<u64>> = self
            .cells
            .iter()
            .filter_map(|c| c.as_ref().map(|e| e.signatures.clone()))
            .collect();
        for s in sigs {
            self.add_sigs(&s);
        }
    }

    pub fn offer_new(
        &mut self,
        genome: Genome,
        fitness: f32,
        descriptor: Vec<f32>,
        signatures: Vec<u64>,
        bonus: f32,
        episode: u64,
    ) -> Settle {
        self.note_fitness(fitness);
        if !fitness.is_finite() {
            self.bump(genome.emitter, Settle::Rejected);
            return Settle::Rejected;
        }
        let cell = self.grid.cell(&descriptor);
        let signatures = dedup(signatures);
        if self.elite(cell).is_none() {
            let emitter = genome.emitter;
            self.place_new(
                cell,
                Elite::fresh(genome, fitness, descriptor, signatures, bonus, episode),
            );
            self.bump(emitter, Settle::Inserted);
            return Settle::Inserted;
        }
        let incumbent = self.elite(cell).unwrap();
        let limit = incumbent.fitness_mean
            + margin(
                incumbent.fitness_mean,
                incumbent.fitness_m2,
                incumbent.evals,
            );
        if fitness > limit {
            let emitter = genome.emitter;
            self.challengers.push(Challenger {
                genome,
                cell,
                first_fitness: fitness,
                descriptor,
                signatures,
                bonus,
                in_flight: false,
            });
            self.bump(emitter, Settle::Challenged);
            Settle::Challenged
        } else {
            self.bump(genome.emitter, Settle::Rejected);
            Settle::Rejected
        }
    }

    /// Hand out the oldest challenger that is not already being evaluated.
    pub fn take_challenger(&mut self) -> Option<(Genome, usize)> {
        let pos = self.challengers.iter().position(|c| !c.in_flight)?;
        self.challengers[pos].in_flight = true;
        let c = &self.challengers[pos];
        Some((c.genome.clone(), c.cell))
    }

    /// Put a challenger whose evaluation was discarded back in line.
    pub fn release_challenger(&mut self, genome_id: u64) {
        if let Some(c) = self.challengers.iter_mut().find(|c| c.genome.id == genome_id) {
            c.in_flight = false;
        }
    }

    pub fn resolve_challenge(
        &mut self,
        genome_id: u64,
        fitness: f32,
        signatures: Vec<u64>,
        bonus: f32,
        episode: u64,
    ) -> Settle {
        self.note_fitness(fitness);
        let Some(pos) = self
            .challengers
            .iter()
            .position(|c| c.genome.id == genome_id)
        else {
            return Settle::Rejected;
        };
        let ch = self.challengers.remove(pos);
        if !fitness.is_finite() {
            self.bump(ch.genome.emitter, Settle::Rejected);
            return Settle::Rejected;
        }
        let mean = 0.5 * (ch.first_fitness + fitness);
        let m2 = (ch.first_fitness - mean).powi(2) + (fitness - mean).powi(2);
        let signatures = dedup(if signatures.is_empty() {
            ch.signatures
        } else {
            signatures
        });
        let cell = ch.cell;
        let emitter = ch.genome.emitter;
        let born = episode;
        let elite = Elite {
            genome: ch.genome,
            fitness_mean: mean,
            fitness_m2: m2,
            evals: 2,
            descriptor: ch.descriptor,
            signatures,
            bonus,
            born,
            last_eval: episode,
            moved_votes: None,
        };
        if cell >= self.cells.len() {
            self.bump(emitter, Settle::Rejected);
            return Settle::Rejected;
        }
        if self.cells[cell].is_none() {
            self.place_new(cell, elite);
            self.bump(emitter, Settle::Inserted);
            return Settle::Inserted;
        }
        let limit = {
            let inc = self.cells[cell].as_ref().unwrap();
            inc.fitness_mean + margin(inc.fitness_mean, inc.fitness_m2, inc.evals)
        };
        if mean > limit {
            self.remove_occupant(cell);
            self.place_new(cell, elite);
            self.bump(emitter, Settle::Replaced);
            Settle::Replaced
        } else {
            self.bump(emitter, Settle::Rejected);
            Settle::Rejected
        }
    }

    pub fn reevaluate(
        &mut self,
        cell: usize,
        fitness: f32,
        descriptor: &[f32],
        episode: u64,
    ) -> Settle {
        self.note_fitness(fitness);
        if !fitness.is_finite() || self.elite(cell).is_none() {
            return Settle::Rejected;
        }
        {
            let elite = self.cells[cell].as_mut().unwrap();
            welford(
                &mut elite.fitness_mean,
                &mut elite.fitness_m2,
                &mut elite.evals,
                fitness,
            );
            elite.last_eval = episode;
        }
        let new_cell = self.grid.cell(descriptor);
        if new_cell == cell {
            self.cells[cell].as_mut().unwrap().moved_votes = None;
            let emitter = self.cells[cell].as_ref().unwrap().genome.emitter;
            self.bump(emitter, Settle::Updated);
            return Settle::Updated;
        }
        let votes = self.cells[cell].as_ref().unwrap().moved_votes;
        if let Some((voted, n)) = votes
            && voted == new_cell
            && n >= 1
        {
            self.cells[cell].as_mut().unwrap().moved_votes = None;
            return self.try_move(cell, new_cell);
        }
        self.cells[cell].as_mut().unwrap().moved_votes = Some((new_cell, 1));
        let emitter = self.cells[cell].as_ref().unwrap().genome.emitter;
        self.bump(emitter, Settle::Updated);
        Settle::Updated
    }

    /// Prefer few evaluations, then a long time since the last one, then the lowest cell index.
    pub fn pick_reeval(&self) -> Option<usize> {
        let mut best: Option<(u32, u64, usize)> = None;
        for (i, slot) in self.cells.iter().enumerate() {
            let Some(e) = slot else { continue };
            let key = (e.evals, e.last_eval, i);
            if best.is_none_or(|b| key < b) {
                best = Some(key);
            }
        }
        best.map(|k| k.2)
    }

    fn try_move(&mut self, from: usize, to: usize) -> Settle {
        let Some(elite) = self.cells[from].take() else {
            return Settle::Rejected;
        };
        let emitter = elite.genome.emitter;
        let fitness = elite.fitness_mean;
        if to >= self.cells.len() {
            self.cells[from] = Some(elite);
            self.bump(emitter, Settle::Stayed);
            return Settle::Stayed;
        }
        if self.cells[to].is_none() {
            self.cells[to] = Some(elite);
            self.bump(emitter, Settle::Moved);
            return Settle::Moved;
        }
        let limit = {
            let inc = self.cells[to].as_ref().unwrap();
            inc.fitness_mean + margin(inc.fitness_mean, inc.fitness_m2, inc.evals)
        };
        if fitness > limit {
            self.remove_occupant(to);
            self.cells[to] = Some(elite);
            self.bump(emitter, Settle::Moved);
            if (emitter as usize) < self.stats.len() {
                self.stats[emitter as usize].replacements += 1;
            }
            Settle::Moved
        } else {
            self.cells[from] = Some(elite);
            self.bump(emitter, Settle::Stayed);
            Settle::Stayed
        }
    }

    fn place_new(&mut self, cell: usize, elite: Elite) {
        self.add_sigs(&elite.signatures);
        self.cells[cell] = Some(elite);
    }

    fn remove_occupant(&mut self, cell: usize) {
        if let Some(old) = self.cells[cell].take() {
            self.remove_sigs(&old.signatures);
        }
    }

    fn add_sigs(&mut self, sigs: &[u64]) {
        for &s in sigs {
            *self.sig_index.entry(s).or_insert(0) += 1;
        }
    }

    fn remove_sigs(&mut self, sigs: &[u64]) {
        for &s in sigs {
            if let Some(c) = self.sig_index.get_mut(&s) {
                *c = c.saturating_sub(1);
                if *c == 0 {
                    self.sig_index.remove(&s);
                }
            }
        }
    }

    fn note_fitness(&mut self, fitness: f32) {
        if !fitness.is_finite() {
            return;
        }
        self.min_seen = Some(
            self.min_seen
                .map(|m| if fitness < m { fitness } else { m })
                .unwrap_or(fitness),
        );
    }

    fn bump(&mut self, emitter: u8, kind: Settle) {
        let id = emitter as usize;
        if self.stats.len() <= id {
            self.stats.resize(id + 1, EmitterStats::default());
        }
        let s = &mut self.stats[id];
        match kind {
            Settle::Inserted => s.insertions += 1,
            Settle::Challenged => s.challenges += 1,
            Settle::Replaced => s.replacements += 1,
            Settle::Rejected => s.rejected += 1,
            Settle::Updated | Settle::Moved | Settle::Stayed => s.reevals += 1,
            Settle::Discarded => {}
        }
    }
}

fn dedup(mut sigs: Vec<u64>) -> Vec<u64> {
    sigs.sort_unstable();
    sigs.dedup();
    sigs
}

fn sample_std(m2: f32, evals: u32) -> f32 {
    if evals < 2 {
        0.0
    } else {
        (m2 / (evals as f32 - 1.0)).max(0.0).sqrt()
    }
}

fn margin(mean: f32, m2: f32, evals: u32) -> f32 {
    let std = sample_std(m2, evals);
    (0.05 * mean.abs()).max(0.5 * std)
}

fn welford(mean: &mut f32, m2: &mut f32, evals: &mut u32, x: f32) {
    *evals += 1;
    let n = *evals as f32;
    let delta = x - *mean;
    *mean += delta / n;
    let delta2 = x - *mean;
    *m2 += delta * delta2;
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn archive() -> Archive {
        let axes = [Axis {
            name: "x",
            min: 0.0,
            max: 4.0,
            bins: 4,
        }];
        Archive::new("test", &axes, spec(), 1)
    }

    fn genome(id: u64, emitter: u8) -> Genome {
        Genome {
            id,
            parent: None,
            emitter,
            params: vec![0.1, 0.2],
        }
    }

    #[test]
    fn grid_clamps_into_the_edge_bins() {
        let axes = [
            Axis {
                name: "x",
                min: 0.0,
                max: 1.0,
                bins: 4,
            },
            Axis {
                name: "y",
                min: -1.0,
                max: 1.0,
                bins: 2,
            },
        ];
        let grid = Grid::new(&axes);
        assert_eq!(grid.cells(), 8);
        assert_eq!(grid.cell(&[-10.0, 0.0]), 0);
        assert_eq!(grid.cell(&[0.0, -1.0]), 0);
        assert_eq!(grid.cell(&[1.0, 1.0]), grid.cells() - 1);
        assert_eq!(grid.cell(&[2.0, 5.0]), grid.cells() - 1);
        let mid = grid.cell(&[0.5, 0.0]);
        let coords = grid.coords(mid);
        assert_eq!(coords.len(), 2);
        assert!(coords[0] <= 3 && coords[1] <= 1);
        assert_eq!(grid.cell(&[0.0, -0.1]), grid.cell(&[-1.0, -2.0]));
    }

    #[test]
    fn challenge_needs_two_evaluations_and_the_margin() {
        let mut a = archive();
        assert_eq!(
            a.offer_new(genome(1, 0), 1.0, vec![0.5], vec![7], 0.0, 1),
            Settle::Inserted
        );
        assert_eq!(a.elite(0).unwrap().std(), 0.0);
        // margin = 0.05 * |1| = 0.05. 1.04 does not clear it.
        assert_eq!(
            a.offer_new(genome(2, 1), 1.04, vec![0.5], vec![8], 0.0, 2),
            Settle::Rejected
        );
        assert_eq!(
            a.offer_new(genome(3, 1), 5.0, vec![0.5], vec![9], 0.2, 3),
            Settle::Challenged
        );
        assert_eq!(a.challengers.len(), 1);
        // Second look is weak: mean of 5 and 0 is 2.5, which still beats 1.05.
        assert_eq!(
            a.resolve_challenge(3, 0.0, vec![9], 0.2, 4),
            Settle::Replaced
        );
        let e = a.elite(0).unwrap();
        assert_eq!(e.genome.id, 3);
        assert!((e.fitness_mean - 2.5).abs() < 1e-5);
        assert_eq!(e.evals, 2);
        assert!(e.std() > 0.0);
        assert_eq!(a.signature_count(7), 0);
        assert_eq!(a.signature_count(9), 1);
        assert_eq!(a.stats[1].challenges, 1);
        assert_eq!(a.stats[1].replacements, 1);
    }

    #[test]
    fn a_second_evaluation_can_fail_the_margin() {
        let mut a = archive();
        a.offer_new(genome(1, 0), 10.0, vec![0.5], vec![], 0.0, 1);
        assert_eq!(
            a.offer_new(genome(2, 1), 10.6, vec![0.5], vec![], 0.0, 2),
            Settle::Challenged
        );
        assert_eq!(
            a.resolve_challenge(2, 0.0, vec![], 0.0, 3),
            Settle::Rejected
        );
        assert_eq!(a.elite(0).unwrap().genome.id, 1);
        assert!((a.elite(0).unwrap().fitness_mean - 10.0).abs() < 1e-5);
    }

    #[test]
    fn reevaluation_updates_mean_and_a_move_needs_two_votes() {
        let mut a = archive();
        a.offer_new(genome(1, 0), 1.0, vec![0.5], vec![4], 0.0, 1);
        a.offer_new(genome(2, 0), 100.0, vec![2.5], vec![5], 0.0, 1);
        assert_eq!(a.reevaluate(0, 3.0, &[0.5], 2), Settle::Updated);
        let e = a.elite(0).unwrap();
        assert!((e.fitness_mean - 2.0).abs() < 1e-5);
        assert!((e.std() - 2.0_f32.sqrt()).abs() < 1e-4);
        assert_eq!(e.evals, 2);

        assert_eq!(
            a.reevaluate(0, 1.0, &[1.5], 3),
            Settle::Updated,
            "one vote does not move"
        );
        assert_eq!(a.elite(0).unwrap().genome.id, 1);
        assert!(a.elite(1).is_none());
        assert_eq!(
            a.reevaluate(0, 1.0, &[2.5], 4),
            Settle::Updated,
            "a different cell resets the vote"
        );
        assert_eq!(a.reevaluate(0, 1.0, &[1.5], 5), Settle::Updated);
        assert_eq!(a.reevaluate(0, 1.0, &[1.5], 6), Settle::Moved);
        assert!(a.elite(0).is_none());
        assert_eq!(a.elite(1).unwrap().genome.id, 1);
        assert_eq!(a.signature_count(4), 1);

        let from = 1;
        assert_eq!(a.reevaluate(from, 1.0, &[2.5], 7), Settle::Updated);
        assert_eq!(a.reevaluate(from, 1.0, &[2.5], 8), Settle::Stayed);
        assert_eq!(
            a.elite(1).unwrap().genome.id,
            1,
            "losing the new cell keeps the old one"
        );
        assert_eq!(a.elite(2).unwrap().genome.id, 2);
    }

    #[test]
    fn signature_index_tracks_insert_replace_and_drop() {
        let mut a = archive();
        a.offer_new(genome(1, 0), 1.0, vec![0.5], vec![7, 7, 8], 0.0, 1);
        a.offer_new(genome(2, 0), 1.0, vec![1.5], vec![7], 0.0, 1);
        assert_eq!(a.signature_count(7), 2);
        assert_eq!(a.signature_count(8), 1);
        assert_eq!(
            a.offer_new(genome(3, 1), 5.0, vec![1.5], vec![9], 0.0, 2),
            Settle::Challenged
        );
        assert_eq!(
            a.resolve_challenge(3, 5.0, vec![9], 0.0, 3),
            Settle::Replaced
        );
        assert_eq!(a.signature_count(7), 1);
        assert_eq!(a.signature_count(9), 1);
    }

    #[test]
    fn coverage_and_qd_score() {
        let mut a = archive();
        assert_eq!(a.coverage(), 0.0);
        assert_eq!(a.qd_score(), 0.0);
        a.offer_new(genome(1, 0), 2.0, vec![0.5], vec![], 0.0, 1);
        a.offer_new(genome(2, 0), 5.0, vec![1.5], vec![], 0.0, 1);
        assert!((a.coverage() - 0.5).abs() < 1e-5);
        assert!(
            (a.qd_score() - 3.0).abs() < 1e-4,
            "means 2 and 5, min seen 2, score (0 + 3)"
        );
        assert!((a.best().unwrap() - 5.0).abs() < 1e-5);
        let picked = a.pick_reeval().unwrap();
        assert!(picked == 0 || picked == 1);
    }
}
