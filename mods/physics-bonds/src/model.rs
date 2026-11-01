// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
use pwc_mod_api::sim::{model::*, reactions::*};
use std::collections::BTreeMap;
const CAP: usize = 343;
const MAX_STRAIN: i32 = 4096;
const MAX_HEAT: i64 = 1 << 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Site {
    u: i32,
    heat: i64,
    yield_at: i32,
    rest: [i32; 3],
}
impl Default for Site {
    fn default() -> Self {
        Self {
            u: 0,
            heat: 0,
            yield_at: 2,
            rest: [0; 3],
        }
    }
}
#[derive(Clone, Default)]
pub(super) struct Bonds {
    sites: BTreeMap<Pos, Site>,
    initialized: bool,
    work: i64,
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
impl Bonds {
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
    fn energy(&self) -> i64 {
        let mut total = self.sites.values().map(|s| s.heat).sum::<i64>();
        for (&p, a) in &self.sites {
            for axis in 0..3 {
                if let Some(b) = neighbor(p, axis, 1).and_then(|p| self.sites.get(&p)) {
                    let strain = i64::from(b.u - a.u - a.rest[axis]);
                    total += strain * strain;
                }
            }
        }
        total
    }
    #[cfg(test)]
    fn heat(&self) -> i64 {
        self.sites.values().map(|s| s.heat).sum()
    }
    fn deposit(&mut self, p: Pos, release: i64) -> Result<(), &'static str> {
        let next = self.sites[&p]
            .heat
            .checked_add(release)
            .filter(|&h| h <= MAX_HEAT)
            .ok_or("bond heat capacity")?;
        self.sites.get_mut(&p).unwrap().heat = next;
        Ok(())
    }
    fn advance(&mut self) -> Result<u64, &'static str> {
        let positions: Vec<_> = self.sites.keys().copied().collect();
        let mut changed = 0;
        // A displacement update changes all incident strains, thereby transporting stress.
        for &p in &positions {
            let mut force = 0i64;
            let mut degree = 0i64;
            for axis in 0..3 {
                if let Some(b) = neighbor(p, axis, 1).and_then(|n| self.sites.get(&n)) {
                    force += i64::from(b.u - self.sites[&p].u - self.sites[&p].rest[axis]);
                    degree += 1;
                }
                if let Some(a) = neighbor(p, axis, -1).and_then(|n| self.sites.get(&n)) {
                    force -= i64::from(self.sites[&p].u - a.u - a.rest[axis]);
                    degree += 1;
                }
            }
            let release = 2 * force.abs() - degree;
            if force == 0 || release <= 0 {
                continue;
            }
            let u = self.sites[&p].u + force.signum() as i32;
            if u.abs() > MAX_STRAIN {
                return Err("bond displacement bound");
            }
            self.deposit(p, release)?;
            self.sites.get_mut(&p).unwrap().u = u;
            changed += 1;
        }
        // Reststrain is a persistent geometrical memory, changed by the same yield law everywhere.
        for p in positions {
            for axis in 0..3 {
                let Some(n) = neighbor(p, axis, 1).filter(|p| self.sites.contains_key(p)) else {
                    continue;
                };
                let strain = self.sites[&n].u - self.sites[&p].u - self.sites[&p].rest[axis];
                let threshold = self.sites[&p].yield_at.min(self.sites[&n].yield_at);
                if strain.abs() <= threshold {
                    continue;
                }
                let rest = self.sites[&p].rest[axis] + strain.signum();
                if rest.abs() > MAX_STRAIN {
                    return Err("bond reststrain bound");
                }
                self.deposit(p, 2 * i64::from(strain.abs()) - 1)?;
                self.sites.get_mut(&p).unwrap().rest[axis] = rest;
                changed += 1;
            }
        }
        Ok(changed)
    }
}
impl Model for Bonds {
    fn id(&self) -> &'static str {
        super::ID
    }
    fn wake_cell(&mut self, _p: Pos) {}
    fn perturb(&mut self, p: Pos, amount: i64) {
        if self.sites.is_empty() && !self.seed(p) {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        let Some(site) = self.sites.get(&p) else {
            self.dropped = self.dropped.saturating_add(1);
            return;
        };
        let Some(u) = i64::from(site.u)
            .checked_add(amount)
            .filter(|&u| (-i64::from(MAX_STRAIN)..=i64::from(MAX_STRAIN)).contains(&u))
        else {
            self.dropped = self.dropped.saturating_add(1);
            return;
        };
        let before = self.energy();
        let previous = site.u;
        self.sites.get_mut(&p).unwrap().u = u as i32;
        let delta = self.energy() - before;
        if let Some(work) = self.work.checked_add(delta) {
            self.work = work;
        } else {
            self.sites.get_mut(&p).unwrap().u = previous;
            self.dropped = self.dropped.saturating_add(1);
        }
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
            for (&p, site) in &mut self.sites {
                let id = store.block_at(p).unwrap();
                site.yield_at = (2 + store.registry().configuration(id).len() / 4) as i32;
            }
            self.initialized = true;
        }
        let before = self.sites.clone();
        match self.advance() {
            Ok(changed) => {
                self.operations = self.operations.saturating_add(changed);
                self.turns = next;
            }
            Err(_) => {
                self.sites = before;
                self.dropped = self.dropped.saturating_add(1);
            }
        }
    }
    fn sample(&self, p: Pos) -> Sample {
        let site = self.sites.get(&p).copied().unwrap_or_default();
        Sample {
            labels: [
                "displacement",
                "local heat",
                "closed energy",
                "external work",
            ],
            values: [i64::from(site.u), site.heat, self.energy(), self.work],
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
            "bonds1 {} {} {} {} {}\n",
            self.turns,
            self.operations,
            self.dropped,
            u8::from(self.initialized),
            self.work
        );
        for (&(x, y, z), s) in &self.sites {
            out.push_str(&format!(
                "{x} {y} {z} {} {} {} {} {} {}\n",
                s.u, s.heat, s.yield_at, s.rest[0], s.rest[1], s.rest[2]
            ));
        }
        out
    }
    fn load(&mut self, data: &str) -> Result<(), String> {
        let mut lines = data.lines();
        let h: Vec<_> = lines
            .next()
            .unwrap_or("")
            .split_whitespace()
            .take(7)
            .collect();
        if h.len() != 6 || h[0] != "bonds1" {
            return Err("unsupported bond state".into());
        }
        let counter = |s: &str| {
            s.parse::<u64>()
                .map_err(|_| "invalid bond counter".to_owned())
        };
        let value = |s: &str| {
            s.parse::<i64>()
                .map_err(|_| "invalid bond integer".to_owned())
        };
        let initialized = match h[4] {
            "0" => false,
            "1" => true,
            _ => return Err("invalid bond initialization flag".into()),
        };
        let mut next = Self {
            turns: counter(h[1])?,
            operations: counter(h[2])?,
            dropped: counter(h[3])?,
            work: value(h[5])?,
            initialized,
            ..Self::default()
        };
        if next.turns == u64::MAX {
            return Err("exhausted bond clock".into());
        }
        for line in lines {
            if next.sites.len() >= CAP {
                return Err("bond domain exceeds capacity".into());
            }
            let r: Vec<_> = line.split_whitespace().take(10).collect();
            if r.len() != 9 {
                return Err("invalid bond row".into());
            }
            let coordinate = |s: &str| {
                s.parse::<i32>()
                    .map_err(|_| "invalid bond position".to_owned())
            };
            let displacement = |s: &str| {
                value(s).and_then(|n| {
                    if (-i64::from(MAX_STRAIN)..=i64::from(MAX_STRAIN)).contains(&n) {
                        Ok(n as i32)
                    } else {
                        Err("bond strain bound".into())
                    }
                })
            };
            let (heat, yield_at) = (value(r[4])?, value(r[5])?);
            if !(0..=MAX_HEAT).contains(&heat) || !(2..=10).contains(&yield_at) {
                return Err("bond row outside numeric bounds".into());
            }
            let site = Site {
                u: displacement(r[3])?,
                heat,
                yield_at: yield_at as i32,
                rest: [
                    displacement(r[6])?,
                    displacement(r[7])?,
                    displacement(r[8])?,
                ],
            };
            if next
                .sites
                .insert(
                    (coordinate(r[0])?, coordinate(r[1])?, coordinate(r[2])?),
                    site,
                )
                .is_some()
            {
                return Err("duplicate bond position".into());
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
            return Err("bond domain must be a complete seven-cell cube".into());
        }
        for (&p, site) in &next.sites {
            for axis in 0..3 {
                if neighbor(p, axis, 1).is_none_or(|p| !next.sites.contains_key(&p))
                    && site.rest[axis] != 0
                {
                    return Err("reststrain without an edge".into());
                }
            }
        }
        if !next.initialized && next.sites.values().any(|s| s.yield_at != 2) {
            return Err("uninitialized bond yields".into());
        }
        if next.initialized && next.sites.is_empty() {
            return Err("initialized empty bond domain".into());
        }
        if next.work != next.energy() {
            return Err("bond work and energy ledger disagree".into());
        }
        *self = next;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn displacement_transports_stress_and_plastic_memory_preserves_the_energy_ledger() {
        let mut m = Bonds::default();
        for x in 0..3 {
            m.sites.insert((x, 0, 0), Site::default());
        }
        m.perturb((0, 0, 0), 32);
        let energy = m.energy();
        let initial = m.sites.clone();
        for _ in 0..6 {
            m.advance().unwrap();
            assert_eq!(m.energy(), energy);
        }
        assert!(
            m.sites[&(1, 0, 0)].u != initial[&(1, 0, 0)].u,
            "shared nodes transmit strain"
        );
        assert!(
            m.sites.values().any(|s| s.rest != [0; 3]),
            "yielding leaves permanent bond memory"
        );
        assert!(m.heat() > 0);
        assert_eq!(m.work, energy);
        println!(
            "bond network after6steps: {:?}; conserved E={energy}, heat={}",
            m.sites,
            m.heat()
        );
    }
}
