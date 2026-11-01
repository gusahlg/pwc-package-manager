// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
use pwc_mod_api::sim::{model::*, reactions::*};
use std::collections::BTreeMap;

const CAP: usize = 343;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Site {
    q: u16,
    p: u16,
    twist: [u16; 3],
}
#[derive(Default)]
pub(super) struct Symplectic {
    sites: BTreeMap<Pos, Site>,
    initialized: bool,
    turns: u64,
    operations: u64,
    dropped: u64,
}
fn neighbor(p: Pos, axis: usize) -> Option<Pos> {
    Some(match axis {
        0 => (p.0.checked_add(1)?, p.1, p.2),
        1 => (p.0, p.1.checked_add(1)?, p.2),
        _ => (p.0, p.1, p.2.checked_add(1)?),
    })
}
/// Odd periodic piecewise-linear analogue of a sine, with amplitude 16384.
fn triangle(q: u16) -> i32 {
    let x = i32::from(q);
    if x < 16384 {
        x
    } else if x < 49152 {
        32768 - x
    } else {
        x - 65536
    }
}
impl Symplectic {
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
    fn edges(&self) -> Vec<(Pos, Pos, usize)> {
        let mut edges = Vec::with_capacity(3 * self.sites.len());
        for &a in self.sites.keys() {
            for axis in 0..3 {
                if let Some(b) = neighbor(a, axis).filter(|b| self.sites.contains_key(b)) {
                    edges.push((a, b, axis));
                }
            }
        }
        edges
    }
    fn kick(&mut self, edges: &[(Pos, Pos, usize)], reverse: bool) {
        for i in 0..edges.len() {
            let (a, b, axis) = edges[if reverse { edges.len() - 1 - i } else { i }];
            let twist = self.sites[&a].twist[axis].wrapping_add(self.sites[&b].twist[axis]);
            let phase = self.sites[&b]
                .q
                .wrapping_sub(self.sites[&a].q)
                .wrapping_sub(twist);
            let f = (triangle(phase) / 4) as i16 as u16;
            let (pa, pb) = (self.sites[&a].p, self.sites[&b].p);
            self.sites.get_mut(&a).unwrap().p = if reverse {
                pa.wrapping_sub(f)
            } else {
                pa.wrapping_add(f)
            };
            self.sites.get_mut(&b).unwrap().p = if reverse {
                pb.wrapping_add(f)
            } else {
                pb.wrapping_sub(f)
            };
        }
    }
    fn drift(&mut self, reverse: bool) {
        for site in self.sites.values_mut() {
            site.q = if reverse {
                site.q.wrapping_sub(site.p)
            } else {
                site.q.wrapping_add(site.p)
            };
        }
    }
    fn advance(&mut self, reverse: bool) {
        let edges = self.edges();
        if reverse {
            self.drift(true);
            self.kick(&edges, true);
        } else {
            self.kick(&edges, false);
            self.drift(false);
        }
    }
    fn modular_momentum(&self) -> u16 {
        self.sites.values().fold(0u16, |a, s| a.wrapping_add(s.p))
    }
}
impl Model for Symplectic {
    fn id(&self) -> &'static str {
        super::ID
    }
    fn wake_cell(&mut self, _p: Pos) {}
    fn perturb(&mut self, p: Pos, amount: i64) {
        if self.sites.is_empty() && !self.seed(p) {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        let Some(site) = self.sites.get_mut(&p) else {
            self.dropped = self.dropped.saturating_add(1);
            return;
        };
        site.p = site.p.wrapping_add(amount.rem_euclid(65536) as u16);
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
                for axis in 0..3 {
                    let sum = store
                        .registry()
                        .configuration(id)
                        .elements()
                        .iter()
                        .map(|e| u32::from(e.0[axis]))
                        .sum::<u32>();
                    site.twist[axis] = ((sum * 64) % 65536) as u16;
                }
            }
            self.initialized = true;
        }
        self.advance(false);
        self.operations = self.operations.saturating_add(self.sites.len() as u64);
        self.turns = next;
    }
    fn sample(&self, p: Pos) -> Sample {
        let site = self.sites.get(&p).copied().unwrap_or_default();
        Sample {
            labels: [
                "phase",
                "modular momentum",
                "total momentum mod 65536",
                "domain cells",
            ],
            values: [
                i64::from(site.q),
                i64::from(site.p),
                i64::from(self.modular_momentum()),
                self.sites.len() as i64,
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
            "torus1 {} {} {} {}\n",
            self.turns,
            self.operations,
            self.dropped,
            u8::from(self.initialized)
        );
        for (&(x, y, z), s) in &self.sites {
            out.push_str(&format!(
                "{x} {y} {z} {} {} {} {} {}\n",
                s.q, s.p, s.twist[0], s.twist[1], s.twist[2]
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
            .take(6)
            .collect();
        if h.len() != 5 || h[0] != "torus1" {
            return Err("unsupported torus state".into());
        }
        let counter = |s: &str| {
            s.parse::<u64>()
                .map_err(|_| "invalid torus counter".to_owned())
        };
        let initialized = match h[4] {
            "0" => false,
            "1" => true,
            _ => return Err("invalid torus initialization flag".into()),
        };
        let mut next = Self {
            turns: counter(h[1])?,
            operations: counter(h[2])?,
            dropped: counter(h[3])?,
            initialized,
            ..Self::default()
        };
        if next.turns == u64::MAX {
            return Err("exhausted torus clock".into());
        }
        for line in lines {
            if next.sites.len() >= CAP {
                return Err("torus domain exceeds capacity".into());
            }
            let r: Vec<_> = line.split_whitespace().take(9).collect();
            if r.len() != 8 {
                return Err("invalid torus row".into());
            }
            let pos = |s: &str| {
                s.parse::<i32>()
                    .map_err(|_| "invalid torus position".to_owned())
            };
            let value = |s: &str| {
                s.parse::<u16>()
                    .map_err(|_| "invalid torus phase".to_owned())
            };
            let site = Site {
                q: value(r[3])?,
                p: value(r[4])?,
                twist: [value(r[5])?, value(r[6])?, value(r[7])?],
            };
            if next
                .sites
                .insert((pos(r[0])?, pos(r[1])?, pos(r[2])?), site)
                .is_some()
            {
                return Err("duplicate torus position".into());
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
            return Err("torus domain must be a complete seven-cell cube".into());
        }
        if !next.initialized && next.sites.values().any(|s| s.twist != [0; 3]) {
            return Err("uninitialized torus twists".into());
        }
        if next.initialized && next.sites.is_empty() {
            return Err("initialized empty torus domain".into());
        }
        *self = next;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nonlinear_wrapping_trajectory_is_exactly_reversible() {
        let mut m = Symplectic::default();
        for x in 0..3 {
            m.sites.insert(
                (x, 0, 0),
                Site {
                    q: (x * 19000) as u16,
                    p: (x * 27001) as u16,
                    twist: [(x * 3001) as u16, 0, 0],
                },
            );
        }
        let initial = m.sites.clone();
        let momentum = m.modular_momentum();
        for _ in 0..32 {
            m.advance(false);
            assert_eq!(m.modular_momentum(), momentum);
        }
        assert_ne!(m.sites, initial);
        println!(
            "torus after32steps: {:?}, totalmomentum mod65536={momentum}",
            m.sites
        );
        for _ in 0..32 {
            m.advance(true);
        }
        assert_eq!(
            m.sites, initial,
            "including wrap crossings and frustrated nonlinear kicks"
        );
        assert_eq!(triangle(16384), 16384);
        assert_eq!(triangle(49152), -16384);
    }
}
