// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Restricted mobility from conserved charge, first spatial moment, and quadratic norm.
mod model;

pub use model::Dipole;
use pwc_module_api::Action;
use pwc_module_api::engine::Key;
use pwc_module_api::input::intent::{Chord, Modules, Source};
use pwc_module_api::prelude::*;
use pwc_module_api::sim::model::ModelSpec;

const ID: &str = "pwc.physics-dipole";

/// Create an independent, initially unactivated dipole-conserving laboratory.
pub fn spec() -> ModelSpec {
    ModelSpec {
        id: ID,
        version: 1,
        create: || Box::new(Dipole::default()),
    }
}

/// Register the optional model and explicit external charge-preparation controls.
pub fn register(registrar: &mut ModuleRegistrar) {
    let enabled = registrar.option(OptionSpec::toggle(
        "enabled",
        "Dipole-Conserving Physics",
        Category::World,
        false,
    ));
    registrar.add(Laboratory { enabled, on: false });
}

struct Laboratory {
    enabled: OptionId,
    on: bool,
}
impl Module for Laboratory {
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
        const ACTIONS: &[Action] = &[
            Action {
                id: "dipole.prepare-pair",
                label: "Prepare neutral signed pair",
                default: &[Chord::with(Modules::CTRL, Source::Key(Key::F5))],
                repeat: false,
                held: false,
                immediate: false,
            },
            Action {
                id: "dipole.prepare-positive",
                label: "Prepare isolated positive occupation",
                default: &[Chord::with(Modules::CTRL, Source::Key(Key::F6))],
                repeat: false,
                held: false,
                immediate: false,
            },
            Action {
                id: "dipole.prepare-negative",
                label: "Prepare isolated negative occupation",
                default: &[Chord::with(Modules::SHIFT, Source::Key(Key::F6))],
                repeat: false,
                held: false,
                immediate: false,
            },
        ];
        ACTIONS
    }
    fn update(&mut self, ctx: &mut ModuleContext) {
        if ctx.networked || ctx.world.simulation_model_id() != Some(ID) {
            return;
        }
        let input = if ctx.action("dipole.prepare-pair") {
            Some(1)
        } else if ctx.action("dipole.prepare-positive") {
            Some(2)
        } else if ctx.action("dipole.prepare-negative") {
            Some(-2)
        } else {
            None
        };
        if let Some(input) = input {
            let p = ctx.world.simulation_cell(ctx.player.position);
            ctx.world.perturb_simulation(p, input);
        }
    }
    fn hud(&self, facts: &HudFacts, world: &World, player: &Player, out: &mut Vec<HudElement>) {
        if world.simulation_model_id() != Some(ID) || !facts.hud_mode.shows_module_hud() {
            return;
        }
        let s = world.simulation_sample(world.simulation_cell(player.position));
        let stats = world.simulation_stats();
        out.push(HudElement::Label {
            at:Anchor::BottomLeft,off:(12,-82),base_fs:15,role:Role::Muted,
            text:format!("Dipole q={} Q={} norm={} Dx={} steps={} refused={} | Ctrl+F5 pair; Ctrl/Shift+F6 +/-",
                s.values[0],s.values[1],s.values[2],s.values[3],stats.turns,stats.dropped).into(),
        });
    }
}
