// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! A memory-bearing elastic network with shared-node strain relaxation, local yielding and an exact heat ledger.
mod model;
use pwc_module_api::Action;
use pwc_module_api::engine::Key;
use pwc_module_api::input::intent::{Chord, Mods, Source};
use pwc_module_api::prelude::*;
use pwc_module_api::sim::model::ModelSpec;

const ID: &str = "pwc.physics-bonds";
/// Construct an independent authoritative laboratory instance.
pub fn spec() -> ModelSpec {
    ModelSpec {
        id: ID,
        version: 1,
        create: || Box::new(model::Bonds::default()),
    }
}
/// Register the optional model and its experimental single-player controls.
pub fn register(registrar: &mut ModuleRegistrar) {
    let enabled = registrar.option(OptionSpec::toggle(
        "enabled",
        "Plastic Bond Physics",
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
        const ACTIONS: &[Action] = &[Action {
            id: "pwc.physics-bonds.input",
            label: "Bond laboratory external input",
            default: &[Chord::with(Mods::CTRL, Source::Key(Key::F5))],
            repeat: false,
            held: false,
            immediate: false,
        }];
        ACTIONS
    }
    fn update(&mut self, ctx: &mut ModuleContext) {
        if !ctx.networked
            && ctx.world.simulation_model_id() == Some(ID)
            && ctx.action("pwc.physics-bonds.input")
        {
            let pos = ctx.world.simulation_cell(ctx.player.position);
            ctx.world.perturb_simulation(pos, 32);
        }
    }
    fn hud(&self, facts: &HudFacts, world: &World, player: &Player, out: &mut Vec<HudElement>) {
        if world.simulation_model_id() != Some(ID) || !facts.hud_mode.shows_module_hud() {
            return;
        }
        let sample = world.simulation_sample(world.simulation_cell(player.position));
        let stats = world.simulation_stats();
        out.push(HudElement::Label {
            at: Anchor::BottomLeft,
            off: (12, -82),
            base_fs: 15,
            role: Role::Muted,
            text: format!(
                "Bond laboratory: {}={} {}={} steps={} refused={} | Ctrl+F5 input",
                sample.labels[0],
                sample.values[0],
                sample.labels[1],
                sample.values[1],
                stats.turns,
                stats.dropped
            )
            .into(),
        });
    }
}
