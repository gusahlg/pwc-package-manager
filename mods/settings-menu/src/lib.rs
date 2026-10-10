//! The settings menu: a hub with one page per settings category, and on each page every tunable of
//! that category, the core's settings and every package's options alike, through the core's
//! options registry. It names no package: a package's option appears because the package declared
//! it, and this menu appears because it registered an entry for the main menu and the pause screen.

use pwc_mod_api::screen::{Places, Screen, ScreenContext, ScreenEntry, ScreenFacts, ScreenOutcome};
use pwc_mod_api::settings::options::CORE;
use pwc_mod_api::settings::{Applies, Category, MenuKind};
use pwc_mod_api::ModRegistrar;
use pwc_ui_kit::{Framed, Menu, Msg, Notice, Row, Style, ValueView, View};

/// The entry this package offers.
pub const ENTRY: ScreenEntry =
    ScreenEntry { id: "pwc.settings-menu", label: "Settings", places: Places::BOTH, order: 20, open: open_hub };

/// Shown after the value of a lane whose visual group no installed, unsuspended package provides.
pub const UNAVAILABLE: &str = "(unavailable in this build)";

/// The package entry point: offers "Settings" on the main menu and the pause screen.
pub fn register(registrar: &mut ModRegistrar) {
    registrar.screen_entry(ENTRY);
}

fn open_hub(_facts: &ScreenFacts) -> Box<dyn Screen> {
    Framed::boxed(SettingsHub)
}

/// The settings hub: one row per [`Category`], each pushing its page.
pub struct SettingsHub;

impl Menu for SettingsHub {
    type Action = Category;

    fn view(&self, ctx: &ScreenContext) -> View<Category> {
        let rows = Category::ALL.iter().map(|(c, name)| Row::action(*name, *c)).collect();
        View {
            title: "SETTINGS".to_string(),
            style: Style::Panel,
            rows,
            default: None,
            hint: "Enter open   Esc back".to_string(),
            notice: ctx.settings().vram_notice.as_ref().map(|t| Notice::info(t.clone())),
        }
    }

    fn update(&mut self, msg: Msg<Category>, _ctx: &mut ScreenContext) -> ScreenOutcome {
        match msg {
            Msg::Pick(cat) => ScreenOutcome::Push(Framed::boxed(SettingsPage::new(cat))),
            Msg::Back => ScreenOutcome::Back,
            _ => ScreenOutcome::Stay,
        }
    }
}

/// One category's page: every entry of the options view on that page, in view order (the core's
/// settings first, then each package's options).
pub struct SettingsPage {
    category: Category,
}

impl SettingsPage {
    pub fn new(category: Category) -> Self {
        Self { category }
    }
}

impl Menu for SettingsPage {
    type Action = usize;

    fn view(&self, ctx: &ScreenContext) -> View<usize> {
        let title = Category::ALL.iter().find(|(c, _)| *c == self.category).map_or("SETTINGS", |(_, n)| n).to_uppercase();
        let options = ctx.options();
        let rows = (0..options.len())
            .filter(|&i| options.info(i).page == self.category)
            .map(|i| {
                let info = options.info(i);
                let stored = options.show(i);
                // A core lane is unavailable when no installed package provides its visual group.
                let stripped = info.owner == CORE && ctx.facts.visuals.strips(info.key);
                let shown = if stripped { format!("{stored} {UNAVAILABLE}") } else { stored };
                let value = match info.menu_kind {
                    MenuKind::Toggle if !stripped => ValueView::Toggle(options.toggled(i).unwrap_or(false)),
                    MenuKind::Toggle | MenuKind::Choice => ValueView::Choice(shown),
                    MenuKind::Bar => ValueView::Bar { t: options.fraction(i), label: shown },
                };
                let row = Row::value(info.label, value, i);
                match info.applies {
                    Applies::NextWorld => row.detail("next new world"),
                    Applies::Live => row,
                }
            })
            .collect();
        View {
            title,
            style: Style::Panel,
            rows,
            default: None,
            hint: "Left/Right or h/l change   Esc back".to_string(),
            notice: ctx.settings().vram_notice.as_ref().map(|t| Notice::info(t.clone())),
        }
    }

    fn update(&mut self, msg: Msg<usize>, ctx: &mut ScreenContext) -> ScreenOutcome {
        match msg {
            Msg::Step(i, dir) => {
                ctx.options_mut().step(i, dir.delta());
                ScreenOutcome::Stay
            }
            Msg::Back => ScreenOutcome::Back,
            _ => ScreenOutcome::Stay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwc_mod_api::render_config::{lane_group, VisualGroup};
    use pwc_mod_api::settings::OptionSpec;
    use pwc_mod_api::{GameBuild, ModDescriptor, VisualMask};
    use pwc_ui_kit::testing::Fixture;
    use pwc_ui_kit::{Dir, RowKind};

    fn page(f: &mut Fixture, category: Category) -> View<usize> {
        SettingsPage::new(category).view(&f.ctx())
    }

    fn shown(row: &Row<usize>) -> &str {
        match &row.kind {
            RowKind::Value(ValueView::Choice(s) | ValueView::Bar { label: s, .. }) => s,
            RowKind::Value(ValueView::Toggle(true)) => "On",
            RowKind::Value(ValueView::Toggle(false)) => "Off",
            _ => "",
        }
    }

    #[test]
    fn the_package_offers_settings_on_the_main_menu_and_the_pause_screen() {
        let mods = GameBuild::new()
            .with_mod(ModDescriptor { id: "pwc.settings-menu", name: "Settings menu", version: "1.0.0", register })
            .mods();
        assert_eq!(mods.len(), 0, "an entry, no mod");
        let entries = mods.screen_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!((entries[0].id, entries[0].label), ("pwc.settings-menu", "Settings"));
        assert!(entries[0].places.contains(Places::MAIN) && entries[0].places.contains(Places::PAUSE));
    }

    #[test]
    fn the_hub_lists_the_pages_and_pushes_one() {
        let mut f = Fixture::new();
        let hub = SettingsHub.view(&f.ctx());
        let labels: Vec<&str> = hub.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["Performance", "Video", "World", "Interface", "Audio"]);
        assert!(matches!(SettingsHub.update(Msg::Pick(Category::Video), &mut f.ctx()), ScreenOutcome::Push(_)));
        assert!(matches!(SettingsHub.update(Msg::Back, &mut f.ctx()), ScreenOutcome::Back));
        f.settings.vram_notice = Some("short of VRAM".into());
        assert_eq!(SettingsHub.view(&f.ctx()).notice.map(|n| n.text).as_deref(), Some("short of VRAM"));
    }

    /// Core settings and a package's option share a page; stepping either moves the revision the
    /// core saves on.
    #[test]
    fn pages_list_core_settings_and_package_options_and_step_them() {
        let mut f = Fixture::new();
        let relief = f.options.declare("pwc.worldgen", OptionSpec::percent("relief", "Relief", Category::World, (25, 300, 25), 100).next_world());
        let world = page(&mut f, Category::World);
        let at = world.rows.iter().position(|r| r.label == "Relief").expect("the option is on the World page");
        assert!(world.rows.iter().position(|r| r.label == "Render Distance").expect("a core row") < at, "core settings first");
        assert_eq!(world.rows[at].detail.as_deref(), Some("next new world"));
        assert_eq!(shown(&world.rows[at]), "100%");
        let tag = world.rows[at].tag.expect("selectable");
        let before = f.options.revision();
        SettingsPage::new(Category::World).update(Msg::Step(tag, Dir::Next), &mut f.ctx());
        assert_eq!(f.options.int(relief), 125);
        assert_ne!(f.options.revision(), before);
        let fov = page(&mut f, Category::Interface).rows.into_iter().find(|r| r.label == "FOV").expect("a core row");
        let before = f.options.revision();
        SettingsPage::new(Category::Interface).update(Msg::Step(fov.tag.unwrap(), Dir::Prev), &mut f.ctx());
        assert_eq!(f.settings.fov, 85.0);
        assert_ne!(f.options.revision(), before);
        assert!(page(&mut f, Category::Video).rows.iter().all(|r| r.label != "Relief"));
    }

    /// The rows marked unavailable are exactly the lanes the renderer strips, and the marker names
    /// no package.
    #[test]
    fn rows_mark_exactly_the_lanes_no_installed_package_provides() {
        let mut f = Fixture::new();
        f.visuals = VisualMask::of([VisualGroup::Atmosphere, VisualGroup::Lighting]);
        let mut lanes = 0;
        for (category, _) in Category::ALL {
            for row in page(&mut f, category).rows {
                let key = f.ctx().options().info(row.tag.unwrap()).key;
                assert_eq!(shown(&row).ends_with(UNAVAILABLE), f.visuals.strips(key), "{key}: {:?}", shown(&row));
                lanes += lane_group(key).is_some() as usize;
            }
        }
        assert!(lanes > 0, "the pages list the visual lanes");
        let bloom = page(&mut f, Category::Video).rows.into_iter().find(|r| r.label == "Bloom").unwrap();
        assert_eq!(shown(&bloom), "On (unavailable in this build)");
    }
}
