// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
use pwc_module_api::sim::{model::*, reactions::*};
use std::collections::BTreeMap;

const CAP: usize = 343;
const MAX_DEMON: i32 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Site {
    spin: i8,
    demon: i32,
    coupling: i8,
}
impl Default for Site {
    fn default() -> Self {
        Self {
            spin: 1,
            demon: 0,
            coupling: 1,
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct Spin {
    sites: BTreeMap<Pos, Site>,
    initialized: bool,
    turns: u64,
    operations: u64,
    dropped: u64,
}

fn neighbor(p: Pos, axis: usize, d: i32) -> Option<Pos> {
    Some(match axis {
        0 => (p.0.checked_add(d)?, p.1, p.2),
        1 => (p.0, p.1.checked_add(d)?, p.2),
        _ => (p.0, p.1, p.2.checked_add(d)?),
    })
}

impl Spin {
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
                    self.sites
                        .insert((p.0 + x, p.1 + y, p.2 + z), Site::default());
                }
            }
        }
        true
    }
    fn flip(&mut self, p: Pos) -> bool {
        let site = self.sites[&p];
        let mut field = 0i32;
        for axis in 0..3 {
            for d in [-1, 1] {
                if let Some(other) = neighbor(p, axis, d).and_then(|n| self.sites.get(&n)) {
                    field += i32::from(site.coupling.min(other.coupling)) * i32::from(other.spin);
                }
            }
        }
        let cost = 2 * i32::from(site.spin) * field;
        let demon = site.demon - cost;
        if !(0..=MAX_DEMON).contains(&demon) {
            return false;
        }
        self.sites.insert(
            p,
            Site {
                spin: -site.spin,
                demon,
                ..site
            },
        );
        true
    }
    fn exchange(&mut self, turn: u64) -> usize {
        let axis = (turn % 3) as usize;
        let parity = ((turn / 3) % 2) as i32;
        let positions: Vec<_> = self.sites.keys().copied().collect();
        let mut changed = 0;
        for p in positions {
            let c = match axis {
                0 => p.0,
                1 => p.1,
                _ => p.2,
            };
            if c.rem_euclid(2) != parity {
                continue;
            }
            let Some(n) = neighbor(p, axis, 1).filter(|n| self.sites.contains_key(n)) else {
                continue;
            };
            let (a, b) = (self.sites[&p].demon, self.sites[&n].demon);
            self.sites.get_mut(&p).unwrap().demon = b;
            self.sites.get_mut(&n).unwrap().demon = a;
            changed += usize::from(a != b);
        }
        changed
    }
    fn advance(&mut self, turn: u64, reverse: bool) -> usize {
        let mut positions: Vec<_> = self.sites.keys().copied().collect();
        let mut changed = 0;
        if reverse {
            changed += self.exchange(turn);
            positions.reverse();
        }
        for p in positions {
            changed += usize::from(self.flip(p));
        }
        if !reverse {
            changed += self.exchange(turn);
        }
        changed
    }
    fn energy(&self) -> i64 {
        let mut energy = self.sites.values().map(|s| i64::from(s.demon)).sum::<i64>();
        for (&p, a) in &self.sites {
            for axis in 0..3 {
                if let Some(b) = neighbor(p, axis, 1).and_then(|n| self.sites.get(&n)) {
                    energy -= i64::from(a.coupling.min(b.coupling))
                        * i64::from(a.spin)
                        * i64::from(b.spin);
                }
            }
        }
        energy
    }
}

impl Model for Spin {
    fn id(&self) -> &'static str {
        super::ID
    }
    fn wake_cell(&mut self, _p: Pos) {}
    fn perturb(&mut self, p: Pos, amount: i64) {
        if self.sites.is_empty() && !self.seed(p) {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        let Some(s) = self.sites.get_mut(&p) else {
            self.dropped = self.dropped.saturating_add(1);
            return;
        };
        let Some(energy) = i64::from(s.demon)
            .checked_add(amount)
            .filter(|&e| (0..=i64::from(MAX_DEMON)).contains(&e))
        else {
            self.dropped = self.dropped.saturating_add(1);
            return;
        };
        s.demon = energy as i32;
    }
    fn tick(&mut self, store: &mut dyn CellStore, budget: Budget, _out: &mut Vec<Mutation>) {
        if self.sites.is_empty() || self.sites.len() > budget.contacts_per_turn {
            return;
        }
        let Some(next) = self.turns.checked_add(1) else {
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
        if !self.initialized {
            for (&p, s) in &mut self.sites {
                let id = store.block_at(p).unwrap();
                s.coupling = (1 + store.registry().configuration(id).len() / 8) as i8;
            }
            self.initialized = true;
        }
        let changed = self.advance(self.turns, false);
        self.operations = self.operations.saturating_add(changed as u64);
        self.turns = next;
    }
    fn sample(&self, p: Pos) -> Sample {
        let s = self.sites.get(&p).copied().unwrap_or_default();
        Sample {
            labels: ["spin", "local energy", "magnetization", "closed energy"],
            values: [
                i64::from(s.spin),
                i64::from(s.demon),
                self.sites.values().map(|s| i64::from(s.spin)).sum(),
                self.energy(),
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
            "spin1 {} {} {} {}\n",
            self.turns,
            self.operations,
            self.dropped,
            u8::from(self.initialized)
        );
        for (&(x, y, z), s) in &self.sites {
            out.push_str(&format!(
                "{x} {y} {z} {} {} {}\n",
                s.spin, s.demon, s.coupling
            ));
        }
        out
    }
    fn load(&mut self, data: &str) -> Result<(), String> {
        let mut lines = data.lines();
        let header: Vec<_> = lines
            .next()
            .unwrap_or("")
            .split_whitespace()
            .take(6)
            .collect();
        if header.len() != 5 || header[0] != "spin1" {
            return Err("unsupported spin state".into());
        }
        let num = |s: &str| {
            s.parse::<u64>()
                .map_err(|_| "invalid spin counter".to_owned())
        };
        let initialized = match header[4] {
            "0" => false,
            "1" => true,
            _ => return Err("invalid spin initialization flag".into()),
        };
        let mut next = Self {
            turns: num(header[1])?,
            operations: num(header[2])?,
            dropped: num(header[3])?,
            initialized,
            ..Self::default()
        };
        if next.turns == u64::MAX {
            return Err("exhausted spin clock".into());
        }
        for line in lines {
            if next.sites.len() >= CAP {
                return Err("spin domain exceeds capacity".into());
            }
            let row: Vec<_> = line.split_whitespace().take(7).collect();
            if row.len() != 6 {
                return Err("invalid spin row".into());
            }
            let n = |s: &str| {
                s.parse::<i32>()
                    .map_err(|_| "invalid spin integer".to_owned())
            };
            let (spin, demon, coupling) = (n(row[3])?, n(row[4])?, n(row[5])?);
            if ![-1, 1].contains(&spin)
                || !(0..=MAX_DEMON).contains(&demon)
                || !(1..=5).contains(&coupling)
            {
                return Err("spin row outside numeric bounds".into());
            }
            if next
                .sites
                .insert(
                    (n(row[0])?, n(row[1])?, n(row[2])?),
                    Site {
                        spin: spin as i8,
                        demon,
                        coupling: coupling as i8,
                    },
                )
                .is_some()
            {
                return Err("duplicate spin position".into());
            }
        }
        validate_patch(next.sites.keys().copied())?;
        if !next.initialized && next.sites.values().any(|s| s.coupling != 1) {
            return Err("uninitialized spin couplings".into());
        }
        if next.initialized && next.sites.is_empty() {
            return Err("initialized empty spin domain".into());
        }
        *self = next;
        Ok(())
    }
}

fn validate_patch(positions: impl Iterator<Item = Pos>) -> Result<(), String> {
    let mut count = 0;
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for (x, y, z) in positions {
        count += 1;
        for (i, v) in [x, y, z].into_iter().enumerate() {
            lo[i] = lo[i].min(v);
            hi[i] = hi[i].max(v);
        }
    }
    if count == 0 {
        return Ok(());
    }
    if count != CAP || (0..3).any(|i| i64::from(hi[i]) - i64::from(lo[i]) != 6) {
        return Err("spin domain must be a complete seven-cell cube".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn energy_moves_between_order_and_demons_and_the_sweep_reverses() {
        let mut m = Spin::default();
        for x in 0..3 {
            m.sites.insert((x, 0, 0), Site::default());
        }
        m.sites.get_mut(&(0, 0, 0)).unwrap().demon = 64;
        let original = m.sites.clone();
        let energy = m.energy();
        for turn in 0..8 {
            m.advance(turn, false);
            assert_eq!(m.energy(), energy);
        }
        assert_ne!(
            m.sites, original,
            "the orientational state actually changes"
        );
        println!(
            "spin after eight sweeps: {:?}; conserved energy={energy}",
            m.sites
        );
        for turn in (0..8).rev() {
            m.advance(turn, true);
            assert_eq!(m.energy(), energy);
        }
        assert_eq!(m.sites, original);
    }
}
