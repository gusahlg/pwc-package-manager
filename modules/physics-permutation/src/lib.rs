// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Whole configurations move under a six-phase reversible, composition-conditioned matching.
use pwc_module_api::material::Configuration;
use pwc_module_api::prelude::*;
use pwc_module_api::sim::{
    model::{Model, ModelSpec, Sample, Stats},
    reactions::{Budget, CellStore, Mutation, Pos},
};
use pwc_module_api::{
    Action,
    engine::Key,
    input::intent::{Chord, Mods, Source},
};
use std::collections::BTreeMap;

const ID: &str = "pwc.physics-permutation";
const R: i32 = 3;
/// Register this independent law; its enabled option defaults off.
pub fn register(r: &mut ModuleRegistrar) {
    let option = r.option(OptionSpec::toggle(
        "enabled",
        "Matter Permutations",
        Category::World,
        false,
    ));
    r.add(Provider {
        option,
        enabled: false,
    });
}
/// Native model descriptor. Version covers both update order and persisted payload.
pub fn spec() -> ModelSpec {
    ModelSpec {
        id: ID,
        version: 1,
        create: || Box::new(Permutation::default()),
    }
}
struct Provider {
    option: OptionId,
    enabled: bool,
}
impl Module for Provider {
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
        const A: &[Action] = &[Action {
            id: "physics.permutation.seed",
            label: "Start matter permutation laboratory",
            default: &[Chord::with(Mods::CTRL, Source::Key(Key::F5))],
            repeat: false,
            held: false,
            immediate: false,
        }];
        A
    }
    fn update(&mut self, c: &mut ModuleContext) {
        if !c.networked
            && c.world.simulation_model_id() == Some(ID)
            && c.action("physics.permutation.seed")
        {
            let p = c.world.simulation_cell(c.player.position);
            c.world.perturb_simulation(p, 1);
        }
    }
    fn hud(&self, f: &HudFacts, w: &World, _: &Player, out: &mut Vec<HudElement>) {
        if w.simulation_model_id() == Some(ID) && f.hud_mode.shows_module_hud() {
            let s = w.simulation_sample((0, 0, 0));
            out.push(HudElement::Label {
                at: Anchor::BottomLeft,
                off: (12, -82),
                base_fs: 15,
                role: Role::Muted,
                text: format!(
                    "Matter swaps={} phase={} cells={} refusals={} | Ctrl+F5 start",
                    s.values[0], s.values[1], s.values[2], s.values[3]
                )
                .into(),
            });
        }
    }
}
fn safe(p: Pos) -> bool {
    [p.0, p.1, p.2]
        .into_iter()
        .all(|v| v.unsigned_abs() <= 2_000_000_000)
}
fn nodes(c: Pos) -> Vec<Pos> {
    (c.0 - R..=c.0 + R)
        .flat_map(move |x| {
            (c.1 - R..=c.1 + R).flat_map(move |y| (c.2 - R..=c.2 + R).map(move |z| (x, y, z)))
        })
        .collect()
}
fn pairs(center: Pos, phase: u8) -> Vec<(Pos, Pos)> {
    let axis = usize::from(phase / 2);
    let parity = i32::from(phase % 2);
    nodes(center)
        .into_iter()
        .filter_map(|p| {
            let v = [p.0 - center.0, p.1 - center.1, p.2 - center.2];
            if (v[axis] + R) % 2 != parity || v[axis] == R {
                return None;
            }
            let mut n = [p.0, p.1, p.2];
            n[axis] += 1;
            Some((p, (n[0], n[1], n[2])))
        })
        .collect()
}
/// A symmetric nonlinear predicate: exchange is its own inverse because the predicate is
/// unchanged by exchange. Names, intern ids and presentation never enter it.
fn exchange(a: &Configuration, b: &Configuration) -> bool {
    let moment = |c: &Configuration| {
        c.elements()
            .iter()
            .flat_map(|e| e.0)
            .map(u32::from)
            .sum::<u32>()
    };
    ((moment(a) ^ moment(b)).count_ones() as usize + a.len() * b.len()) % 2 == 1
}
#[derive(Default)]
struct Permutation {
    center: Option<Pos>,
    turns: u64,
    operations: u64,
    dropped: u64,
}
impl Permutation {
    fn phase(
        &mut self,
        s: &mut dyn CellStore,
        phase: u8,
        budget: Budget,
        out: &mut Vec<Mutation>,
    ) -> bool {
        let Some(center) = self.center else {
            return false;
        };
        let matching = pairs(center, phase);
        if matching.len() > budget.contacts_per_turn {
            return false;
        }
        // Read/admit a complete geometric phase before its first cell mutation. Pair refusal
        // remains a separate storage issue: admitted individual swaps are still conservative.
        let mut incoming = BTreeMap::new();
        for p in nodes(center) {
            if s.resolve(p) != p {
                self.dropped = self.dropped.saturating_add(1);
                return false;
            }
            let Some(id) = s.block_at(p) else {
                self.dropped = self.dropped.saturating_add(1);
                return false;
            };
            incoming.insert(p, id);
        }
        for (p, q) in matching {
            let (a, b) = (incoming[&p], incoming[&q]);
            if a == b || !exchange(s.registry().configuration(a), s.registry().configuration(b)) {
                continue;
            }
            if !s.set_pair(p, a, b, q, b, a) {
                self.dropped = self.dropped.saturating_add(1);
                continue;
            }
            out.push(Mutation {
                pos: p,
                from: a,
                to: b,
            });
            out.push(Mutation {
                pos: q,
                from: b,
                to: a,
            });
            self.operations = self.operations.saturating_add(1);
        }
        true
    }
}
impl Model for Permutation {
    fn id(&self) -> &'static str {
        ID
    }
    fn wake_cell(&mut self, _: Pos) {}
    fn perturb(&mut self, p: Pos, amount: i64) {
        if amount == 0 || !safe(p) {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        if self.center.is_none() {
            self.center = Some(p);
        } // Activates existing matter; creates none.
    }
    fn tick(&mut self, s: &mut dyn CellStore, budget: Budget, out: &mut Vec<Mutation>) {
        let Some(next) = self.turns.checked_add(1) else {
            self.dropped = self.dropped.saturating_add(1);
            return;
        };
        if self.phase(s, (self.turns % 6) as u8, budget, out) {
            self.turns = next;
        }
    }
    fn stats(&self) -> Stats {
        Stats {
            turns: self.turns,
            operations: self.operations,
            dropped: self.dropped,
            pending: if self.center.is_some() { 343 } else { 0 },
        }
    }
    fn sample(&self, _: Pos) -> Sample {
        Sample {
            labels: ["swaps", "phase", "domain cells", "refusals"],
            values: [
                self.operations as i64,
                (self.turns % 6) as i64,
                if self.center.is_some() { 343 } else { 0 },
                self.dropped as i64,
            ],
        }
    }
    fn save(&self) -> String {
        let mut t = format!(
            "permutation1 {} {} {}\n",
            self.turns, self.operations, self.dropped
        );
        if let Some(p) = self.center {
            t.push_str(&format!("{} {} {}\n", p.0, p.1, p.2));
        }
        t
    }
    fn load(&mut self, t: &str) -> Result<(), String> {
        let mut lines = t.lines();
        let h: Vec<_> = lines
            .next()
            .ok_or("missing permutation header")?
            .split_whitespace()
            .collect();
        if h.len() != 4 || h[0] != "permutation1" {
            return Err("invalid permutation header".into());
        }
        let count = |s: &str| {
            s.parse::<u64>()
                .map_err(|_| "invalid permutation counter".to_string())
        };
        let mut n = Self {
            turns: count(h[1])?,
            operations: count(h[2])?,
            dropped: count(h[3])?,
            center: None,
        };
        if n.turns == u64::MAX {
            return Err("exhausted permutation clock".into());
        }
        if let Some(line) = lines.next() {
            let v: Vec<_> = line.split_whitespace().collect();
            if v.len() != 3 {
                return Err("invalid permutation center".into());
            }
            let coord = |s: &str| {
                s.parse::<i32>()
                    .map_err(|_| "invalid permutation position".to_string())
            };
            let p = (coord(v[0])?, coord(v[1])?, coord(v[2])?);
            if !safe(p) {
                return Err("permutation domain out of range".into());
            }
            n.center = Some(p);
        }
        if lines.next().is_some() {
            return Err("extra permutation state".into());
        }
        *self = n;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use pwc_module_api::{block::AIR, material::Element};
    struct Map {
        r: BlockRegistry,
        c: BTreeMap<Pos, BlockId>,
    }
    impl CellStore for Map {
        fn block_at(&self, p: Pos) -> Option<BlockId> {
            Some(self.c.get(&p).copied().unwrap_or(AIR))
        }
        fn set_block(&mut self, p: Pos, id: BlockId) -> Option<BlockId> {
            let old = self.block_at(p)?;
            if id == AIR {
                self.c.remove(&p);
            } else {
                self.c.insert(p, id);
            }
            Some(old)
        }
        fn registry(&self) -> &BlockRegistry {
            &self.r
        }
        fn registry_mut(&mut self) -> &mut BlockRegistry {
            &mut self.r
        }
        fn set_pair(
            &mut self,
            p: Pos,
            a: BlockId,
            na: BlockId,
            q: Pos,
            b: BlockId,
            nb: BlockId,
        ) -> bool {
            if p == q || self.block_at(p) != Some(a) || self.block_at(q) != Some(b) {
                return false;
            }
            self.set_block(p, na);
            self.set_block(q, nb);
            true
        }
    }
    #[test]
    fn all_six_phases_reverse_and_whole_matter_stays_conserved() {
        let mut map = Map {
            r: BlockRegistry::with_builtins(),
            c: BTreeMap::new(),
        };
        let id = map
            .r
            .intern(&Configuration::single(Element::new([1, 0, 0, 0])))
            .unwrap();
        map.c.insert((0, 0, 0), id);
        let initial = map.c.clone();
        let mut m = Permutation::default();
        m.perturb((0, 0, 0), 1);
        let mut out = Vec::new();
        for _ in 0..6 {
            m.tick(&mut map, Budget::DEFAULT, &mut out);
        }
        assert!(!out.is_empty());
        assert_eq!(map.c.len(), 1);
        for phase in (0..6).rev() {
            assert!(m.phase(&mut map, phase, Budget::DEFAULT, &mut out));
        }
        assert_eq!(map.c, initial);
        let t = m.save();
        let mut r = Permutation::default();
        r.load(&t).unwrap();
        assert_eq!(r.save(), t);
        println!(
            "six conditional disjoint matchings invert exactly and retain the same material occurrence"
        );
    }
}
