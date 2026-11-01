// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: AGPL-3.0-or-later
//! A small, exactly invertible integer field laboratory. See ../README.md.
use std::collections::BTreeMap;

use pwc_module_api::Action;
use pwc_module_api::engine::Key;
use pwc_module_api::input::intent::{Chord, Mods, Source};
use pwc_module_api::prelude::*;
use pwc_module_api::sim::{model::*, reactions::*};

const ID: &str = "pwc.physics-wave";
const RADIUS: i32 = 3;
const MAX_CELLS: usize = 343;
const KICK_SCALE: i64 = 16;
const DRIFT_SCALE: i64 = 8;

pub fn register(registrar: &mut ModuleRegistrar) {
    let enabled = registrar.option(OptionSpec::toggle(
        "enabled",
        "Integer Wave Physics",
        Category::World,
        false,
    ));
    registrar.add(WaveModule { enabled, on: false });
}

pub fn spec() -> ModelSpec {
    ModelSpec {
        id: ID,
        version: 1,
        create: || Box::new(Wave::default()),
    }
}

pub struct WaveModule {
    enabled: OptionId,
    on: bool,
}

impl Module for WaveModule {
    fn id(&self) -> &'static str {
        ID
    }
    fn on_options(&mut self, options: &Options) {
        self.on = options.bool(self.enabled);
    }
    fn reaction_model(&self) -> Option<ModelSpec> {
        self.on.then_some(spec())
    }
    fn actions(&self) -> &[Action] {
        const ACTIONS: &[Action] = &[Action {
            id: "physics.wave.impulse",
            label: "Wave laboratory impulse",
            default: &[Chord::with(Mods::CTRL, Source::Key(Key::F6))],
            repeat: false,
            held: false,
            immediate: false,
        }];
        ACTIONS
    }
    fn update(&mut self, ctx: &mut ModuleContext) {
        if !ctx.networked
            && ctx.world.simulation_model_id() == Some(ID)
            && ctx.action("physics.wave.impulse")
        {
            let pos = ctx.world.simulation_cell(ctx.player.position);
            ctx.world.perturb_simulation(pos, 4096);
        }
    }
    fn hud(&self, facts: &HudFacts, world: &World, player: &Player, out: &mut Vec<HudElement>) {
        if world.simulation_model_id() != Some(ID) || !facts.hud_mode.shows_module_hud() {
            return;
        }
        let s = world.simulation_sample(world.simulation_cell(player.position));
        let stats = world.simulation_stats();
        out.push(HudElement::Label {
            at: Anchor::BottomLeft,
            off: (12, -82),
            base_fs: 15,
            role: Role::Muted,
            text: format!(
                "Wave q={} p={} cells={} steps={} | Ctrl+F6 impulse",
                s.values[0], s.values[1], s.values[2], stats.turns
            )
            .into(),
        });
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct State {
    q: i64,
    p: i64,
}

/// Only an explicitly activated finite patch stores dynamical state. No phase BlockIds.
#[derive(Default)]
pub struct Wave {
    cells: BTreeMap<Pos, State>,
    turns: u64,
    operations: u64,
    dropped: u64,
}

fn offset(p: Pos, axis: u8, delta: i32) -> Option<Pos> {
    let (x, y, z) = p;
    Some(match axis {
        0 => (x.checked_add(delta)?, y, z),
        1 => (x, y.checked_add(delta)?, z),
        _ => (x, y, z.checked_add(delta)?),
    })
}

/// Each edge applies equal and opposite kicks; callers restore the entire phase on overflow.
fn kick(cells: &mut BTreeMap<Pos, State>, edges: &[(Pos, Pos)], reverse: bool) -> bool {
    for step in 0..edges.len() {
        let (a, b) = edges[if reverse {
            edges.len() - step - 1
        } else {
            step
        }];
        let force =
            ((i128::from(cells[&b].q) - i128::from(cells[&a].q)) / i128::from(KICK_SCALE)) as i64;
        let force = if reverse { -force } else { force };
        let (Some(pa), Some(pb)) = (
            cells[&a].p.checked_add(force),
            cells[&b].p.checked_sub(force),
        ) else {
            return false;
        };
        cells.get_mut(&a).unwrap().p = pa;
        cells.get_mut(&b).unwrap().p = pb;
    }
    true
}

fn drift(cells: &mut BTreeMap<Pos, State>, mass: &BTreeMap<Pos, i64>, reverse: bool) -> bool {
    for (pos, state) in cells {
        let delta = state.p / (DRIFT_SCALE * mass[pos]);
        let next = if reverse {
            state.q.checked_sub(delta)
        } else {
            state.q.checked_add(delta)
        };
        let Some(q) = next else {
            return false;
        };
        state.q = q;
    }
    true
}

impl Model for Wave {
    fn id(&self) -> &'static str {
        ID
    }
    fn wake_cell(&mut self, _pos: Pos) {}
    fn perturb(&mut self, pos: Pos, amount: i64) {
        if self.cells.is_empty() {
            // Reflected boundaries are an explicit finite-laboratory rule, not a silent loss.
            for x in -RADIUS..=RADIUS {
                for y in -RADIUS..=RADIUS {
                    for z in -RADIUS..=RADIUS {
                        if let (Some(a), Some(b), Some(c)) = (
                            pos.0.checked_add(x),
                            pos.1.checked_add(y),
                            pos.2.checked_add(z),
                        ) {
                            self.cells.insert((a, b, c), State::default());
                        }
                    }
                }
            }
        }
        match self.cells.get_mut(&pos) {
            Some(s) => match s.p.checked_add(amount) {
                Some(p) => s.p = p,
                None => self.dropped = self.dropped.saturating_add(1),
            },
            None => self.dropped = self.dropped.saturating_add(1),
        }
    }
    fn tick(&mut self, store: &mut dyn CellStore, budget: Budget, _out: &mut Vec<Mutation>) {
        if self.cells.is_empty() || self.cells.len() > budget.contacts_per_turn {
            return;
        }
        if self
            .cells
            .keys()
            .any(|&p| store.resolve(p) != p || store.block_at(p).is_none())
        {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        // Snapshot geometry and mass before either shear; a complete phase is atomic.
        let mass: BTreeMap<_, _> = self
            .cells
            .keys()
            .map(|&p| {
                let n = store
                    .block_at(store.resolve(p))
                    .map(|id| store.registry().configuration(id).len())
                    .unwrap_or(0);
                (p, 1 + n as i64)
            })
            .collect();
        let mut edges = Vec::with_capacity(self.cells.len() * 3);
        for &p in self.cells.keys() {
            for axis in 0..3 {
                if let Some(n) = offset(p, axis, 1) {
                    if self.cells.contains_key(&n) {
                        edges.push((p, n));
                    }
                }
            }
        }
        let before = self.cells.clone();
        if !kick(&mut self.cells, &edges, false) || !drift(&mut self.cells, &mass, false) {
            self.cells = before;
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        self.turns = self.turns.saturating_add(1);
        self.operations = self.operations.saturating_add(self.cells.len() as u64);
    }
    fn sample(&self, pos: Pos) -> Sample {
        let s = self.cells.get(&pos).copied().unwrap_or_default();
        Sample {
            labels: ["displacement", "momentum", "domain cells", "momentum sum"],
            values: [
                s.q,
                s.p,
                self.cells.len() as i64,
                self.cells
                    .values()
                    .map(|s| i128::from(s.p))
                    .sum::<i128>()
                    .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64,
            ],
        }
    }
    fn stats(&self) -> Stats {
        Stats {
            turns: self.turns,
            operations: self.operations,
            dropped: self.dropped,
            pending: self.cells.len(),
        }
    }
    fn save(&self) -> String {
        let mut s = format!(
            "wave1 {} {} {}\n",
            self.turns, self.operations, self.dropped
        );
        for (&(x, y, z), v) in &self.cells {
            s.push_str(&format!("{x} {y} {z} {} {}\n", v.q, v.p));
        }
        s
    }
    fn load(&mut self, data: &str) -> Result<(), String> {
        let mut lines = data.lines();
        let h: Vec<_> = lines.next().unwrap_or("").split_whitespace().collect();
        if h.len() != 4 || h[0] != "wave1" {
            return Err("unsupported wave state".into());
        }
        let parse = |s: &str| s.parse::<u64>().map_err(|_| "bad wave counter".to_owned());
        let mut next = Wave {
            turns: parse(h[1])?,
            operations: parse(h[2])?,
            dropped: parse(h[3])?,
            ..Self::default()
        };
        for line in lines {
            if next.cells.len() >= MAX_CELLS {
                return Err("wave domain exceeds capacity".into());
            }
            let v: Vec<_> = line.split_whitespace().collect();
            if v.len() != 5 {
                return Err("bad wave row".into());
            }
            let coord = |s: &str| s.parse::<i32>().map_err(|_| "bad wave position".to_owned());
            let value = |s: &str| s.parse::<i64>().map_err(|_| "bad wave value".to_owned());
            if next
                .cells
                .insert(
                    (coord(v[0])?, coord(v[1])?, coord(v[2])?),
                    State {
                        q: value(v[3])?,
                        p: value(v[4])?,
                    },
                )
                .is_some()
            {
                return Err("duplicate wave position".into());
            }
        }
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn impulse_propagates_and_both_shears_invert_exactly() {
        let mut states = BTreeMap::from([
            ((0, 0, 0), State { q: 0, p: 4096 }),
            ((1, 0, 0), State::default()),
            ((2, 0, 0), State::default()),
        ]);
        let original = states.clone();
        let mass = states.keys().map(|&p| (p, 1)).collect();
        let edges = [((0, 0, 0), (1, 0, 0)), ((1, 0, 0), (2, 0, 0))];
        for _ in 0..24 {
            assert!(kick(&mut states, &edges, false));
            assert!(drift(&mut states, &mass, false));
        }
        assert_ne!(
            states[&(2, 0, 0)],
            State::default(),
            "a distant cell receives a signal"
        );
        assert_eq!(states.values().map(|s| s.p).sum::<i64>(), 4096);
        println!("wave after 24 steps: {states:?}");
        for _ in 0..24 {
            assert!(drift(&mut states, &mass, true));
            assert!(kick(&mut states, &edges, true));
        }
        assert_eq!(states, original);
    }
}
