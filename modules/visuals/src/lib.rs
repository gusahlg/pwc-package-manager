//! The visual modules. The core renderer is sunlight-only; installed, these allow the shipped look.
//!
//! Three modules, one per render group: **Atmosphere** (`atmosphere`), **Post** (`post`) and
//! **Lighting** (`lighting`). Each provides a [`VisualGroup`]; the game ORs the groups of the
//! installed, unsuspended modules into its render mask. A lane whose group nothing provides is forced
//! off, and settings screens mark it unavailable in this build.

use pwc_module_api::render_config::VisualGroup;
use pwc_module_api::{Module, ModuleRegistrar};

/// The package entry point: installs the three visual modules.
pub fn register(registrar: &mut ModuleRegistrar) {
    // Fancy lanes live in modules; a build without this package has the core look.
    registrar.add(AtmosphereModule);
    registrar.add(PostModule);
    registrar.add(LightingModule);
}

macro_rules! visual_module {
    ($ty:ident, $name:literal, $id:literal, $desc:literal, $group:ident) => {
        #[doc = concat!("The ", $name, " module (id `", $id, "`): ", $desc)]
        pub struct $ty;

        impl Module for $ty {
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

visual_module!(
    AtmosphereModule,
    "Atmosphere",
    "atmosphere",
    "Sky, clouds, weather, stars, day/night, and fog.",
    Atmosphere
);
visual_module!(
    PostModule,
    "Post",
    "post",
    "Bloom, god rays, TAA, exposure, vignette, and variable-rate shading.",
    Post
);
visual_module!(
    LightingModule,
    "Lighting",
    "lighting",
    "Shadows, ambient fill, and block light. Sunlight stays in the core.",
    Lighting
);

#[cfg(test)]
mod tests {
    use super::*;
    use pwc_module_api::testing::Harness;
    use pwc_module_api::settings::Settings;
    use pwc_module_api::{GameBuild, ModuleDescriptor, VisualMask};

    fn build() -> Harness {
        Harness::new(GameBuild::new()
            .with_module(ModuleDescriptor { id: "pwc.visuals", name: "Visuals", version: "1.0.0", register }))
    }

    #[test]
    fn registers_three_modules_one_per_group() {
        let modules = build();
        let ids: Vec<&str> = (0..modules.len()).map(|i| modules.id(i)).collect();
        assert_eq!(ids, ["atmosphere", "post", "lighting"]);
        let groups: Vec<_> = (0..modules.len()).map(|i| modules.visual_group(i)).collect();
        assert_eq!(groups, [Some(VisualGroup::Atmosphere), Some(VisualGroup::Post), Some(VisualGroup::Lighting)]);
        for i in 0..modules.len() {
            assert!(modules.is_active(i), "{} runs", modules.id(i));
            assert_eq!(modules.package(i), Some("pwc.visuals"));
        }
        assert_eq!(modules.visual_mask(), VisualMask::ALL, "installed means allowed");
        assert_eq!(modules.visual_packages(), ["pwc.visuals"]);
    }

    #[test]
    fn a_suspended_package_strips_every_group_it_provides() {
        let mut modules = build();
        modules.suspend(&["pwc.visuals"]);
        let settings = Settings::default();
        let stripped = modules.effective_render(&settings);
        assert!(!stripped.clouds && !stripped.bloom && !stripped.shadows);
        assert!(stripped.sunlight);
        assert!(modules.visual_mask().strips("bloom") && !modules.visual_mask().strips("sunlight"));
        let full = build().effective_render(&settings);
        assert_eq!((full.clouds, full.bloom, full.shadows), (settings.clouds, settings.bloom, settings.shadows));
        modules.resume();
        assert_eq!(modules.visual_mask(), VisualMask::ALL);
    }
}
