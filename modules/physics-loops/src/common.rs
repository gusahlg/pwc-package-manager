// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Registration and finite laboratory coordinates, shared only within this package.
use pwc_module_api::prelude::*;
use pwc_module_api::sim::{
    model::ModelSpec,
    reactions::{CellStore, Pos},
};
use pwc_module_api::{
    Action,
    engine::Key,
    input::intent::{Chord, Modules, Source},
};

pub const RADIUS: i32 = 3;
pub const SITES: usize = 343;
pub const MAX_TURNS: u64 = 1 << 60;

pub fn register(registrar: &mut ModuleRegistrar) {
    let enabled = registrar.option(OptionSpec::toggle(
        "enabled",
        crate::TITLE,
        Category::World,
        false,
    ));
    registrar.add(Provider { enabled, on: false });
}
struct Provider {
    enabled: OptionId,
    on: bool,
}
impl Module for Provider {
    fn id(&self) -> &'static str {
        crate::ID
    }
    fn on_options(&mut self, options: &Options) {
        self.on = options.bool(self.enabled);
    }
    fn reaction_model(&self) -> Option<ModelSpec> {
        self.on.then_some(crate::spec())
    }
    fn actions(&self) -> &[Action] {
        const INPUT: &[Action] = &[Action {
            id: crate::ACTION,
            label: crate::TITLE,
            default: &[Chord::with(Modules::CTRL, Source::Key(Key::F5))],
            repeat: false,
            held: false,
            immediate: false,
        }];
        INPUT
    }
    fn update(&mut self, ctx: &mut ModuleContext) {
        if !ctx.networked
            && ctx.world.simulation_model_id() == Some(crate::ID)
            && ctx.action(crate::ACTION)
        {
            let pos = ctx.world.simulation_cell(ctx.player.position);
            ctx.world.perturb_simulation(pos, crate::INPUT);
        }
    }
    fn hud(&self, facts: &HudFacts, world: &World, player: &Player, out: &mut Vec<HudElement>) {
        if world.simulation_model_id() != Some(crate::ID) || !facts.hud_mode.shows_module_hud() {
            return;
        }
        let s = world.simulation_sample(world.simulation_cell(player.position));
        out.push(HudElement::Label {
            at: Anchor::BottomLeft,
            off: (12, -82),
            base_fs: 15,
            role: Role::Muted,
            text: format!(
                "{}: {}={} {}={} {}={} {}={} | Ctrl+F5",
                crate::TITLE,
                s.labels[0],
                s.values[0],
                s.labels[1],
                s.values[1],
                s.labels[2],
                s.values[2],
                s.labels[3],
                s.values[3]
            )
            .into(),
        });
    }
}

pub fn offset(mut pos: Pos, axis: u8, amount: i32) -> Option<Pos> {
    match axis {
        0 => pos.0 = pos.0.checked_add(amount)?,
        1 => pos.1 = pos.1.checked_add(amount)?,
        2 => pos.2 = pos.2.checked_add(amount)?,
        _ => return None,
    }
    Some(pos)
}
pub fn contains(origin: Pos, p: Pos) -> bool {
    [
        i64::from(p.0) - i64::from(origin.0),
        i64::from(p.1) - i64::from(origin.1),
        i64::from(p.2) - i64::from(origin.2),
    ]
    .iter()
    .all(|v| v.abs() <= i64::from(RADIUS))
}
pub fn cube(origin: Pos) -> Option<Vec<Pos>> {
    let mut out = Vec::with_capacity(SITES);
    for x in -RADIUS..=RADIUS {
        for y in -RADIUS..=RADIUS {
            for z in -RADIUS..=RADIUS {
                out.push((
                    origin.0.checked_add(x)?,
                    origin.1.checked_add(y)?,
                    origin.2.checked_add(z)?,
                ));
            }
        }
    }
    Some(out)
}
pub fn readable(store: &dyn CellStore, positions: &[Pos]) -> bool {
    positions
        .iter()
        .all(|&p| store.resolve(p) == p && store.block_at(p).is_some())
}

#[derive(Clone, Copy, Default)]
pub struct Clock {
    pub turns: u64,
    pub operations: u64,
    pub refused: u64,
}
impl Clock {
    pub fn refuse(&mut self) {
        self.refused = self.refused.saturating_add(1);
    }
    pub fn advance(&mut self, count: usize) {
        self.turns += 1;
        self.operations = self.operations.saturating_add(count as u64);
    }
    pub fn header(self, tag: &str, origin: Option<Pos>) -> String {
        let pos = origin.map_or_else(|| "none".into(), |(x, y, z)| format!("{x} {y} {z}"));
        format!(
            "{tag} {} {} {} {pos}\n",
            self.turns, self.operations, self.refused
        )
    }
}
pub fn header(line: &str, tag: &str) -> Result<(Clock, Option<Pos>), String> {
    let h: Vec<_> = line.split_whitespace().collect();
    if !matches!(h.len(), 5 | 7) || h[0] != tag {
        return Err("unsupported laboratory state".into());
    }
    let number = |s: &str| {
        s.parse::<u64>()
            .map_err(|_| "invalid laboratory counter".to_owned())
    };
    let clock = Clock {
        turns: number(h[1])?,
        operations: number(h[2])?,
        refused: number(h[3])?,
    };
    if clock.turns > MAX_TURNS {
        return Err("laboratory clock exceeds bound".into());
    }
    let origin = if h.len() == 7 {
        Some((coord(h[4])?, coord(h[5])?, coord(h[6])?))
    } else if h[4] == "none" {
        None
    } else {
        return Err("invalid laboratory anchor".into());
    };
    if origin.is_some_and(|p| cube(p).is_none()) {
        return Err("anchor crosses integer coordinate border".into());
    }
    Ok((clock, origin))
}
pub fn coord(s: &str) -> Result<i32, String> {
    s.parse()
        .map_err(|_| "invalid laboratory coordinate".into())
}
