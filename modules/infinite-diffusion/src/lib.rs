//! InfiniteDiffusion worldgen module: mountains and valleys on the surface, caves and abandoned mines
//! below, planets in space above (see [`pwc_module_api::world::terrain`]). Its options apply to new
//! worlds.
//!
//! The terrain itself lives in the game (world generation is part of the deterministic core and
//! the network fingerprint); this module selects it and declares its options (relief, caves, mines,
//! space) in the game's options registry. A build without the module makes the core's flat world.

use pwc_module_api::settings::{Category, OptionId, OptionSpec, Options};
use pwc_module_api::world::generation::WorldgenKind;
use pwc_module_api::world::terrain::TerrainCfg;
use pwc_module_api::{Module, ModuleRegistrar};

const STEP: i32 = TerrainCfg::STEP as i32;
const RELIEF: (i32, i32, i32) = (TerrainCfg::RELIEF.0 as i32, TerrainCfg::RELIEF.1 as i32, STEP);
const DENSITY: (i32, i32, i32) = (TerrainCfg::DENSITY.0 as i32, TerrainCfg::DENSITY.1 as i32, STEP);

/// The four options, in the order the settings page lists them. Each applies to new worlds, and
/// each reads the knob value an older game kept in `modules.cfg` (`diffusion.state=relief=…`) once.
pub const OPTIONS: [OptionSpec; 4] = [
    OptionSpec::percent("relief", "Terrain Relief", Category::World, RELIEF, 100)
        .next_world()
        .legacy_key("diffusion.state.relief"),
    OptionSpec::percent("caves", "Caves", Category::World, DENSITY, 100).next_world().legacy_key("diffusion.state.caves"),
    OptionSpec::percent("mines", "Mines", Category::World, DENSITY, 100).next_world().legacy_key("diffusion.state.mines"),
    OptionSpec::percent("space", "Space", Category::World, DENSITY, 100).next_world().legacy_key("diffusion.state.space"),
];

/// The package entry point: declares the terrain options and installs [`InfiniteDiffusionModule`].
pub fn register(registrar: &mut ModuleRegistrar) {
    let ids = OPTIONS.map(|spec| registrar.option(spec));
    registrar.add(InfiniteDiffusionModule { options: Some(ids), ..InfiniteDiffusionModule::new() });
}

/// The InfiniteDiffusion module (id `diffusion`).
pub struct InfiniteDiffusionModule {
    /// Relief, caves, mines and space, when registered through the package.
    options: Option<[OptionId; 4]>,
    cfg: TerrainCfg,
}

impl InfiniteDiffusionModule {
    /// The module with every knob at 100 %.
    pub fn new() -> Self {
        Self { options: None, cfg: TerrainCfg::default() }
    }

    /// The generator configuration new worlds get.
    pub fn cfg(&self) -> TerrainCfg {
        self.cfg
    }
}

impl Default for InfiniteDiffusionModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for InfiniteDiffusionModule {
    fn name(&self) -> &str {
        "InfiniteDiffusion"
    }

    fn id(&self) -> &'static str {
        WorldgenKind::Diffusion.id()
    }

    fn on_options(&mut self, options: &Options) {
        let Some([relief, caves, mines, space]) = self.options else { return };
        let percent = |id| options.int(id).clamp(0, u16::MAX as i32) as u16;
        self.cfg = TerrainCfg {
            relief: percent(relief),
            caves: percent(caves),
            mines: percent(mines),
            space: percent(space),
            ..self.cfg
        }
        .clamp();
    }

    fn worldgen(&self) -> Option<WorldgenKind> {
        Some(WorldgenKind::Diffusion)
    }

    fn worldgen_config(&self) -> Option<String> {
        Some(self.cfg.to_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwc_module_api::testing::Harness;
    use pwc_module_api::settings::OptionValue;
    use pwc_module_api::{GameBuild, ModuleDescriptor};

    fn build() -> Harness {
        Harness::new(GameBuild::new()
            .with_module(ModuleDescriptor { id: "pwc.infinite-diffusion", name: "InfiniteDiffusion", version: "1.1.0", register }))
    }

    fn payload_cfg(modules: &Harness) -> TerrainCfg {
        modules.worldgen_config().as_deref().map(TerrainCfg::from_text).unwrap_or_default()
    }

    fn option(modules: &Harness, key: &str) -> OptionId {
        modules.options().find(&format!("pwc.infinite-diffusion.{key}")).expect("declared")
    }

    #[test]
    fn worldgen_kind_follows_the_module() {
        let mut modules = build();
        assert_eq!(modules.worldgen_kind(), WorldgenKind::Diffusion);
        assert!(modules.is_worldgen(0));
        assert_eq!(modules.package(0), Some("pwc.infinite-diffusion"));
        assert_eq!((modules.id(0), modules.name(0)), (WorldgenKind::Diffusion.id(), "InfiniteDiffusion"));
        modules.suspend(&["pwc.infinite-diffusion"]);
        assert_eq!(modules.worldgen_kind(), WorldgenKind::Flat, "the core fallback is the flat world");
        assert_eq!(modules.worldgen_config(), None);
    }

    #[test]
    fn the_options_are_the_next_new_world_payload() {
        let mut modules = build();
        assert_eq!(payload_cfg(&modules), TerrainCfg::default());
        let (relief, caves, space) = (option(&modules, "relief"), option(&modules, "caves"), option(&modules, "space"));
        let spec = modules.options().spec(relief);
        assert_eq!((spec.label, spec.page), ("Terrain Relief", Category::World));
        assert_eq!(spec.applies, pwc_module_api::settings::Applies::NextWorld);
        assert_eq!(spec.legacy_key, Some("diffusion.state.relief"), "the old modules.cfg knob carries over");
        modules.options_mut().step(relief, 1);
        modules.options_mut().step(caves, -1);
        modules.options_mut().step(space, 1);
        modules.options_changed();
        let cfg = payload_cfg(&modules);
        assert_eq!((cfg.relief, cfg.caves, cfg.space, cfg.mines), (125, 75, 125, 100));
        for _ in 0..20 {
            modules.options_mut().step(relief, 1);
        }
        modules.options_mut().set(caves, OptionValue::Int(37));
        modules.options_changed();
        let cfg = payload_cfg(&modules);
        assert_eq!(cfg.relief, TerrainCfg::RELIEF.1, "relief stops at its top");
        assert_eq!(cfg.caves, 25, "a stray value snaps onto the stepper");
    }

    #[test]
    fn a_module_without_options_keeps_the_defaults() {
        let mut m = InfiniteDiffusionModule::new();
        m.on_options(&Options::new());
        assert_eq!(m.cfg(), TerrainCfg::default());
        assert_eq!(m.worldgen_config().map(|t| TerrainCfg::from_text(&t)), Some(TerrainCfg::default()));
    }
}
