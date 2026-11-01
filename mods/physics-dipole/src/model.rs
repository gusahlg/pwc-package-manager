// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
use pwc_mod_api::sim::{model::*, reactions::*};
use std::collections::BTreeMap;
const CAP: usize = 343;

/// A finite signed-occupation universe. Dynamical identities are never material BlockIds.
#[derive(Clone, Default)]
pub struct Dipole {
    sites: BTreeMap<Pos, i8>,
    turns: u64,
    operations: u64,
    dropped: u64,
}

fn shift(p: Pos, axis: usize, amount: i32) -> Option<Pos> {
    Some(match axis {
        0 => (p.0.checked_add(amount)?, p.1, p.2),
        1 => (p.0, p.1.checked_add(amount)?, p.2),
        _ => (p.0, p.1, p.2.checked_add(amount)?),
    })
}

/// The only local transition: a norm-preserving reflection along an affine-null vector.
fn reflect<const N: usize>(q: [i8; N], v: [i32; N]) -> [i8; N] {
    let dot = q.iter().zip(v).map(|(&q, v)| i32::from(q) * v).sum::<i32>();
    let norm = v.iter().map(|v| v * v).sum::<i32>();
    let numerator = -2 * dot;
    if numerator % norm != 0 {
        return q;
    }
    let amplitude = numerator / norm;
    let candidate = std::array::from_fn::<_, N, _>(|i| i32::from(q[i]) + amplitude * v[i]);
    if candidate.iter().any(|q| !(-1..=1).contains(q)) {
        return q;
    }
    candidate.map(|q| q as i8)
}

impl Dipole {
    fn seed(&mut self, p: Pos) -> bool {
        if [p.0, p.1, p.2]
            .into_iter()
            .any(|v| v < i32::MIN + 3 || v > i32::MAX - 3)
        {
            return false;
        }
        for x in -3..=3 {
            for y in -3..=3 {
                for z in -3..=3 {
                    self.sites.insert((p.0 + x, p.1 + y, p.2 + z), 0);
                }
            }
        }
        true
    }
    /// Add an explicit external unit charge. This source changes the closed invariants.
    /// A nonempty destination refuses the source; no occupation is overwritten or clipped.
    pub fn inject_charge(&mut self, p: Pos, charge: i8) -> bool {
        if ![-1, 1].contains(&charge) || (self.sites.is_empty() && !self.seed(p)) {
            self.dropped = self.dropped.saturating_add(1);
            return false;
        }
        if self.sites.get(&p) != Some(&0) {
            self.dropped = self.dropped.saturating_add(1);
            return false;
        }
        self.sites.insert(p, charge);
        true
    }
    fn inject_pair(&mut self, p: Pos, charge: i8) -> bool {
        if self.sites.is_empty() && !self.seed(p) {
            self.dropped = self.dropped.saturating_add(1);
            return false;
        }
        let Some(n) = shift(p, 0, 1) else {
            self.dropped = self.dropped.saturating_add(1);
            return false;
        };
        if self.sites.get(&p) != Some(&0) || self.sites.get(&n) != Some(&0) {
            self.dropped = self.dropped.saturating_add(1);
            return false;
        }
        self.sites.insert(p, charge);
        self.sites.insert(n, -charge);
        true
    }
    fn lines(&self, turn: u64) -> Vec<[Pos; 3]> {
        let axis = (turn % 3) as usize;
        let phase = ((turn / 3) % 3) as i32;
        let mut stencils = Vec::new();
        for &p in self.sites.keys() {
            let c = match axis {
                0 => p.0,
                1 => p.1,
                _ => p.2,
            };
            if c.rem_euclid(3) != phase {
                continue;
            }
            let (Some(a), Some(b)) = (shift(p, axis, 1), shift(p, axis, 2)) else {
                continue;
            };
            if self.sites.contains_key(&a) && self.sites.contains_key(&b) {
                stencils.push([p, a, b]);
            }
        }
        stencils
    }
    fn plaquettes(&self, turn: u64) -> Vec<[Pos; 4]> {
        let a = (turn % 3) as usize;
        let b = (a + 1) % 3;
        let phase = (turn / 3) % 4;
        let (pa, pb) = ((phase & 1) as i32, ((phase >> 1) & 1) as i32);
        let mut stencils = Vec::new();
        for &p in self.sites.keys() {
            let coords = [p.0, p.1, p.2];
            if coords[a].rem_euclid(2) != pa || coords[b].rem_euclid(2) != pb {
                continue;
            }
            let (Some(x), Some(y)) = (shift(p, a, 1), shift(p, b, 1)) else {
                continue;
            };
            let Some(xy) = shift(x, b, 1) else {
                continue;
            };
            if [x, y, xy].iter().all(|p| self.sites.contains_key(p)) {
                stencils.push([p, x, y, xy]);
            }
        }
        stencils
    }
    fn apply<const N: usize>(&mut self, stencils: &[[Pos; N]], vector: [i32; N]) -> u64 {
        let mut changed = 0;
        for positions in stencils {
            let before = positions.map(|p| self.sites[&p]);
            let after = reflect(before, vector);
            if after == before {
                continue;
            }
            for (&p, q) in positions.iter().zip(after) {
                self.sites.insert(p, q);
            }
            changed += 1;
        }
        changed
    }
    /// Return exact closed charge, Cartesian dipole moment, and quadratic norm.
    pub fn invariants(&self) -> (i64, [i64; 3], i64) {
        let mut charge = 0;
        let mut dipole = [0i64; 3];
        let mut norm = 0;
        for (&(x, y, z), &q) in &self.sites {
            let q = i64::from(q);
            charge += q;
            norm += q * q;
            for (axis, c) in [x, y, z].into_iter().enumerate() {
                dipole[axis] += i64::from(c) * q;
            }
        }
        (charge, dipole, norm)
    }
}

impl Model for Dipole {
    fn id(&self) -> &'static str {
        super::ID
    }
    fn wake_cell(&mut self, _p: Pos) {}
    /// External source selector, separate from the reflection law:
    /// +1/-1 prepares an oriented neutral pair; +2/-2 prepares one signed unit.
    fn perturb(&mut self, p: Pos, source: i64) {
        match source {
            1 => {
                self.inject_pair(p, 1);
            }
            -1 => {
                self.inject_pair(p, -1);
            }
            2 => {
                self.inject_charge(p, 1);
            }
            -2 => {
                self.inject_charge(p, -1);
            }
            _ => {
                self.dropped = self.dropped.saturating_add(1);
            }
        }
    }
    fn tick(&mut self, store: &mut dyn CellStore, budget: Budget, _out: &mut Vec<Mutation>) {
        if self.sites.is_empty() {
            return;
        }
        let Some(next) = self.turns.checked_add(1).filter(|&next| next != u64::MAX) else {
            self.dropped = self.dropped.saturating_add(1);
            return;
        };
        if self
            .sites
            .keys()
            .any(|&p| store.resolve(p) != p || store.block_at(p).is_none())
        {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        let lines = self.lines(self.turns);
        let plaquettes = self.plaquettes(self.turns);
        if lines.len() + plaquettes.len() > budget.contacts_per_turn {
            return;
        }
        let changes = self.apply(&lines, [1, -2, 1]) + self.apply(&plaquettes, [1, -1, -1, 1]);
        self.operations = self.operations.saturating_add(changes);
        self.turns = next;
    }
    fn sample(&self, p: Pos) -> Sample {
        let (charge, dipole, norm) = self.invariants();
        Sample {
            labels: ["occupation", "closed charge", "quadratic norm", "dipole x"],
            values: [
                i64::from(self.sites.get(&p).copied().unwrap_or(0)),
                charge,
                norm,
                dipole[0],
            ],
        }
    }
    fn stats(&self) -> Stats {
        Stats {
            turns: self.turns,
            operations: self.operations,
            dropped: self.dropped,
            pending: self.sites.len(),
        }
    }
    fn save(&self) -> String {
        let mut out = format!(
            "dipole1 {} {} {}\n",
            self.turns, self.operations, self.dropped
        );
        for (&(x, y, z), &q) in &self.sites {
            out.push_str(&format!("{x} {y} {z} {q}\n"));
        }
        out
    }
    fn load(&mut self, data: &str) -> Result<(), String> {
        let mut lines = data.lines();
        let h: Vec<_> = lines
            .next()
            .unwrap_or("")
            .split_whitespace()
            .take(5)
            .collect();
        if h.len() != 4 || h[0] != "dipole1" {
            return Err("unsupported dipole state".into());
        }
        let number = |s: &str| {
            s.parse::<u64>()
                .map_err(|_| "invalid dipole counter".to_owned())
        };
        let mut next = Self {
            turns: number(h[1])?,
            operations: number(h[2])?,
            dropped: number(h[3])?,
            ..Self::default()
        };
        if next.turns == u64::MAX {
            return Err("exhausted dipole clock".into());
        }
        for line in lines {
            if next.sites.len() >= CAP {
                return Err("dipole domain exceeds capacity".into());
            }
            let r: Vec<_> = line.split_whitespace().take(5).collect();
            if r.len() != 4 {
                return Err("invalid dipole row".into());
            }
            let coordinate = |s: &str| {
                s.parse::<i32>()
                    .map_err(|_| "invalid dipole coordinate".to_owned())
            };
            let q = r[3]
                .parse::<i8>()
                .map_err(|_| "invalid signed occupation".to_owned())?;
            if !(-1..=1).contains(&q) {
                return Err("occupation outside ternary bounds".into());
            }
            if next
                .sites
                .insert((coordinate(r[0])?, coordinate(r[1])?, coordinate(r[2])?), q)
                .is_some()
            {
                return Err("duplicate dipole position".into());
            }
        }
        let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
        for &(x, y, z) in next.sites.keys() {
            for (i, v) in [x, y, z].into_iter().enumerate() {
                lo[i] = lo[i].min(v);
                hi[i] = hi[i].max(v);
            }
        }
        if !next.sites.is_empty()
            && (next.sites.len() != CAP || (0..3).any(|i| i64::from(hi[i]) - i64::from(lo[i]) != 6))
        {
            return Err("dipole domain must be a complete seven-cell cube".into());
        }
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_charge_is_frozen_but_neutral_pairs_translate_with_all_invariants() {
        let mut isolated = Dipole::default();
        assert!(isolated.inject_charge((0, 0, 0), 1));
        let original = isolated.sites.clone();
        let inv = isolated.invariants();
        for turn in 0..36 {
            isolated.apply(&isolated.lines(turn), [1, -2, 1]);
            isolated.apply(&isolated.plaquettes(turn), [1, -1, -1, 1]);
            assert_eq!(isolated.invariants(), inv);
        }
        assert_eq!(
            isolated.sites, original,
            "norm-one charge cannot change its position"
        );
        let mut pair = Dipole::default();
        assert!(pair.inject_pair((0, 0, 0), 1));
        let inv = pair.invariants();
        let line = [[(0, 0, 0), (1, 0, 0), (2, 0, 0)]];
        assert_eq!(pair.apply(&line, [1, -2, 1]), 1);
        assert_eq!(pair.sites[&(1, 0, 0)], 1);
        assert_eq!(pair.sites[&(2, 0, 0)], -1);
        assert_eq!(pair.invariants(), inv);
        let transverse = [[(1, 0, 0), (2, 0, 0), (1, 1, 0), (2, 1, 0)]];
        assert_eq!(pair.apply(&transverse, [1, -1, -1, 1]), 1);
        assert_eq!(pair.sites[&(1, 1, 0)], 1);
        assert_eq!(pair.sites[&(2, 1, 0)], -1);
        assert_eq!(pair.invariants(), inv);
        pair.apply(&transverse, [1, -1, -1, 1]);
        pair.apply(&line, [1, -2, 1]);
        assert_eq!(pair.sites[&(0, 0, 0)], 1);
        assert_eq!(pair.sites[&(1, 0, 0)], -1);
        assert_eq!(pair.invariants(), inv);
        println!(
            "single charge frozen for36phases; neutral pair moved longitudinally and transversely, conserved (Q,D,norm)={inv:?}, then reversed"
        );
    }
}
