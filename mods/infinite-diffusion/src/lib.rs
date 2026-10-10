//! InfiniteDiffusion worldgen mod: mountains and valleys on the surface, caves and abandoned mines
//! below, planets in space above (see [`pwc_mod_api::world::terrain`]). Its options apply to new
//! worlds.
//!
//! The terrain itself lives in the game (world generation is part of the deterministic core and
//! the network fingerprint); this mod selects it and declares its options (relief, caves, mines,
//! space) in the game's options registry. A build without the mod makes the core's flat world.

use pwc_mod_api::settings::{Category, OptionId, OptionSpec, Options};
use pwc_mod_api::world::generation::WorldgenKind;
use pwc_mod_api::world::terrain::TerrainCfg;
use pwc_mod_api::{Mod, ModRegistrar};

const STEP: i32 = TerrainCfg::STEP as i32;
const RELIEF: (i32, i32, i32) = (TerrainCfg::RELIEF.0 as i32, TerrainCfg::RELIEF.1 as i32, STEP);
const DENSITY: (i32, i32, i32) = (TerrainCfg::DENSITY.0 as i32, TerrainCfg::DENSITY.1 as i32, STEP);

/// The four options, in the order the settings page lists them. Each applies to new worlds.
pub const OPTIONS: [OptionSpec; 4] = [
    OptionSpec::percent("relief", "Terrain Relief", Category::World, RELIEF, 100).next_world(),
    OptionSpec::percent("caves", "Caves", Category::World, DENSITY, 100).next_world(),
    OptionSpec::percent("mines", "Mines", Category::World, DENSITY, 100).next_world(),
    OptionSpec::percent("space", "Space", Category::World, DENSITY, 100).next_world(),
];

/// The package entry point: declares the terrain options and installs [`InfiniteDiffusionMod`].
pub fn register(registrar: &mut ModRegistrar) {
    let ids = OPTIONS.map(|spec| registrar.option(spec));
    registrar.add(InfiniteDiffusionMod { options: Some(ids), ..InfiniteDiffusionMod::new() });
}

/// The InfiniteDiffusion mod (id `diffusion`).
pub struct InfiniteDiffusionMod {
    /// Relief, caves, mines and space, when registered through the package.
    options: Option<[OptionId; 4]>,
    cfg: TerrainCfg,
}

impl InfiniteDiffusionMod {
    /// The mod with every knob at 100 %.
    pub fn new() -> Self {
        Self { options: None, cfg: TerrainCfg::default() }
    }

    /// The generator configuration new worlds get.
    pub fn cfg(&self) -> TerrainCfg {
        self.cfg
    }
}

impl Default for InfiniteDiffusionMod {
    fn default() -> Self {
        Self::new()
    }
}

impl Mod for InfiniteDiffusionMod {
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
    use pwc_mod_api::settings::OptionValue;
    use pwc_mod_api::{GameBuild, ModDescriptor, Mods};

    fn build() -> Mods {
        GameBuild::new()
            .with_mod(ModDescriptor { id: "pwc.infinite-diffusion", name: "InfiniteDiffusion", version: "1.1.0", register })
            .mods()
    }

    fn payload_cfg(mods: &Mods) -> TerrainCfg {
        mods.worldgen_config().as_deref().map(TerrainCfg::from_text).unwrap_or_default()
    }

    fn option(mods: &Mods, key: &str) -> OptionId {
        mods.options().find(&format!("pwc.infinite-diffusion.{key}")).expect("declared")
    }

    #[test]
    fn worldgen_kind_follows_the_mod() {
        let mut mods = build();
        assert_eq!(mods.worldgen_kind(), WorldgenKind::Diffusion);
        assert!(mods.is_worldgen(0));
        assert_eq!(mods.package(0), Some("pwc.infinite-diffusion"));
        assert_eq!((mods.id(0), mods.name(0)), (WorldgenKind::Diffusion.id(), "InfiniteDiffusion"));
        mods.suspend_packages(&["pwc.infinite-diffusion".to_string()]);
        assert_eq!(mods.worldgen_kind(), WorldgenKind::Flat, "the core fallback is the flat world");
        assert_eq!(mods.worldgen_config(), None);
    }

    #[test]
    fn the_options_are_the_next_new_world_payload() {
        let mut mods = build();
        assert_eq!(payload_cfg(&mods), TerrainCfg::default());
        let (relief, caves, space) = (option(&mods, "relief"), option(&mods, "caves"), option(&mods, "space"));
        let spec = mods.options().spec(relief);
        assert_eq!((spec.label, spec.page), ("Terrain Relief", Category::World));
        assert_eq!(spec.applies, pwc_mod_api::settings::Applies::NextWorld);
        mods.options_mut().step(relief, 1);
        mods.options_mut().step(caves, -1);
        mods.options_mut().step(space, 1);
        mods.options_changed();
        let cfg = payload_cfg(&mods);
        assert_eq!((cfg.relief, cfg.caves, cfg.space, cfg.mines), (125, 75, 125, 100));
        for _ in 0..20 {
            mods.options_mut().step(relief, 1);
        }
        mods.options_mut().set(caves, OptionValue::Int(37));
        mods.options_changed();
        let cfg = payload_cfg(&mods);
        assert_eq!(cfg.relief, TerrainCfg::RELIEF.1, "relief stops at its top");
        assert_eq!(cfg.caves, 25, "a stray value snaps onto the stepper");
    }

    #[test]
    fn a_mod_without_options_keeps_the_defaults() {
        let mut m = InfiniteDiffusionMod::new();
        m.on_options(&Options::new());
        assert_eq!(m.cfg(), TerrainCfg::default());
        assert_eq!(m.worldgen_config().map(|t| TerrainCfg::from_text(&t)), Some(TerrainCfg::default()));
    }
}
