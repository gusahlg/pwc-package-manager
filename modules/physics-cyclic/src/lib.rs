// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Excitable phases transmit a front through stationary configurations under one contact rule.
mod common;
use common::*;
use pwc_module_api::material::fit_raw;
use pwc_module_api::sim::{model::*, reactions::*};
use std::collections::BTreeMap;
/// Stable package and dynamics identity.
pub const ID: &str = "pwc.physics-cyclic";
const TITLE: &str = "Excitable Phase";
const ACTION: &str = "physics.cyclic.input";
const INPUT: i64 = 1;
const PHASES: u8 = 6;
/// Register the optional provider, disabled by default.
pub fn register(r: &mut pwc_module_api::ModuleRegistrar) {
    common::register(r);
}
/// Native authority factory for explicit isolated experiments.
pub fn spec() -> ModelSpec {
    ModelSpec {
        id: ID,
        version: 1,
        create: || Box::new(Cyclic::default()),
    }
}
/// A resting, firing, and refractory scalar field in a fixed finite region.
#[derive(Default)]
pub struct Cyclic {
    origin: Option<Pos>,
    phases: BTreeMap<Pos, u8>,
    clock: Clock,
}
fn linked(store: &dyn CellStore, a: Pos, b: Pos) -> bool {
    let (Some(a), Some(b)) = (store.block_at(a), store.block_at(b)) else {
        return false;
    };
    let ra = store.registry().configuration(a);
    let rb = store.registry().configuration(b);
    // Empty sums are zero, so the same inequality also describes vacuum contacts.
    ra.elements()
        .iter()
        .flat_map(|&x| rb.elements().iter().map(move |&y| i64::from(fit_raw(x, y))))
        .sum::<i64>()
        >= 0
}
impl Model for Cyclic {
    fn id(&self) -> &'static str {
        ID
    }
    fn wake_cell(&mut self, _pos: Pos) {}
    fn perturb(&mut self, pos: Pos, amount: i64) {
        if amount != 1 {
            self.clock.refuse();
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
        let mut next = if self.origin.is_none() {
            domain.into_iter().map(|p| (p, 0)).collect()
        } else {
            self.phases.clone()
        };
        let Some(phase) = next.get_mut(&pos) else {
            self.clock.refuse();
            return;
        };
        if *phase != 0 {
            self.clock.refuse();
            return;
        }
        *phase = 1;
        self.origin = Some(origin);
        self.phases = next;
    }
    fn tick(&mut self, store: &mut dyn CellStore, budget: Budget, _out: &mut Vec<Mutation>) {
        let Some(origin) = self.origin else {
            return;
        };
        if !self.phases.values().any(|&v| v != 0) {
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
        let mut next = self.phases.clone();
        let mut changed = 0;
        for &pos in &domain {
            let old = self.phases[&pos];
            let new = if old != 0 {
                (old + 1) % PHASES
            } else {
                let fires = (0..3).any(|axis| {
                    [-1, 1].into_iter().any(|delta| {
                        offset(pos, axis, delta)
                            .filter(|&p| contains(origin, p))
                            .is_some_and(|p| {
                                self.phases.get(&p) == Some(&1) && linked(store, pos, p)
                            })
                    })
                });
                u8::from(fires)
            };
            *next.get_mut(&pos).unwrap() = new;
            if new != old {
                changed += 1;
            }
        }
        self.phases = next;
        self.clock.advance(changed);
    }
    fn stats(&self) -> Stats {
        Stats {
            turns: self.clock.turns,
            operations: self.clock.operations,
            dropped: self.clock.refused,
            pending: self.phases.values().filter(|&&v| v != 0).count(),
        }
    }
    fn sample(&self, pos: Pos) -> Sample {
        Sample {
            labels: ["phase", "firing sites", "refractory sites", "turns"],
            values: [
                i64::from(*self.phases.get(&pos).unwrap_or(&0)),
                self.phases.values().filter(|&&v| v == 1).count() as i64,
                self.phases.values().filter(|&&v| v > 1).count() as i64,
                self.clock.turns as i64,
            ],
        }
    }
    fn save(&self) -> String {
        let mut out = self.clock.header("cyclic1", self.origin);
        for (&(x, y, z), phase) in &self.phases {
            out.push_str(&format!("{x} {y} {z} {phase}\n"));
        }
        out
    }
    fn load(&mut self, data: &str) -> Result<(), String> {
        let mut lines = data.lines();
        let (clock, origin) = header(lines.next().unwrap_or(""), "cyclic1")?;
        let mut next = Self {
            clock,
            origin,
            ..Self::default()
        };
        for line in lines {
            if next.phases.len() >= SITES {
                return Err("phase domain exceeds capacity".into());
            }
            let v: Vec<_> = line.split_whitespace().collect();
            if v.len() != 4 {
                return Err("invalid phase row".into());
            }
            let p = (coord(v[0])?, coord(v[1])?, coord(v[2])?);
            let phase = v[3].parse::<u8>().map_err(|_| "invalid phase")?;
            if phase >= PHASES || !origin.is_some_and(|o| contains(o, p)) {
                return Err("phase outside laboratory bounds".into());
            }
            if next.phases.insert(p, phase).is_some() {
                return Err("duplicate phase position".into());
            }
        }
        if origin.is_some() && next.phases.len() != SITES {
            return Err("incomplete phase domain".into());
        }
        *self = next;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn front_refractory_memory_and_persistence() {
        let mut model = Cyclic::default();
        model.perturb((0, 0, 0), 1);
        let mut store = Air(pwc_module_api::block::BlockRegistry::with_builtins());
        model.tick(&mut store, Budget::DEFAULT, &mut Vec::new());
        assert_eq!(model.phases[&(0, 0, 0)], 2);
        assert_eq!(model.phases[&(1, 0, 0)], 1);
        let saved = model.save();
        let mut resumed = Cyclic::default();
        resumed.load(&saved).unwrap();
        for _ in 0..8 {
            model.tick(&mut store, Budget::DEFAULT, &mut Vec::new());
            resumed.tick(&mut store, Budget::DEFAULT, &mut Vec::new());
            assert_eq!(model.save(), resumed.save());
        }
        let before = resumed.save();
        assert!(resumed.load("cyclic1 0 0 0 0 0 0\n0 0 0 6\n").is_err());
        assert_eq!(before, resumed.save());
    }
}
