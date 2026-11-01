// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! A finite chemical resource budget supports a catalytic front without manufacturing feedstock.
mod common;
use common::*;
use pwc_module_api::sim::{model::*, reactions::*};
use std::collections::BTreeMap;

/// Stable package and dynamics identity.
pub const ID: &str = "pwc.physics-fuel";
const TITLE: &str = "Finite Fuel";
const ACTION: &str = "physics.fuel.input";
const INPUT: i64 = 1;
const MAX_TOTAL: u64 = 1 << 20;
const INITIAL_FEED: u32 = 8;
/// Register the optional provider, disabled by default.
pub fn register(r: &mut pwc_module_api::ModuleRegistrar) {
    common::register(r);
}
/// Native authority factory for explicit isolated experiments.
pub fn spec() -> ModelSpec {
    ModelSpec {
        id: ID,
        version: 1,
        create: || Box::new(Fuel::default()),
    }
}

#[derive(Clone, Copy, Default)]
struct Cell {
    feed: u32,
    active: u32,
    spent: u32,
}
/// Finite laboratory populations, separate from material identity.
#[derive(Default)]
pub struct Fuel {
    origin: Option<Pos>,
    cells: BTreeMap<Pos, Cell>,
    clock: Clock,
}
impl Fuel {
    fn total(&self) -> u64 {
        self.cells
            .values()
            .map(|v| u64::from(v.feed) + u64::from(v.active) + u64::from(v.spent))
            .sum()
    }
    fn active(&self) -> u64 {
        self.cells.values().map(|v| u64::from(v.active)).sum()
    }
}
impl Model for Fuel {
    fn id(&self) -> &'static str {
        ID
    }
    fn wake_cell(&mut self, _pos: Pos) {}
    fn perturb(&mut self, pos: Pos, amount: i64) {
        let Ok(amount) = u32::try_from(amount) else {
            self.clock.refuse();
            return;
        };
        if amount == 0 {
            return;
        }
        let origin = self.origin.unwrap_or(pos);
        let Some(domain) = cube(origin) else {
            self.clock.refuse();
            return;
        };
        if !contains(origin, pos) {
            self.clock.refuse();
            return;
        }
        let mut cells = if self.origin.is_none() {
            domain
                .into_iter()
                .map(|p| {
                    (
                        p,
                        Cell {
                            feed: INITIAL_FEED,
                            ..Cell::default()
                        },
                    )
                })
                .collect()
        } else {
            self.cells.clone()
        };
        let before: u64 = cells
            .values()
            .map(|v| u64::from(v.feed) + u64::from(v.active) + u64::from(v.spent))
            .sum();
        if before + u64::from(amount) > MAX_TOTAL {
            self.clock.refuse();
            return;
        }
        let Some(v) = cells.get_mut(&pos) else {
            self.clock.refuse();
            return;
        };
        let Some(active) = v.active.checked_add(amount) else {
            self.clock.refuse();
            return;
        };
        v.active = active;
        self.origin = Some(origin);
        self.cells = cells;
    }
    fn tick(&mut self, store: &mut dyn CellStore, budget: Budget, _out: &mut Vec<Mutation>) {
        let Some(origin) = self.origin else {
            return;
        };
        if self.active() == 0 {
            return;
        }
        if self.clock.turns >= MAX_TURNS {
            self.clock.refuse();
            return;
        }
        let Some(domain) = cube(origin) else {
            self.clock.refuse();
            return;
        };
        if domain.len() > budget.contacts_per_turn {
            return;
        }
        if !readable(store, &domain) {
            self.clock.refuse();
            return;
        }
        let mut next = self.cells.clone();
        let axis = ((self.clock.turns / 2) % 3) as u8;
        let parity = (self.clock.turns % 2) as i32;
        let mut operations = 0;
        // A disjoint matching per turn makes transport independent of map traversal order.
        for &a in &domain {
            let coordinate = match axis {
                0 => a.0,
                1 => a.1,
                _ => a.2,
            };
            if coordinate.rem_euclid(2) != parity {
                continue;
            }
            let Some(b) = offset(a, axis, 1).filter(|&p| contains(origin, p)) else {
                continue;
            };
            let (aa, ab) = (self.cells[&a].active, self.cells[&b].active);
            let (from, to) = if aa > ab.saturating_add(1) {
                (a, b)
            } else if ab > aa.saturating_add(1) {
                (b, a)
            } else {
                continue;
            };
            next.get_mut(&from).unwrap().active -= 1;
            let receiver = next.get_mut(&to).unwrap();
            let Some(active) = receiver.active.checked_add(1) else {
                self.clock.refuse();
                return;
            };
            receiver.active = active;
            operations += 1;
        }
        for v in next.values_mut() {
            if v.active == 0 {
                continue;
            }
            if v.feed > 0 {
                let Some(active) = v.active.checked_add(1) else {
                    self.clock.refuse();
                    return;
                };
                v.feed -= 1;
                v.active = active;
            } else {
                let Some(spent) = v.spent.checked_add(1) else {
                    self.clock.refuse();
                    return;
                };
                v.active -= 1;
                v.spent = spent;
            }
            operations += 1;
        }
        let total: u64 = next
            .values()
            .map(|v| u64::from(v.feed) + u64::from(v.active) + u64::from(v.spent))
            .sum();
        if total != self.total() || total > MAX_TOTAL {
            self.clock.refuse();
            return;
        }
        self.cells = next;
        self.clock.advance(operations);
    }
    fn stats(&self) -> Stats {
        Stats {
            turns: self.clock.turns,
            operations: self.clock.operations,
            dropped: self.clock.refused,
            pending: self.cells.values().filter(|v| v.active > 0).count(),
        }
    }
    fn sample(&self, pos: Pos) -> Sample {
        let v = self.cells.get(&pos).copied().unwrap_or_default();
        Sample {
            labels: ["feed", "catalyst", "spent", "all tokens"],
            values: [
                i64::from(v.feed),
                i64::from(v.active),
                i64::from(v.spent),
                self.total() as i64,
            ],
        }
    }
    fn save(&self) -> String {
        let mut out = self.clock.header("fuel1", self.origin);
        for (&(x, y, z), v) in &self.cells {
            out.push_str(&format!(
                "{x} {y} {z} {} {} {}\n",
                v.feed, v.active, v.spent
            ));
        }
        out
    }
    fn load(&mut self, data: &str) -> Result<(), String> {
        let mut lines = data.lines();
        let (clock, origin) = header(lines.next().unwrap_or(""), "fuel1")?;
        let mut next = Self {
            clock,
            origin,
            ..Self::default()
        };
        for line in lines {
            if next.cells.len() >= SITES {
                return Err("fuel site capacity exceeded".into());
            }
            let v: Vec<_> = line.split_whitespace().collect();
            if v.len() != 6 {
                return Err("invalid fuel row".into());
            }
            let pos = (coord(v[0])?, coord(v[1])?, coord(v[2])?);
            if !origin.is_some_and(|o| contains(o, pos)) {
                return Err("fuel site outside anchor".into());
            }
            let value = |s: &str| {
                s.parse::<u32>()
                    .map_err(|_| "invalid fuel population".to_owned())
            };
            let cell = Cell {
                feed: value(v[3])?,
                active: value(v[4])?,
                spent: value(v[5])?,
            };
            if next.cells.insert(pos, cell).is_some() {
                return Err("duplicate fuel site".into());
            }
            if next.total() > MAX_TOTAL {
                return Err("fuel resource bound exceeded".into());
            }
        }
        if origin.is_some() && next.cells.len() != SITES {
            return Err("incomplete fuel domain".into());
        }
        *self = next;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_front_conserves_resources_and_restart() {
        let mut model = Fuel::default();
        model.perturb((0, 0, 0), 1);
        let total = model.total();
        let initial = model.save();
        let mut store = Air(pwc_module_api::block::BlockRegistry::with_builtins());
        for _ in 0..96 {
            model.tick(&mut store, Budget::DEFAULT, &mut Vec::new());
            assert_eq!(model.total(), total);
        }
        assert_ne!(model.save(), initial);
        assert!(model.cells.values().any(|v| v.spent > 0));
        let saved = model.save();
        let mut resumed = Fuel::default();
        resumed.load(&saved).unwrap();
        model.tick(&mut store, Budget::DEFAULT, &mut Vec::new());
        resumed.tick(&mut store, Budget::DEFAULT, &mut Vec::new());
        assert_eq!(model.save(), resumed.save());
        let before = resumed.save();
        assert!(resumed.load("fuel1 0 0 0 0 0 0\n0 0 0 1 1 1\n").is_err());
        assert_eq!(resumed.save(), before);
    }
}
