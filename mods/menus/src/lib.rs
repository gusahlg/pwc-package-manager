//! `pwc.menus`: the standard look for every screen outside a world.
//!
//! The title screen, the settings screens, the mods list, the saved-worlds page and the host and
//! join forms are all drawn through a menu theme. This package supplies PWC's standard one, the
//! API's [`DefaultTheme`], by returning it from [`Mod::menu_theme`].
//!
//! A theme decides presentation and nothing more: where each row sits on screen and how it is
//! painted. It never sees menu input or menu state. What a screen lists and how it answers keys
//! stays in the game's menu code, so no theme can get in the way of navigation.
//!
//! When no enabled mod offers a theme, the game falls back to its built-in one, which is this same
//! [`DefaultTheme`]; switching this package off therefore leaves every screen looking the same.
//! Another package may offer a different theme instead. If several enabled mods offer one, the
//! first of them in registration order is used.

use pwc_mod_api::menu::theme::{DefaultTheme, MenuTheme};
use pwc_mod_api::{Mod, ModRegistrar};

/// The theme this package hands out. It is a stateless unit value, so one static instance serves
/// every frame.
static STANDARD_THEME: DefaultTheme = DefaultTheme;

/// The package entry point: installs [`Menus`], enabled.
pub fn register(registrar: &mut ModRegistrar) {
    registrar.add(Menus);
}

/// The mod that selects the standard menu theme (id `menus`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Menus;

impl Mod for Menus {
    fn name(&self) -> &str {
        "Menus"
    }

    fn id(&self) -> &'static str {
        "menus"
    }

    fn menu_theme(&self) -> Option<&dyn MenuTheme> {
        Some(&STANDARD_THEME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwc_mod_api::menu::theme::RowRect;
    use pwc_mod_api::menu::{present, Row, Style, View};
    use pwc_mod_api::{GameBuild, ModDescriptor, Mods};

    fn build() -> Mods {
        GameBuild::new()
            .with_mod(ModDescriptor { id: "pwc.menus", name: "Menus", version: "1.0.0", register })
            .mods()
    }

    fn sample_view() -> View<u8> {
        View {
            title: "SAMPLE".to_string(),
            style: Style::Title { subtitle: "a test screen".to_string() },
            rows: vec![Row::heading("Section"), Row::action("First", 1), Row::action("Second", 2).detail("more")],
            default: None,
            hint: "Up/Down select".to_string(),
            notice: None,
        }
    }

    fn layout(theme: &dyn MenuTheme) -> Vec<RowRect> {
        theme.layout(&present(&sample_view(), 1.0), 1280, 720)
    }

    #[test]
    fn registers_one_mod_named_menus() {
        let mods = build();
        assert_eq!(mods.len(), 1);
        assert_eq!((mods.id(0), mods.name(0)), ("menus", "Menus"));
        assert_eq!(mods.package(0), Some("pwc.menus"));
    }

    #[test]
    fn the_theme_is_offered_unless_suspended() {
        let mut mods = build();
        assert!(mods.menu_theme().is_some());
        mods.suspend_packages(&["pwc.menus".to_string()]);
        assert!(mods.menu_theme().is_none(), "suspended: the game draws its built-in theme");
        mods.resume_packages();
        assert!(mods.menu_theme().is_some());
    }

    #[test]
    fn the_theme_lays_out_every_row_like_the_built_in_fallback() {
        let mods = build();
        let ours = layout(mods.menu_theme().expect("enabled"));
        assert_eq!(ours.len(), sample_view().rows.len(), "one rectangle per row");
        assert_eq!(ours, layout(&DefaultTheme), "switching the mod off does not move anything");
        assert!(ours.windows(2).all(|w| w[0].y < w[1].y), "rows stack top to bottom: {ours:?}");
    }
}
