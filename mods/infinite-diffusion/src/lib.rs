//! InfiniteDiffusion worldgen mod: mountains and valleys on the surface, caves and abandoned mines
//! below, planets in space above (see [`pwc_mod_api::world::terrain`]). Knobs apply to new worlds.
//!
//! The terrain itself lives in the game (world generation is part of the deterministic core and
//! the network fingerprint); this mod selects it and carries its knobs. With the mod disabled, new
//! worlds use the core's flat world.

use pwc_mod_api::world::generation::WorldgenKind;
use pwc_mod_api::world::terrain::TerrainCfg;
use pwc_mod_api::world::World;
use pwc_mod_api::{Knob, Mod, ModRegistrar, ESSENTIALS};

/// The package entry point: installs [`InfiniteDiffusionMod`], enabled.
pub fn register(registrar: &mut ModRegistrar) {
    registrar.add(InfiniteDiffusionMod::new());
}

/// The InfiniteDiffusion mod (id `diffusion`).
pub struct InfiniteDiffusionMod {
    cfg: TerrainCfg,
}

impl InfiniteDiffusionMod {
    /// The mod with default knobs (every knob at 100 %).
    pub fn new() -> Self {
        Self { cfg: TerrainCfg::default() }
    }

    #[cfg(test)]
    fn cfg(&self) -> TerrainCfg {
        self.cfg
    }

    fn apply_cfg_text(&mut self, data: &str) {
        self.cfg = self.cfg.overlay(data);
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

    fn description(&self) -> &str {
        "Mountain ranges and carved valleys, caves and abandoned mines below, planets in space above (new worlds)."
    }

    fn group(&self) -> &'static str {
        ESSENTIALS
    }

    fn worldgen(&self) -> Option<WorldgenKind> {
        Some(WorldgenKind::Diffusion)
    }

    fn worldgen_config(&self) -> Option<String> {
        Some(self.cfg.to_text())
    }

    fn knobs(&self) -> Vec<Knob> {
        let (rlo, rhi) = TerrainCfg::RELIEF;
        let (dlo, dhi) = TerrainCfg::DENSITY;
        vec![
            Knob { label: "Relief", value: format!("{}%", self.cfg.relief), hint: format!("{rlo}..{rhi}%") },
            Knob { label: "Caves", value: format!("{}%", self.cfg.caves), hint: format!("{dlo}..{dhi}%") },
            Knob { label: "Mines", value: format!("{}%", self.cfg.mines), hint: format!("{dlo}..{dhi}%") },
            Knob { label: "Space", value: format!("{}%", self.cfg.space), hint: format!("{dlo}..{dhi}%") },
        ]
    }

    fn step_knob(&mut self, index: usize, delta: i32) {
        let step = |v: u16| (v as i32 + delta * TerrainCfg::STEP as i32).max(0) as u16;
        match index {
            0 => self.cfg.relief = step(self.cfg.relief),
            1 => self.cfg.caves = step(self.cfg.caves),
            2 => self.cfg.mines = step(self.cfg.mines),
            3 => self.cfg.space = step(self.cfg.space),
            _ => {}
        }
        self.cfg = self.cfg.clamp();
    }

    fn save_state(&self, _world: &World) -> Option<(u16, String)> {
        Some((2, self.cfg.to_text()))
    }

    fn load_state(&mut self, _version: u16, data: &str, _world: &mut World) -> u32 {
        self.apply_cfg_text(data);
        0
    }

    fn save_choice_state(&self) -> Option<String> {
        Some(self.cfg.to_text())
    }

    fn load_choice_state(&mut self, data: &str) {
        self.apply_cfg_text(data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwc_mod_api::{GameBuild, ModDescriptor, Mods};

    fn build() -> Mods {
        GameBuild::new()
            .with_mod(ModDescriptor { id: "pwc.infinite-diffusion", name: "InfiniteDiffusion", version: "1.0.0", register })
            .mods()
    }

    fn payload_cfg(mods: &Mods) -> TerrainCfg {
        mods.worldgen_config().as_deref().map(TerrainCfg::from_text).unwrap_or_default()
    }

    #[test]
    fn knobs_step_snap_and_round_trip_through_text() {
        let mut m = InfiniteDiffusionMod::new();
        assert_eq!(m.id(), WorldgenKind::Diffusion.id());
        m.step_knob(0, 2);
        assert_eq!(m.cfg().relief, 150);
        m.step_knob(2, -10);
        assert_eq!(m.cfg().mines, 0, "density knobs bottom out at 0");
        m.step_knob(0, 10);
        assert_eq!(m.cfg().relief, TerrainCfg::RELIEF.1);
        let text = m.save_choice_state().unwrap();
        let mut n = InfiniteDiffusionMod::new();
        n.load_choice_state(&text);
        assert_eq!(n.cfg(), m.cfg());
        n.load_choice_state("relief=37,unknown=9");
        assert_eq!(n.cfg().relief, 25, "a stray value snaps onto the stepper");
    }

    #[test]
    fn worldgen_kind_follows_the_mod() {
        let mods = build();
        assert_eq!(mods.worldgen_kind(), WorldgenKind::Diffusion, "InfiniteDiffusion is on by default");
        assert!(mods.is_worldgen(0));
        assert_eq!(mods.group(0), ESSENTIALS);
        assert_eq!(mods.package(0), Some("pwc.infinite-diffusion"));
        let mut off = build();
        off.set_enabled("diffusion", false);
        assert_eq!(off.worldgen_kind(), WorldgenKind::Flat, "the core fallback is the flat world");
    }

    #[test]
    fn set_enabled_keys_on_id_case_insensitively() {
        let mut mods = build();
        assert_eq!(mods.id(0), WorldgenKind::Diffusion.id());
        assert_eq!(mods.name(0), "InfiniteDiffusion");
        assert_ne!(mods.id(0), mods.name(0));
        mods.set_enabled("diffusion", true);
        assert_eq!(mods.worldgen_kind(), WorldgenKind::Diffusion);
        mods.set_enabled("DIFFUSION", false);
        assert_eq!(mods.worldgen_kind(), WorldgenKind::Flat);
        mods.set_enabled("InfiniteDiffusion", true);
        assert_eq!(mods.worldgen_kind(), WorldgenKind::Flat, "display name is not a set_enabled key");
    }

    #[test]
    fn worldgen_config_is_the_winning_kind_payload() {
        let mut off = build();
        off.set_enabled("diffusion", false);
        assert_eq!(off.worldgen_config(), None);
        let mut on = build();
        let text = on.worldgen_config().expect("payload");
        assert_eq!(TerrainCfg::from_text(&text), TerrainCfg::default());
        on.step_knob(0, 3, 1);
        assert_eq!(TerrainCfg::from_text(&on.worldgen_config().unwrap()).space, 125);
    }

    #[test]
    fn choices_text_round_trips_knobs_and_ignores_junk() {
        let mut mods = build();
        let defaults = mods.choices_text();
        assert!(defaults.contains("diffusion=on"));
        assert!(defaults.contains("diffusion.state=relief=100,caves=100,mines=100,space=100"), "{defaults}");
        mods.step_knob(0, 0, 1);
        mods.step_knob(0, 1, -1);
        let cfg = payload_cfg(&mods);
        assert_eq!((cfg.relief, cfg.caves), (125, 75));
        let text = mods.choices_text();
        assert!(text.contains(&format!("diffusion.state={}", cfg.to_text())));
        let mut fresh = build();
        fresh.apply_choices_text(&text);
        assert_eq!(payload_cfg(&fresh), cfg);
        let mut junk = build();
        junk.apply_choices_text("version=2\ndiffusion=on\ndiffusion.state=relief=150\nunknown.state=tile=16\n");
        assert_eq!(payload_cfg(&junk).relief, 150);
        assert!(junk.is_enabled(0));
    }

    /// A pre-marker file wrote `diffusion=off` (and an unrelated knob payload) for everyone:
    /// it must not switch off the world generator.
    #[test]
    fn version_one_choices_keep_the_world_generator() {
        let mut mods = build();
        mods.apply_choices_text("diffusion=off\ndiffusion.state=tile=16,stride=16,phases=8,relief=1.00\n");
        let text = mods.choices_text();
        assert!(text.starts_with("version=2\n"));
        assert!(text.contains("diffusion=on"), "{text}");
        assert_eq!(payload_cfg(&mods).relief, 100);
        mods.apply_choices_text(&text.replace("diffusion=on", "diffusion=off"));
        assert!(mods.choices_text().contains("diffusion=off"), "a current file's choice applies");
    }

    #[test]
    fn world_state_round_trips_through_the_save_line() {
        let mut world = World::new(1);
        let mut mods = build();
        mods.step_knob(0, 2, 1);
        let saved = mods.save_states(&world);
        let cfg = payload_cfg(&mods);
        assert_eq!(saved, [("diffusion".to_string(), format!("v2;{}", cfg.to_text()))]);
        let mut fresh = build();
        fresh.load_state("diffusion", &saved[0].1, &mut world);
        assert_eq!(payload_cfg(&fresh), cfg);
    }

    #[test]
    fn bench_pins_switch_the_generator() {
        let mut mods = build();
        mods.apply_bench_env(Some(false), None);
        assert_eq!(mods.worldgen_kind(), WorldgenKind::Flat);
        assert!(mods.choices_text().contains("diffusion=off"));
        mods.apply_bench_env(Some(true), Some(true));
        assert_eq!(mods.worldgen_kind(), WorldgenKind::Diffusion);
    }
}
