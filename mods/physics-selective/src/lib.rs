// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: AGPL-3.0-or-later
//! Native provider of the historical selective-transfer law. No core fallback selects it.
pub mod model;
use pwc_mod_api::settings::{Category, OptionId, OptionSpec, Options};
use pwc_mod_api::sim::model::ModelSpec;
use pwc_mod_api::{Mod, ModRegistrar};

struct Physics {
    enabled: bool,
    option: OptionId,
}
pub fn spec() -> ModelSpec {
    ModelSpec {
        id: model::ID,
        version: 1,
        create: || Box::new(model::SelectiveTransfer::new()),
    }
}
impl Mod for Physics {
    fn id(&self) -> &'static str {
        model::ID
    }
    fn on_options(&mut self, options: &Options) {
        self.enabled = options.bool(self.option);
    }
    fn reaction_model(&self) -> Option<ModelSpec> {
        self.enabled.then_some(spec())
    }
}
pub fn register(registrar: &mut ModRegistrar) {
    let option = registrar.option(OptionSpec::toggle(
        "enabled",
        "Selective Transfer Physics",
        Category::World,
        true,
    ));
    registrar.add(Physics {
        enabled: true,
        option,
    });
}
