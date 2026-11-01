// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Integer edge circulation evolves by reversible local plaquette shears, not named wires.
mod common;
use common::*;
use pwc_mod_api::sim::{model::*, reactions::*};
use std::collections::BTreeMap;
/// Stable package and dynamics identity.
pub const ID: &str = "pwc.physics-loops";
const TITLE: &str = "Circulation Loops";
const ACTION: &str = "physics.loops.input";
const INPUT: i64 = 16;
const LIMIT: i32 = 4096;
const MAX_EDGES: usize = 882;
/// Register the optional provider, disabled by default.
pub fn register(r: &mut pwc_mod_api::ModRegistrar) {
    common::register(r);
}
/// Native authority factory for explicit isolated experiments.
pub fn spec() -> ModelSpec {
    ModelSpec {
        id: ID,
        version: 1,
        create: || Box::new(Loops::default()),
    }
}
type Edge = (Pos, u8);
/// Sparse signed flow on positive-axis edges inside a fixed laboratory.
#[derive(Default)]
pub struct Loops {
    origin: Option<Pos>,
    edges: BTreeMap<Edge, i32>,
    clock: Clock,
}
fn boundary(p: Pos, a: u8, b: u8) -> Option<[Edge; 4]> {
    Some([(p, a), (offset(p, a, 1)?, b), (offset(p, b, 1)?, a), (p, b)])
}
fn faces(origin: Pos, phase: u8) -> Vec<[Edge; 4]> {
    let (a, b) = [(0, 1), (1, 2), (0, 2)][usize::from(phase / 4)];
    let color = phase % 4;
    let mut faces = Vec::new();
    let Some(domain) = cube(origin) else {
        return faces;
    };
    for p in domain {
        let coordinate = |axis| match axis {
            0 => p.0,
            1 => p.1,
            _ => p.2,
        };
        if coordinate(a).rem_euclid(2) != i32::from(color % 2)
            || coordinate(b).rem_euclid(2) != i32::from(color / 2)
        {
            continue;
        }
        let Some(face) = boundary(p, a, b) else {
            continue;
        };
        if face.iter().all(|&(lo, axis)| {
            contains(origin, lo) && offset(lo, axis, 1).is_some_and(|hi| contains(origin, hi))
        }) {
            faces.push(face);
        }
    }
    faces
}
fn divergence(edges: &BTreeMap<Edge, i32>) -> BTreeMap<Pos, i64> {
    let mut out = BTreeMap::new();
    for (&(lo, axis), &value) in edges {
        *out.entry(lo).or_default() += i64::from(value);
        if let Some(hi) = offset(lo, axis, 1) {
            *out.entry(hi).or_default() -= i64::from(value);
        }
    }
    out.retain(|_, value| *value != 0);
    out
}
impl Loops {
    fn read(&self, edge: Edge) -> i32 {
        *self.edges.get(&edge).unwrap_or(&0)
    }
    fn propose(&self, faces: &[[Edge; 4]]) -> Option<(BTreeMap<Edge, i32>, usize)> {
        let mut next = self.edges.clone();
        let mut changed = 0;
        for face in faces {
            let f = [
                i64::from(self.read(face[0])),
                i64::from(self.read(face[1])),
                -i64::from(self.read(face[2])),
                -i64::from(self.read(face[3])),
            ];
            let q = f.iter().sum::<i64>().div_euclid(4);
            if q == 0 {
                continue;
            }
            let delta = q.checked_mul(-2)?;
            for i in 0..4 {
                let value = f[i].checked_add(delta)?;
                let value = if i < 2 { value } else { value.checked_neg()? };
                let value = i32::try_from(value).ok()?;
                if i64::from(value).abs() > i64::from(LIMIT) {
                    return None;
                }
                if value == 0 {
                    next.remove(&face[i]);
                } else {
                    next.insert(face[i], value);
                }
            }
            changed += 1;
        }
        if next.len() > MAX_EDGES || !divergence(&next).is_empty() {
            return None;
        }
        Some((next, changed))
    }
}
impl Model for Loops {
    fn id(&self) -> &'static str {
        ID
    }
    fn wake_cell(&mut self, _pos: Pos) {}
    fn perturb(&mut self, pos: Pos, amount: i64) {
        let Ok(amount) = i32::try_from(amount) else {
            self.clock.refuse();
            return;
        };
        if amount == 0 {
            return;
        }
        let origin = self.origin.unwrap_or(pos);
        if cube(origin).is_none() {
            self.clock.refuse();
            return;
        }
        let Some(face) = boundary(pos, 0, 1) else {
            self.clock.refuse();
            return;
        };
        if !face.iter().all(|&(p, axis)| {
            contains(origin, p) && offset(p, axis, 1).is_some_and(|n| contains(origin, n))
        }) {
            self.clock.refuse();
            return;
        }
        let mut next = self.edges.clone();
        for (i, &edge) in face.iter().enumerate() {
            let delta = if i < 2 {
                i64::from(amount)
            } else {
                -i64::from(amount)
            };
            let Some(value) = i64::from(self.read(edge))
                .checked_add(delta)
                .and_then(|v| i32::try_from(v).ok())
            else {
                self.clock.refuse();
                return;
            };
            if i64::from(value).abs() > i64::from(LIMIT) {
                self.clock.refuse();
                return;
            }
            if value == 0 {
                next.remove(&edge);
            } else {
                next.insert(edge, value);
            }
        }
        if next.len() > MAX_EDGES || !divergence(&next).is_empty() {
            self.clock.refuse();
            return;
        }
        self.origin = Some(origin);
        self.edges = next;
    }
    fn tick(&mut self, store: &mut dyn CellStore, budget: Budget, _out: &mut Vec<Mutation>) {
        let Some(origin) = self.origin else {
            return;
        };
        if self.edges.is_empty() {
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
        let phase = (self.clock.turns % 12) as u8;
        let faces = faces(origin, phase);
        if faces.len() > budget.contacts_per_turn {
            return;
        }
        if !readable(store, &domain) {
            self.clock.refuse();
            return;
        }
        let Some((next, operations)) = self.propose(&faces) else {
            self.clock.refuse();
            return;
        };
        self.edges = next;
        self.clock.advance(operations);
    }
    fn stats(&self) -> Stats {
        Stats {
            turns: self.clock.turns,
            operations: self.clock.operations,
            dropped: self.clock.refused,
            pending: self.edges.len(),
        }
    }
    fn sample(&self, pos: Pos) -> Sample {
        Sample {
            labels: ["Jx", "Jy", "Jz", "occupied edges"],
            values: [
                i64::from(self.read((pos, 0))),
                i64::from(self.read((pos, 1))),
                i64::from(self.read((pos, 2))),
                self.edges.len() as i64,
            ],
        }
    }
    fn save(&self) -> String {
        let mut out = self.clock.header("loops1", self.origin);
        for (&((x, y, z), axis), value) in &self.edges {
            out.push_str(&format!("{x} {y} {z} {axis} {value}\n"));
        }
        out
    }
    fn load(&mut self, data: &str) -> Result<(), String> {
        let mut lines = data.lines();
        let (clock, origin) = header(lines.next().unwrap_or(""), "loops1")?;
        let mut next = Self {
            clock,
            origin,
            ..Self::default()
        };
        for line in lines {
            if next.edges.len() >= MAX_EDGES {
                return Err("loop edge capacity exceeded".into());
            }
            let v: Vec<_> = line.split_whitespace().collect();
            if v.len() != 5 {
                return Err("invalid loop row".into());
            }
            let p = (coord(v[0])?, coord(v[1])?, coord(v[2])?);
            let axis = v[3].parse::<u8>().map_err(|_| "invalid loop axis")?;
            let value = v[4]
                .parse::<i32>()
                .map_err(|_| "invalid edge circulation")?;
            if value == 0
                || i64::from(value).abs() > i64::from(LIMIT)
                || axis > 2
                || !origin.is_some_and(|o| {
                    contains(o, p) && offset(p, axis, 1).is_some_and(|n| contains(o, n))
                })
            {
                return Err("loop edge outside bounds".into());
            }
            if next.edges.insert((p, axis), value).is_some() {
                return Err("duplicate loop edge".into());
            }
        }
        if !divergence(&next.edges).is_empty() {
            return Err("edge field is not a closed circulation".into());
        }
        *self = next;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plaquette_phases_invert_and_persistence_keeps_closed_loops() {
        let mut model = Loops::default();
        model.perturb((0, 0, 0), 16);
        let original = model.edges.clone();
        let origin = model.origin.unwrap();
        for phase in 0..12 {
            let (next, _) = model.propose(&faces(origin, phase)).unwrap();
            model.edges = next;
            assert!(divergence(&model.edges).is_empty());
        }
        assert_ne!(model.edges, original, "neighboring loops interact");
        for phase in (0..12).rev() {
            let (next, _) = model.propose(&faces(origin, phase)).unwrap();
            model.edges = next;
        }
        assert_eq!(model.edges, original);
        let saved = model.save();
        let mut resumed = Loops::default();
        resumed.load(&saved).unwrap();
        assert_eq!(resumed.save(), saved);
        let before = resumed.save();
        assert!(resumed.load("loops1 0 0 0 0 0 0\n0 0 0 0 1\n").is_err());
        assert_eq!(resumed.save(), before);
    }
}
