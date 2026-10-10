//! The pause menu: what Esc opens in a world. Resume, the entries other packages offer for the
//! pause screen (a settings menu, a mod list), and Leave World. The world keeps running
//! underneath; the core gives this screen the input and saves on leaving. It names no package.

use pwc_mod_api::screen::{AppRequest, Places, Screen, ScreenContext, ScreenFacts, ScreenOutcome};
use pwc_mod_api::{Mod, ModRegistrar};
use pwc_ui_kit::{Framed, Menu, Msg, Row, Style, View};

/// The package entry point: installs [`PauseMenuMod`].
pub fn register(registrar: &mut ModRegistrar) {
    registrar.add(PauseMenuMod);
}

/// The pause-menu mod (id `pause_menu`): answers the pause screen slot.
pub struct PauseMenuMod;

impl Mod for PauseMenuMod {
    fn name(&self) -> &str {
        "Pause menu"
    }

    fn id(&self) -> &'static str {
        "pause_menu"
    }

    fn pause_screen(&self, _facts: &ScreenFacts) -> Option<Box<dyn Screen>> {
        Some(Framed::boxed(PauseMenu))
    }
}

/// What a row of the pause menu does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseAction {
    /// Close the menu and play on.
    Resume,
    /// Open a registered entry, by id.
    Entry(&'static str),
    /// Save and leave the world.
    Leave,
}

/// Resume, the pause entries, Leave World.
pub struct PauseMenu;

impl Menu for PauseMenu {
    type Action = PauseAction;

    fn view(&self, ctx: &ScreenContext) -> View<PauseAction> {
        let mut rows = vec![Row::action("Resume", PauseAction::Resume)];
        rows.extend(ctx.facts.entries_for(Places::PAUSE).map(|e| Row::action(e.label, PauseAction::Entry(e.id))));
        rows.push(Row::action("Leave World", PauseAction::Leave).detail("saves the world first"));
        View {
            title: "PAUSED".to_string(),
            style: Style::Panel,
            rows,
            default: None,
            hint: "Up/Down select   Enter choose   Esc resume".to_string(),
            notice: None,
        }
    }

    fn update(&mut self, msg: Msg<PauseAction>, _ctx: &mut ScreenContext) -> ScreenOutcome {
        match msg {
            Msg::Pick(PauseAction::Resume) => ScreenOutcome::Request(AppRequest::Resume),
            Msg::Pick(PauseAction::Entry(id)) => ScreenOutcome::Open(id),
            Msg::Pick(PauseAction::Leave) => ScreenOutcome::Request(AppRequest::LeaveWorld),
            Msg::Back => ScreenOutcome::Back,
            _ => ScreenOutcome::Stay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwc_mod_api::testing::Harness;
    use pwc_mod_api::screen::ScreenEntry;
    use pwc_mod_api::{GameBuild, ModDescriptor};
    use pwc_ui_kit::testing::Fixture;

    fn nothing(_facts: &ScreenFacts) -> Box<dyn Screen> {
        unreachable!("the core opens entries")
    }

    const ENTRIES: &[ScreenEntry] = &[
        ScreenEntry { id: "t.mods", label: "Mods", places: Places::BOTH, order: 10, open: nothing },
        ScreenEntry { id: "t.settings", label: "Settings", places: Places::BOTH, order: 20, open: nothing },
        ScreenEntry { id: "t.worlds", label: "Worlds", places: Places::MAIN, order: 0, open: nothing },
    ];

    fn request(outcome: ScreenOutcome) -> Option<AppRequest> {
        match outcome {
            ScreenOutcome::Request(r) => Some(r),
            _ => None,
        }
    }

    #[test]
    fn the_pause_menu_resumes_opens_pause_entries_and_leaves() {
        let mut f = Fixture::new();
        f.in_world = true;
        f.entries = ENTRIES.to_vec();
        let mut menu = PauseMenu;
        let view = menu.view(&f.ctx());
        let labels: Vec<&str> = view.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["Resume", "Mods", "Settings", "Leave World"], "pause entries only, in order");
        assert_eq!(view.title, "PAUSED");
        assert_eq!(request(menu.update(Msg::Pick(PauseAction::Resume), &mut f.ctx())), Some(AppRequest::Resume));
        assert_eq!(request(menu.update(Msg::Pick(PauseAction::Leave), &mut f.ctx())), Some(AppRequest::LeaveWorld));
        assert!(matches!(menu.update(Msg::Pick(PauseAction::Entry("t.settings")), &mut f.ctx()), ScreenOutcome::Open("t.settings")));
        assert!(matches!(menu.update(Msg::Back, &mut f.ctx()), ScreenOutcome::Back), "Esc resumes through the core");
    }

    #[test]
    fn the_package_answers_the_pause_slot() {
        let f = Fixture::new();
        let mut mods = Harness::new(GameBuild::new()
            .with_mod(ModDescriptor { id: "pwc.pause-menu", name: "Pause menu", version: "1.0.0", register }));
        assert!(mods.pause_screen(&f.facts()).is_some());
        assert!(mods.root_screen(&f.facts()).is_none());
        mods.suspend(&["pwc.pause-menu"]);
        assert!(mods.pause_screen(&f.facts()).is_none(), "suspended: Esc leaves the world as before");
    }
}
