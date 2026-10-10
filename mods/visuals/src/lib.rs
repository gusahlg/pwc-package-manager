//! The visual mods. The core renderer is sunlight-only; installed, these allow the shipped look.
//!
//! Three mods, one per render group: **Atmosphere** (`atmosphere`), **Post** (`post`) and
//! **Lighting** (`lighting`). Each provides a [`VisualGroup`]; the game ORs the groups of the
//! installed, unsuspended mods into its render mask. A lane whose group nothing provides is forced
//! off, and settings screens mark it unavailable in this build.

use pwc_mod_api::render_config::VisualGroup;
use pwc_mod_api::{Mod, ModRegistrar};

/// The package entry point: installs the three visual mods.
pub fn register(registrar: &mut ModRegistrar) {
    // Fancy lanes live in mods; a build without this package has the core look.
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
    use pwc_mod_api::{GameBuild, ModDescriptor, Mods, VisualMask};

    fn build() -> Mods {
        GameBuild::new()
            .with_mod(ModDescriptor { id: "pwc.visuals", name: "Visuals", version: "1.0.0", register })
            .mods()
    }

    #[test]
    fn registers_three_mods_one_per_group() {
        let mods = build();
        let ids: Vec<&str> = (0..mods.len()).map(|i| mods.id(i)).collect();
        assert_eq!(ids, ["atmosphere", "post", "lighting"]);
        let groups: Vec<_> = (0..mods.len()).map(|i| mods.visual_group(i)).collect();
        assert_eq!(groups, [Some(VisualGroup::Atmosphere), Some(VisualGroup::Post), Some(VisualGroup::Lighting)]);
        for i in 0..mods.len() {
            assert!(mods.is_active(i), "{} runs", mods.id(i));
            assert_eq!(mods.package(i), Some("pwc.visuals"));
        }
        assert_eq!(mods.visual_mask(), VisualMask::ALL, "installed means allowed");
        assert_eq!(mods.visual_packages(), ["pwc.visuals"]);
    }

    #[test]
    fn a_suspended_package_strips_every_group_it_provides() {
        let mut mods = build();
        mods.suspend_packages(&["pwc.visuals".to_string()]);
        let settings = Settings::default();
        let stripped = mods.effective_render(&settings);
        assert!(!stripped.clouds && !stripped.bloom && !stripped.shadows);
        assert!(stripped.sunlight);
        assert!(mods.visual_mask().strips("bloom") && !mods.visual_mask().strips("sunlight"));
        let full = build().effective_render(&settings);
        assert_eq!((full.clouds, full.bloom, full.shadows), (settings.clouds, settings.bloom, settings.shadows));
        mods.resume_packages();
        assert_eq!(mods.visual_mask(), VisualMask::ALL);
    }
}
