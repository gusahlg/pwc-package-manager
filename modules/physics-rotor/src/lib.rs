// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: AGPL-3.0-or-later
//! Conserved impulses traverse matter with local routing memory in a bounded laboratory.

use std::collections::BTreeMap;

use pwc_module_api::Action;
use pwc_module_api::engine::Key;
use pwc_module_api::input::intent::{Chord, Modules, Source};
use pwc_module_api::prelude::*;
use pwc_module_api::sim::{model::*, reactions::*};

pub const ID: &str = "pwc.physics-rotor";
const RADIUS: i64 = 3;
const MAX_SITES: usize = 343;
const MAX_IMPULSES: u32 = 65_535;

pub fn register(registrar: &mut ModuleRegistrar) {
    let enabled = registrar.option(OptionSpec::toggle(
        "enabled",
        "Memory Matter Physics",
        Category::World,
        false,
    ));
    registrar.add(RotorModule { enabled, on: false });
}

/// Factory for explicit server configurations and small headless laboratories.
pub fn spec() -> ModelSpec {
    ModelSpec {
        id: ID,
        version: 1,
        create: || Box::new(Rotor::default()),
    }
}

pub struct RotorModule {
    enabled: OptionId,
    on: bool,
}

impl Module for RotorModule {
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
            id: "physics.rotor.impulse",
            label: "Memory matter: add an impulse",
            default: &[Chord::with(Modules::CTRL, Source::Key(Key::F4))],
            repeat: false,
            held: false,
            immediate: false,
        }];
        ACTIONS
    }
    fn update(&mut self, ctx: &mut ModuleContext) {
        if !ctx.networked
            && ctx.world.simulation_model_id() == Some(ID)
            && ctx.action("physics.rotor.impulse")
        {
            let pos = ctx.world.simulation_cell(ctx.player.position);
            ctx.world.perturb_simulation(pos, 1);
        }
    }
    fn hud(&self, facts: &HudFacts, world: &World, player: &Player, out: &mut Vec<HudElement>) {
        if world.simulation_model_id() != Some(ID) || !facts.hud_mode.shows_module_hud() {
            return;
        }
        let state = world.simulation_sample(world.simulation_cell(player.position));
        let stats = world.simulation_stats();
        out.push(HudElement::Label {
            at: Anchor::BottomLeft,
            off: (12, -82),
            base_fs: 15,
            role: Role::Muted,
            text: format!(
                "Memory matter impulses={} here={} rotor={} turns={} refused={} | Ctrl+F4",
                state.values[2], state.values[0], state.values[1], stats.turns, stats.dropped
            )
            .into(),
        });
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Site {
    impulses: u32,
    rotor: u8,
}

/// Sparse visited sites retain memory; absent sites begin with rotor zero.
#[derive(Default)]
pub struct Rotor {
    origin: Option<Pos>,
    sites: BTreeMap<Pos, Site>,
    turns: u64,
    operations: u64,
    refused: u64,
}

fn contains(origin: Pos, pos: Pos) -> bool {
    (i64::from(origin.0) - i64::from(pos.0)).abs() <= RADIUS
        && (i64::from(origin.1) - i64::from(pos.1)).abs() <= RADIUS
        && (i64::from(origin.2) - i64::from(pos.2)).abs() <= RADIUS
}

fn adjacent(pos: Pos, port: u8) -> Option<Pos> {
    let (x, y, z) = pos;
    Some(match port {
        0 => (x.checked_add(1)?, y, z),
        1 => (x.checked_sub(1)?, y, z),
        2 => (x, y.checked_add(1)?, z),
        3 => (x, y.checked_sub(1)?, z),
        4 => (x, y, z.checked_add(1)?),
        5 => (x, y, z.checked_sub(1)?),
        _ => return None,
    })
}

/// One common, permutation-independent formula changes orientation and chirality for any matter.
fn ports(store: &dyn CellStore, pos: Pos) -> Option<(u8, bool)> {
    if store.resolve(pos) != pos {
        return None;
    }
    let id = store.block_at(pos)?;
    let cfg = store.registry().configuration(id);
    let sum: u32 = cfg
        .elements()
        .iter()
        .map(|e| {
            u32::from(e.0[0])
                + 3 * u32::from(e.0[1])
                + 5 * u32::from(e.0[2])
                + 7 * u32::from(e.0[3])
        })
        .sum();
    Some(((sum % 6) as u8, (sum / 6 + cfg.len() as u32) % 2 != 0))
}

impl Rotor {
    fn refuse(&mut self) {
        self.refused = self.refused.saturating_add(1);
    }
    fn total(&self) -> u64 {
        self.sites.values().map(|s| u64::from(s.impulses)).sum()
    }
}

impl Model for Rotor {
    fn id(&self) -> &'static str {
        ID
    }
    fn wake_cell(&mut self, _pos: Pos) {}

    fn perturb(&mut self, pos: Pos, amount: i64) {
        let origin = self.origin.unwrap_or(pos);
        let before = self.sites.get(&pos).copied().unwrap_or_default();
        let Ok(amount) = u32::try_from(amount) else {
            self.refuse();
            return;
        };
        let Some(after) = before.impulses.checked_add(amount) else {
            self.refuse();
            return;
        };
        if amount == 0 {
            return;
        }
        if !contains(origin, pos)
            || after > MAX_IMPULSES
            || (!self.sites.contains_key(&pos) && self.sites.len() >= MAX_SITES)
        {
            self.refuse();
            return;
        }
        self.origin = Some(origin);
        self.sites.insert(
            pos,
            Site {
                impulses: after,
                ..before
            },
        );
    }

    fn tick(&mut self, store: &mut dyn CellStore, budget: Budget, _out: &mut Vec<Mutation>) {
        let Some(origin) = self.origin else {
            return;
        };
        let active = self.sites.values().filter(|s| s.impulses > 0).count();
        // A logical turn attempts one departure at each initially occupied site.
        if active == 0 || active > budget.contacts_per_turn {
            return;
        }
        let mut moves = Vec::with_capacity(active);
        for (&source, state) in &self.sites {
            if state.impulses == 0 {
                continue;
            }
            let Some((orientation, reverse)) = ports(store, source) else {
                continue;
            };
            for skip in 0..6 {
                let slot = (state.rotor + skip) % 6;
                let port = (orientation + if reverse { (6 - slot) % 6 } else { slot }) % 6;
                let Some(dest) = adjacent(source, port) else {
                    continue;
                };
                // This lab refuses chart seams because resolve does not rotate directions.
                if contains(origin, dest)
                    && store.resolve(dest) == dest
                    && store.block_at(dest).is_some()
                {
                    moves.push((source, dest, (slot + 1) % 6));
                    break;
                }
            }
        }
        let mut next = self.sites.clone();
        // Remove every departing impulse before additions: incoming impulses wait one full turn.
        for &(source, _, rotor) in &moves {
            let site = next.get_mut(&source).unwrap();
            site.impulses -= 1;
            site.rotor = rotor;
        }
        for &(_, dest, _) in &moves {
            if !next.contains_key(&dest) && next.len() >= MAX_SITES {
                self.refuse();
                return;
            }
            let site = next.entry(dest).or_default();
            if site.impulses == MAX_IMPULSES {
                self.refuse();
                return;
            }
            site.impulses += 1;
        }
        // Capacity refusal above leaves every impulse and rotor in the old state.
        self.sites = next;
        self.turns = self.turns.saturating_add(1);
        self.operations = self.operations.saturating_add(moves.len() as u64);
    }

    fn stats(&self) -> Stats {
        Stats {
            turns: self.turns,
            operations: self.operations,
            dropped: self.refused,
            pending: self.sites.values().filter(|s| s.impulses > 0).count(),
        }
    }

    fn sample(&self, pos: Pos) -> Sample {
        let site = self.sites.get(&pos).copied().unwrap_or_default();
        Sample {
            labels: [
                "local impulses",
                "rotor index",
                "total impulses",
                "visited sites",
            ],
            values: [
                i64::from(site.impulses),
                i64::from(site.rotor),
                self.total() as i64,
                self.sites.len() as i64,
            ],
        }
    }

    fn save(&self) -> String {
        let origin = self
            .origin
            .map_or_else(|| "none".into(), |(x, y, z)| format!("{x} {y} {z}"));
        let mut out = format!(
            "rotor1 {} {} {} {origin}\n",
            self.turns, self.operations, self.refused
        );
        for (&(x, y, z), site) in &self.sites {
            out.push_str(&format!("{x} {y} {z} {} {}\n", site.impulses, site.rotor));
        }
        out
    }

    fn load(&mut self, data: &str) -> Result<(), String> {
        let mut lines = data.lines();
        let header: Vec<_> = lines.next().unwrap_or("").split_whitespace().collect();
        if !matches!(header.len(), 5 | 7) || header[0] != "rotor1" {
            return Err("unsupported memory matter state".into());
        }
        let counter = |s: &str| {
            s.parse::<u64>()
                .map_err(|_| "invalid rotor counter".to_owned())
        };
        let coord = |s: &str| {
            s.parse::<i32>()
                .map_err(|_| "invalid rotor coordinate".to_owned())
        };
        let origin = if header.len() == 7 {
            Some((coord(header[4])?, coord(header[5])?, coord(header[6])?))
        } else if header[4] == "none" {
            None
        } else {
            return Err("invalid rotor origin".into());
        };
        let mut next = Self {
            origin,
            turns: counter(header[1])?,
            operations: counter(header[2])?,
            refused: counter(header[3])?,
            ..Self::default()
        };
        for line in lines {
            if next.sites.len() >= MAX_SITES {
                return Err("rotor state exceeds site capacity".into());
            }
            let values: Vec<_> = line.split_whitespace().collect();
            if values.len() != 5 {
                return Err("invalid rotor row".into());
            }
            let pos = (coord(values[0])?, coord(values[1])?, coord(values[2])?);
            let impulses = values[3]
                .parse::<u32>()
                .map_err(|_| "invalid rotor impulses")?;
            let rotor = values[4].parse::<u8>().map_err(|_| "invalid rotor index")?;
            if impulses > MAX_IMPULSES || rotor >= 6 || !origin.is_some_and(|o| contains(o, pos)) {
                return Err("rotor row outside state bounds".into());
            }
            if next.sites.insert(pos, Site { impulses, rotor }).is_some() {
                return Err("duplicate rotor position".into());
            }
        }
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwc_module_api::block::AIR;

    struct Square(BlockRegistry);
    impl CellStore for Square {
        fn block_at(&self, (x, y, z): Pos) -> Option<BlockId> {
            ((0..=1).contains(&x) && (0..=1).contains(&y) && z == 0).then_some(AIR)
        }
        fn set_block(&mut self, _pos: Pos, _id: BlockId) -> Option<BlockId> {
            panic!("rotor experiment must never write matter")
        }
        fn registry(&self) -> &BlockRegistry {
            &self.0
        }
        fn registry_mut(&mut self) -> &mut BlockRegistry {
            &mut self.0
        }
    }

    #[test]
    fn closed_square_discovers_a_cycle_and_preserves_count_across_reload() {
        let mut square = Square(BlockRegistry::with_builtins());
        let mut rotor = Rotor::default();
        rotor.perturb((0, 0, 0), 1);
        let mut observed = BTreeMap::new();
        let mut period = None;
        for turn in 0..128 {
            if let Some(previous) = observed.insert(rotor.sites.clone(), turn) {
                period = Some(turn - previous);
                break;
            }
            rotor.tick(&mut square, Budget::DEFAULT, &mut Vec::new());
            assert_eq!(rotor.total(), 1);
        }
        let period = period.expect("bounded square should show a repeated routing state");
        assert!(period > 1);
        println!(
            "Memory matter closed-square cycle: {period} turns; visited {} sites",
            rotor.sites.len()
        );

        let saved = rotor.save();
        let mut restored = Rotor::default();
        restored.load(&saved).unwrap();
        assert_eq!(restored.save(), saved);
        restored.perturb((0, 0, 0), i64::from(MAX_IMPULSES) + 1);
        assert_eq!(
            restored.total(),
            1,
            "rejected input must not change conserved count"
        );
        assert_eq!(restored.stats().dropped, 1);
        restored = Rotor::default();
        restored.load(&saved).unwrap();
        for _ in 0..32 {
            rotor.tick(&mut square, Budget::DEFAULT, &mut Vec::new());
            restored.tick(&mut square, Budget::DEFAULT, &mut Vec::new());
            assert_eq!(restored.save(), rotor.save());
            assert_eq!(rotor.total(), 1);
        }
    }
}
