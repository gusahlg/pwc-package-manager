//! Default-enabled visual mods. The core renderer is sunlight-only; these
//! restore the shipped look when enabled and strip their lanes when not.
//!
//! Three mods, one per render group: **Atmosphere** (`atmosphere`), **Post** (`post`) and
//! **Lighting** (`lighting`). Each owns a [`VisualGroup`]; the game ORs the enabled groups into
//! its render mask, and a disabled group's settings lanes are forced off (the settings menu marks
//! them `(off: <mod> mod)`).

use pwc_mod_api::render_config::VisualGroup;
use pwc_mod_api::{Mod, ModRegistrar, ESSENTIALS};

/// The package entry point: installs the three visual mods, enabled.
pub fn register(registrar: &mut ModRegistrar) {
    // Fancy lanes live in mods; disable any of these to get the core look.
    registrar.add(AtmosphereMod);
    registrar.add(PostMod);
    registrar.add(LightingMod);
}

macro_rules! visual_mod {
    ($ty:ident, $name:literal, $id:literal, $desc:literal, $group:ident) => {
        #[doc = concat!("The ", $name, " mod (id `", $id, "`): ", $desc)]
        pub struct $ty;

        impl Mod for $ty {
            fn name(&self) -> &str {
                $name
            }
            fn id(&self) -> &'static str {
                $id
            }
            fn description(&self) -> &str {
                $desc
            }
            fn group(&self) -> &'static str {
                ESSENTIALS
            }
            fn visual_group(&self) -> Option<VisualGroup> {
                Some(VisualGroup::$group)
            }
        }
    };
}

visual_mod!(
    AtmosphereMod,
    "Atmosphere",
    "atmosphere",
    "Sky, clouds, weather, stars, day/night, and fog.",
    Atmosphere
);
visual_mod!(
    PostMod,
    "Post",
    "post",
    "Bloom, god rays, TAA, exposure, vignette, and variable-rate shading.",
    Post
);
visual_mod!(
    LightingMod,
    "Lighting",
    "lighting",
    "Shadows, ambient fill, and block light. Sunlight stays in the core.",
    Lighting
);

#[cfg(test)]
mod tests {
    use super::*;
    use pwc_mod_api::settings::Settings;
    use pwc_mod_api::{annotate_setting, forced_off_marker, GameBuild, ModDescriptor, Mods};

    fn build() -> Mods {
        GameBuild::new()
            .with_mod(ModDescriptor { id: "pwc.visuals", name: "Visuals", version: "1.0.0", register })
            .mods()
    }

    #[test]
    fn registers_three_enabled_essentials_one_per_group() {
        let mods = build();
        let ids: Vec<&str> = (0..mods.len()).map(|i| mods.id(i)).collect();
        assert_eq!(ids, ["atmosphere", "post", "lighting"]);
        let groups: Vec<_> = (0..mods.len()).map(|i| mods.visual_group(i)).collect();
        assert_eq!(groups, [Some(VisualGroup::Atmosphere), Some(VisualGroup::Post), Some(VisualGroup::Lighting)]);
        for i in 0..mods.len() {
            assert!(mods.is_enabled(i), "{} starts on", mods.id(i));
            assert_eq!(mods.group(i), ESSENTIALS);
            assert_eq!(mods.package(i), Some("pwc.visuals"));
            assert!(mods.knobs(i).is_empty());
        }
        let mask = mods.visual_mask();
        assert!(mask.atmosphere && mask.post && mask.lighting);
    }

    #[test]
    fn effective_render_strips_disabled_visual_groups() {
        let mut mods = build();
        mods.set_enabled("Atmosphere", false);
        mods.set_enabled("Post", false);
        mods.set_enabled("Lighting", false);
        let settings = Settings::default();
        let stripped = mods.effective_render(&settings);
        assert!(!stripped.clouds);
        assert!(!stripped.bloom);
        assert!(!stripped.shadows);
        assert!(stripped.sunlight);
        let full = build().effective_render(&settings);
        assert_eq!(full.clouds, settings.clouds);
        assert_eq!(full.bloom, settings.bloom);
        assert_eq!(full.shadows, settings.shadows);
        let via_mask = mods.visual_mask().effective_render(&settings);
        assert!(!via_mask.clouds && !via_mask.bloom && !via_mask.shadows);
        assert_eq!(via_mask.sunlight, stripped.sunlight);
    }

    #[test]
    fn annotate_setting_names_the_mod_that_forced_the_lane_off() {
        let mut mods = build();
        mods.set_enabled("Post", false);
        let mask = mods.visual_mask();
        assert_eq!(forced_off_marker("Post"), "(off: Post mod)");
        assert_eq!(
            annotate_setting("On".to_string(), "bloom", mask),
            format!("On {}", forced_off_marker("Post"))
        );
        assert_eq!(annotate_setting("On".to_string(), "shadows", mask), "On");
        mods.set_enabled("Lighting", false);
        let mask = mods.visual_mask();
        assert_eq!(
            annotate_setting("On".to_string(), "shadows", mask),
            format!("On {}", forced_off_marker("Lighting"))
        );
    }

    #[test]
    fn bench_pin_switches_every_visual_group_off() {
        let mut mods = build();
        mods.apply_bench_env(None, Some(false));
        assert_eq!(mods.visual_mask(), Default::default(), "no pin, no change");
        mods.apply_bench_env(None, Some(true));
        let mask = mods.visual_mask();
        assert!(!mask.atmosphere && !mask.post && !mask.lighting);
    }

    #[test]
    fn choices_persist_each_member_line_and_round_trip() {
        let mut mods = build();
        mods.set_group_enabled(ESSENTIALS, false);
        let text = mods.choices_text();
        for id in ["atmosphere", "post", "lighting"] {
            assert!(text.contains(&format!("{id}=off")), "{id} should be off in:\n{text}");
        }
        assert!(!text.lines().any(|l| l.starts_with("essentials=")), "no group-level key");
        let mut fresh = build();
        fresh.apply_choices_text("version=2\nlighting=off\natmosphere=true\nnot-a-mod=on\npost=nope\n");
        let mask = fresh.visual_mask();
        assert!(mask.atmosphere && mask.post && !mask.lighting);
    }
}
