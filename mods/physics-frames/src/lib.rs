// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Commuting transformations rest; incompatible transformations rewrite their local frames.
use pwc_mod_api::prelude::*;
use pwc_mod_api::sim::{
    model::{Model, ModelSpec, Sample, Stats},
    reactions::{Budget, CellStore, Mutation, Pos},
};
use pwc_mod_api::{
    Action,
    engine::Key,
    input::intent::{Chord, Mods, Source},
};
use std::collections::BTreeMap;

const ID: &str = "pwc.physics-frames";
const RADIUS: i32 = 3;
const CELL_COUNT: usize = 343;

/// Register this independent native model; its enabled option defaults off.
pub fn register(r: &mut ModRegistrar) {
    let option = r.option(OptionSpec::toggle(
        "enabled",
        "Relational Frame Physics",
        Category::World,
        false,
    ));
    r.add(Provider {
        option,
        enabled: false,
    });
}
/// Stable native descriptor; version covers law, schedule, input and persisted state.
pub fn spec() -> ModelSpec {
    ModelSpec {
        id: ID,
        version: 1,
        create: || Box::new(Frames::default()),
    }
}
struct Provider {
    option: OptionId,
    enabled: bool,
}
impl Mod for Provider {
    fn id(&self) -> &'static str {
        ID
    }
    fn on_options(&mut self, o: &Options) {
        self.enabled = o.bool(self.option);
    }
    fn reaction_model(&self) -> Option<ModelSpec> {
        self.enabled.then_some(spec())
    }
    fn actions(&self) -> &[Action] {
        const A: &[Action] = &[
            Action {
                id: "physics.frames.conjugate",
                label: "Prepare a frame conflict",
                default: &[Chord::with(Mods::CTRL, Source::Key(Key::F5))],
                repeat: false,
                held: false,
                immediate: false,
            },
            Action {
                id: "physics.frames.cycle",
                label: "Cycle local laboratory frame",
                default: &[Chord::with(Mods::CTRL, Source::Key(Key::F6))],
                repeat: false,
                held: false,
                immediate: false,
            },
        ];
        A
    }
    fn update(&mut self, c: &mut ModContext) {
        if c.networked || c.world.simulation_model_id() != Some(ID) {
            return;
        }
        let p = c.world.simulation_cell(c.player.position);
        if c.action("physics.frames.conjugate") {
            c.world.perturb_simulation(p, 1);
        }
        if c.action("physics.frames.cycle") {
            c.world.perturb_simulation(p, 2);
        }
    }
    fn hud(&self, f: &HudFacts, w: &World, p: &Player, out: &mut Vec<HudElement>) {
        if w.simulation_model_id() != Some(ID) || !f.hud_mode.shows_mod_hud() {
            return;
        }
        let s = w.simulation_sample(w.simulation_cell(p.position));
        out.push(HudElement::Label {
            at: Anchor::BottomLeft,
            off: (12, -82),
            base_fs: 15,
            role: Role::Muted,
            text: format!(
                "Frame={} conflicts={} changes={} turns={} | Ctrl+F5 disturb; Ctrl+F6 cycle",
                s.values[0], s.values[1], s.values[2], s.values[3]
            )
            .into(),
        });
    }
}

/// A permutation of three abstract markers; products compose indices, without a table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Frame([u8; 3]);
const IDENTITY: Frame = Frame([0, 1, 2]);
const GROUND: Frame = Frame([1, 0, 2]);
const INPUT_OPERATOR: Frame = Frame([0, 2, 1]);
impl Frame {
    fn valid(self) -> bool {
        self.0.into_iter().all(|v| v < 3)
            && self.0[0] != self.0[1]
            && self.0[0] != self.0[2]
            && self.0[1] != self.0[2]
    }
    fn product(self, right: Self) -> Self {
        Self(right.0.map(|v| self.0[usize::from(v)]))
    }
    fn inverse(self) -> Self {
        let mut p = [0; 3];
        for (i, &v) in self.0.iter().enumerate() {
            p[usize::from(v)] = i as u8;
        }
        Self(p)
    }
    fn conjugated(self, by: Self) -> Self {
        by.inverse().product(self).product(by)
    }
    fn commutes(self, other: Self) -> bool {
        self.product(other) == other.product(self)
    }
    fn rank(self) -> usize {
        all_frames()
            .iter()
            .position(|&g| g == self)
            .expect("validated permutation")
    }
    #[cfg(test)]
    fn class(self) -> usize {
        self.0
            .into_iter()
            .enumerate()
            .filter(|&(i, v)| i == usize::from(v))
            .count()
    }
}
fn all_frames() -> Vec<Frame> {
    let mut out = Vec::with_capacity(6);
    for a in 0..3 {
        for b in 0..3 {
            for c in 0..3 {
                let g = Frame([a, b, c]);
                if g.valid() {
                    out.push(g);
                }
            }
        }
    }
    out
}
/// Eligibility is invariant under the encounter; therefore the commutation gate loses no state.
fn collide(a: Frame, b: Frame, reverse: bool) -> (Frame, Frame) {
    if a.commutes(b) {
        (a, b)
    } else if reverse {
        (a.product(b).product(a.inverse()), a)
    } else {
        (b, a.conjugated(b))
    }
}
fn safe(p: Pos) -> bool {
    [p.0, p.1, p.2]
        .into_iter()
        .all(|v| v.unsigned_abs() <= 2_000_000_000)
}
fn contains(o: Pos, p: Pos) -> bool {
    [
        i64::from(p.0) - i64::from(o.0),
        i64::from(p.1) - i64::from(o.1),
        i64::from(p.2) - i64::from(o.2),
    ]
    .into_iter()
    .all(|d| d.abs() <= i64::from(RADIUS))
}
fn nodes(o: Pos) -> Vec<Pos> {
    (o.0 - RADIUS..=o.0 + RADIUS)
        .flat_map(move |x| {
            (o.1 - RADIUS..=o.1 + RADIUS)
                .flat_map(move |y| (o.2 - RADIUS..=o.2 + RADIUS).map(move |z| (x, y, z)))
        })
        .collect()
}
fn neighbor(p: Pos, axis: usize) -> Pos {
    let mut a = [p.0, p.1, p.2];
    a[axis] += 1;
    (a[0], a[1], a[2])
}
fn pairs(o: Pos, phase: u8) -> Vec<(Pos, Pos)> {
    let axis = usize::from(phase / 2);
    let parity = i32::from(phase % 2);
    nodes(o)
        .into_iter()
        .filter_map(|p| {
            let rel = [p.0 - o.0, p.1 - o.1, p.2 - o.2][axis];
            (rel < RADIUS && (rel + RADIUS).rem_euclid(2) == parity).then(|| (p, neighbor(p, axis)))
        })
        .collect()
}
fn conflicts(cells: &BTreeMap<Pos, Frame>) -> usize {
    cells
        .iter()
        .map(|(&p, &a)| {
            (0..3)
                .filter(|&axis| {
                    cells
                        .get(&neighbor(p, axis))
                        .is_some_and(|&b| !a.commutes(b))
                })
                .count()
        })
        .sum()
}
#[derive(Default)]
struct Frames {
    origin: Option<Pos>,
    cells: BTreeMap<Pos, Frame>,
    active: usize,
    turns: u64,
    operations: u64,
    dropped: u64,
}
impl Frames {
    fn refuse(&mut self) {
        self.dropped = self.dropped.saturating_add(1);
    }
    fn phase(&mut self, phase: u8, reverse: bool) -> usize {
        let Some(o) = self.origin else {
            return 0;
        };
        let mut next = self.cells.clone();
        let mut changed = 0;
        for (p, q) in pairs(o, phase) {
            let (a, b) = (self.cells[&p], self.cells[&q]);
            let (na, nb) = collide(a, b, reverse);
            if (na, nb) != (a, b) {
                next.insert(p, na);
                next.insert(q, nb);
                changed += 1;
            }
        }
        self.cells = next;
        self.active = conflicts(&self.cells);
        changed
    }
}
impl Model for Frames {
    fn id(&self) -> &'static str {
        ID
    }
    fn wake_cell(&mut self, _: Pos) {}
    fn perturb(&mut self, p: Pos, kind: i64) {
        if !matches!(kind, 1 | 2 | -1) || !safe(p) {
            self.refuse();
            return;
        }
        let o = self.origin.unwrap_or(p);
        if !contains(o, p) {
            self.refuse();
            return;
        }
        let mut next = if self.origin.is_none() {
            nodes(o)
                .into_iter()
                .map(|p| (p, GROUND))
                .collect::<BTreeMap<_, _>>()
        } else {
            self.cells.clone()
        };
        let old = next[&p];
        let value = match kind {
            1 => old.conjugated(INPUT_OPERATOR),
            2 => all_frames()[(old.rank() + 1) % 6],
            _ => IDENTITY,
        };
        next.insert(p, value);
        self.active = conflicts(&next);
        self.origin = Some(o);
        self.cells = next;
    }
    fn tick(&mut self, s: &mut dyn CellStore, budget: Budget, _: &mut Vec<Mutation>) {
        if self.origin.is_none() || self.active == 0 || budget.contacts_per_turn < CELL_COUNT {
            return;
        }
        let Some(next) = self.turns.checked_add(1).filter(|&t| t < u64::MAX) else {
            self.refuse();
            return;
        };
        if self
            .cells
            .keys()
            .copied()
            .any(|p| s.resolve(p) != p || s.block_at(p).is_none())
        {
            self.refuse();
            return;
        }
        let changed = self.phase((self.turns % 6) as u8, false);
        self.operations = self.operations.saturating_add(changed as u64);
        self.turns = next;
    }
    fn stats(&self) -> Stats {
        Stats {
            turns: self.turns,
            operations: self.operations,
            dropped: self.dropped,
            pending: self.active,
        }
    }
    fn sample(&self, p: Pos) -> Sample {
        Sample {
            labels: ["frame rank", "noncommuting edges", "changed pairs", "turns"],
            values: [
                self.cells.get(&p).map_or(-1, |&g| g.rank() as i64),
                self.active as i64,
                self.operations.min(i64::MAX as u64) as i64,
                self.turns.min(i64::MAX as u64) as i64,
            ],
        }
    }
    fn save(&self) -> String {
        let mut text = format!(
            "frames1 {} {} {}",
            self.turns, self.operations, self.dropped
        );
        if let Some(o) = self.origin {
            text.push_str(&format!(" {} {} {}", o.0, o.1, o.2));
        }
        text.push('\n');
        for (&p, &g) in &self.cells {
            text.push_str(&format!(
                "{} {} {} {} {} {}\n",
                p.0, p.1, p.2, g.0[0], g.0[1], g.0[2]
            ));
        }
        text
    }
    fn load(&mut self, text: &str) -> Result<(), String> {
        let mut lines = text.lines();
        let h: Vec<_> = lines
            .next()
            .ok_or("missing frame header")?
            .split_whitespace()
            .collect();
        if !matches!(h.len(), 4 | 7) || h[0] != "frames1" {
            return Err("invalid frame header".into());
        }
        let count = |s: &str| {
            s.parse::<u64>()
                .map_err(|_| "invalid frame counter".to_string())
        };
        let coord = |s: &str| {
            s.parse::<i32>()
                .map_err(|_| "invalid frame position".to_string())
        };
        let mut next = Self {
            turns: count(h[1])?,
            operations: count(h[2])?,
            dropped: count(h[3])?,
            ..Default::default()
        };
        if next.turns == u64::MAX {
            return Err("exhausted frame clock".into());
        }
        if h.len() == 7 {
            let o = (coord(h[4])?, coord(h[5])?, coord(h[6])?);
            if !safe(o) {
                return Err("frame domain out of range".into());
            }
            next.origin = Some(o);
        }
        for line in lines {
            let v: Vec<_> = line.split_whitespace().collect();
            if v.len() != 6 || next.cells.len() == CELL_COUNT {
                return Err("invalid/oversized frame state".into());
            }
            let Some(o) = next.origin else {
                return Err("frame state without a domain".into());
            };
            let p = (coord(v[0])?, coord(v[1])?, coord(v[2])?);
            if !contains(o, p) {
                return Err("frame cell outside domain".into());
            }
            let byte = |s: &str| {
                s.parse::<u8>()
                    .map_err(|_| "invalid frame marker".to_string())
            };
            let g = Frame([byte(v[3])?, byte(v[4])?, byte(v[5])?]);
            if !g.valid() || next.cells.insert(p, g).is_some() {
                return Err("non-permutation or duplicate frame".into());
            }
        }
        if next.origin.is_some() && next.cells.len() != CELL_COUNT {
            return Err("incomplete frame domain".into());
        }
        next.active = conflicts(&next.cells);
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quiet_backgrounds_history_and_propagation_follow_the_same_small_rule() {
        for a in all_frames() {
            for b in all_frames() {
                let (c, d) = collide(a, b, false);
                assert_eq!(collide(c, d, true), (a, b));
                assert_eq!(a.product(b), c.product(d));
                let mut before = [a.class(), b.class()];
                let mut after = [c.class(), d.class()];
                before.sort();
                after.sort();
                assert_eq!(before, after);
                assert_eq!(a == IDENTITY, c == IDENTITY);
                assert_eq!(b == IDENTITY, d == IDENTITY);
            }
        }
        let step = |mut t: [Frame; 3], i: usize| {
            let (a, b) = collide(t[i], t[i + 1], false);
            t[i] = a;
            t[i + 1] = b;
            t
        };
        let mut failures = 0;
        for a in all_frames() {
            for b in all_frames() {
                for c in all_frames() {
                    if step(step(step([a, b, c], 0), 1), 0) != step(step(step([a, b, c], 1), 0), 1)
                    {
                        failures += 1;
                    }
                }
            }
        }
        assert_eq!(failures, 48, "quiescence deliberately breaks Yang-Baxter");
        let (a, b, c) = (GROUND, INPUT_OPERATOR, Frame([2, 1, 0]));
        assert_eq!(collide(a, a, false), (a, a));
        assert_eq!(collide(a, b, false), (b, c));
        assert_eq!(collide(b, c, false), (c, a));
        assert_eq!(collide(c, a, false), (a, b));
        assert_ne!(
            collide(b, c, false),
            (a, b),
            "two encounters restore carrier order but alter internal state"
        );
        let mut m = Frames::default();
        assert_eq!(m.sample((0, 0, 0)).values[0], -1);
        m.perturb((0, 0, 0), 1);
        let before = m.cells.clone();
        assert!(m.active > 0);
        for phase in 0..6 {
            m.phase(phase, false);
        }
        let touched = m
            .cells
            .iter()
            .filter(|(p, g)| before.get(*p) != Some(*g))
            .count();
        assert!(touched > 1);
        for phase in (0..6).rev() {
            m.phase(phase, true);
        }
        assert_eq!(m.cells, before);
        let saved = m.save();
        let mut resumed = Frames::default();
        resumed.load(&saved).unwrap();
        assert_eq!(resumed.save(), saved);
        assert!(resumed.load("frames1 0 0 0 0 0 0\n0 0 0 1 1 2\n").is_err());
        assert_eq!(resumed.save(), saved);
        println!(
            "36 inverse/product/class cases; 48/216 Yang-Baxter failures; three-encounter recurrence; disturbance changed {touched} sites and six phases reversed exactly"
        );
    }
}
