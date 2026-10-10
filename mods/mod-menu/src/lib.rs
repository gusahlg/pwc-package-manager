//! The mod menu: the packages compiled into this build, read-only. It reads the list the builder
//! generated ([`ModRegistrar::build`]'s [`BuildInfo`], handed to screens as
//! [`ScreenFacts::build`]), groups packages under the bundles that include them
//! ([`bundles_of`]), and marks the ones the server suspended for this session. Nothing on it
//! switches anything: the build decides what is in. It names no package, no package depends on it,
//! and it depends on none but the UI kit.

use pwc_mod_api::screen::{Places, Screen, ScreenContext, ScreenEntry, ScreenFacts, ScreenOutcome};
use pwc_mod_api::{bundles_of, BuildInfo, ModRegistrar, PackageInfo, PackageKind};
use pwc_ui_kit::{Framed, Menu, Msg, Notice, Row, Style, View};

/// The entry this package offers.
pub const ENTRY: ScreenEntry = ScreenEntry { id: "pwc.mod-menu", label: "Mods", places: Places::BOTH, order: 10, open };

/// The persistent notice: how the set of mods changes.
pub const MODS_NOTICE: &str = "Mods are compiled in: change them with `pwc mod add` / `pwc mod remove` and `pwc build`.";

/// The package entry point: offers "Mods" on the main menu and the pause screen.
pub fn register(registrar: &mut ModRegistrar) {
    registrar.screen_entry(ENTRY);
}

fn open(_facts: &ScreenFacts) -> Box<dyn Screen> {
    Framed::boxed(ModsMenu::default())
}

/// The package list. Rows are the build's index of each package, so picking one changes nothing.
#[derive(Default)]
pub struct ModsMenu {
    /// The package the highlight is on, whose dependencies and bundles the notice spells out.
    hovered: Option<usize>,
}

/// Bundles no other bundle includes, in build order: the sections of the page.
fn top_bundles(packages: &[PackageInfo]) -> Vec<&PackageInfo> {
    packages
        .iter()
        .filter(|p| p.kind == PackageKind::Bundle && bundles_of(packages, p.id).is_empty())
        .collect()
}

/// The display name of the package `id` in `build`, or the id itself.
fn name_of<'a>(build: &'a BuildInfo, id: &'a str) -> &'a str {
    build.package(id).map_or(id, |p| p.name)
}

/// "needs: …" and "part of: …" for one package, or `None` when it has neither.
fn relations(build: &BuildInfo, package: &PackageInfo) -> Option<String> {
    let needs: Vec<&str> = package.dependencies.iter().map(|id| name_of(build, id)).collect();
    let part_of: Vec<&str> = bundles_of(build.packages(), package.id).iter().map(|b| b.name).collect();
    let mut parts = Vec::new();
    if !needs.is_empty() {
        parts.push(format!("needs: {}", needs.join(", ")));
    }
    if !part_of.is_empty() {
        parts.push(format!("part of: {}", part_of.join(", ")));
    }
    (!parts.is_empty()).then(|| format!("{}: {}", package.name, parts.join(" · ")))
}

fn package_row(facts: &ScreenFacts, i: usize, package: &PackageInfo) -> Row<usize> {
    let label = if facts.is_suspended(package.id) {
        format!("  {} {} (off on this server)", package.name, package.version)
    } else {
        format!("  {} {}", package.name, package.version)
    };
    let row = Row::action(label, i);
    if package.description.is_empty() { row } else { row.detail(package.description) }
}

impl Menu for ModsMenu {
    type Action = usize;

    fn view(&self, ctx: &ScreenContext) -> View<usize> {
        let facts = &ctx.facts;
        let packages = facts.build.packages();
        let listed = |p: &PackageInfo| p.kind != PackageKind::Bundle;
        let mut placed = vec![false; packages.len()];
        let mut rows = Vec::new();
        for bundle in top_bundles(packages) {
            let members: Vec<usize> = (0..packages.len())
                .filter(|&i| !placed[i] && listed(&packages[i]))
                .filter(|&i| bundles_of(packages, packages[i].id).iter().any(|b| b.id == bundle.id))
                .collect();
            if members.is_empty() {
                continue;
            }
            let heading = Row::heading(bundle.name);
            rows.push(if bundle.description.is_empty() { heading } else { heading.detail(bundle.description) });
            for i in members {
                placed[i] = true;
                rows.push(package_row(facts, i, &packages[i]));
            }
        }
        let other: Vec<usize> = (0..packages.len()).filter(|&i| !placed[i] && listed(&packages[i])).collect();
        if !other.is_empty() && !rows.is_empty() {
            rows.push(Row::heading("Other"));
        }
        for i in other {
            rows.push(package_row(facts, i, &packages[i]));
        }
        let mut notice = MODS_NOTICE.to_string();
        if let Some(line) = self.hovered.and_then(|i| packages.get(i)).and_then(|p| relations(facts.build, p)) {
            notice = format!("{line}\n{notice}");
        }
        View {
            title: "MODS".to_string(),
            style: Style::Panel,
            rows,
            default: None,
            hint: "Up/Down browse   Esc back".to_string(),
            notice: Some(Notice::info(notice)),
        }
    }

    fn update(&mut self, msg: Msg<usize>, _ctx: &mut ScreenContext) -> ScreenOutcome {
        match msg {
            Msg::Hover(i) | Msg::Pick(i) => {
                self.hovered = Some(i);
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
    use pwc_mod_api::{GameBuild, ModDescriptor};
    use pwc_ui_kit::testing::Fixture;
    use pwc_ui_kit::RowKind;

    const fn package(
        id: &'static str,
        name: &'static str,
        kind: PackageKind,
        dependencies: &'static [&'static str],
    ) -> PackageInfo {
        PackageInfo { id, name, version: "1.0.0", description: "", kind, dependencies, register: None }
    }

    /// A build as the builder lists it: a library, mods, a nested bundle, a top bundle, an
    /// unbundled mod.
    static PACKAGES: &[PackageInfo] = &[
        package("t.kit", "UI kit", PackageKind::Library, &[]),
        PackageInfo { description: "Text chat.", ..package("t.chat", "Chat", PackageKind::Mod, &["t.kit"]) },
        package("t.commands", "Commands", PackageKind::Mod, &["t.chat"]),
        package("t.chat-commands", "Chat and commands", PackageKind::Bundle, &["t.chat", "t.commands"]),
        package("t.hotbar", "Hotbar", PackageKind::Mod, &[]),
        PackageInfo { description: "Everything.", ..package("t.essentials", "Essentials", PackageKind::Bundle, &["t.chat-commands", "t.hotbar"]) },
        package("t.toolkit", "Toolkit", PackageKind::Mod, &["t.commands"]),
    ];

    fn fixture() -> Fixture {
        let mut f = Fixture::new();
        f.build = GameBuild::from_static("sha256:00", PACKAGES).info().clone();
        f
    }

    fn labels(view: &View<usize>) -> Vec<&str> {
        view.rows.iter().map(|r| r.label.as_str()).collect()
    }

    #[test]
    fn the_package_offers_mods_on_the_main_menu_and_the_pause_screen() {
        let mods = GameBuild::new()
            .with_mod(ModDescriptor { id: "pwc.mod-menu", name: "Mod menu", version: "1.0.0", register })
            .mods();
        let entries = mods.screen_entries();
        assert_eq!((entries.len(), entries[0].label, entries[0].places), (1, "Mods", Places::BOTH));
    }

    /// Bundles head their members, each package is listed once, and what no bundle includes is
    /// "Other". Nothing on the page toggles anything.
    #[test]
    fn packages_are_grouped_under_the_bundles_that_include_them() {
        let mut f = fixture();
        let view = ModsMenu::default().view(&f.ctx());
        assert_eq!(
            labels(&view),
            ["Essentials", "  Chat 1.0.0", "  Commands 1.0.0", "  Hotbar 1.0.0", "Other", "  UI kit 1.0.0", "  Toolkit 1.0.0"]
        );
        assert!(matches!(view.rows[0].kind, RowKind::Heading));
        assert_eq!(view.rows[0].detail.as_deref(), Some("Everything."));
        assert_eq!(view.rows[1].detail.as_deref(), Some("Text chat."));
        assert!(view.rows.iter().all(|r| matches!(r.kind, RowKind::Heading | RowKind::Action)), "no switches");
        assert_eq!(view.notice.as_ref().map(|n| n.text.as_str()), Some(MODS_NOTICE));
        let mut f = Fixture::new();
        f.build = GameBuild::from_static("sha256:00", &PACKAGES[4..5]).info().clone();
        assert_eq!(labels(&ModsMenu::default().view(&f.ctx())), ["  Hotbar 1.0.0"], "no bundles: a plain list");
    }

    #[test]
    fn the_highlighted_package_shows_what_it_needs_and_belongs_to() {
        let mut f = fixture();
        let mut menu = ModsMenu::default();
        let chat = labels(&menu.view(&f.ctx())).iter().position(|l| l.contains("Chat 1.0.0")).unwrap();
        let tag = menu.view(&f.ctx()).rows[chat].tag.unwrap();
        assert!(matches!(menu.update(Msg::Hover(tag), &mut f.ctx()), ScreenOutcome::Stay));
        let notice = menu.view(&f.ctx()).notice.unwrap().text;
        assert_eq!(notice, format!("Chat: needs: UI kit · part of: Chat and commands, Essentials\n{MODS_NOTICE}"));
        assert!(matches!(menu.update(Msg::Pick(tag), &mut f.ctx()), ScreenOutcome::Stay), "picking changes nothing");
        assert!(matches!(menu.update(Msg::Back, &mut f.ctx()), ScreenOutcome::Back));
    }

    #[test]
    fn a_package_the_server_suspended_is_marked_off() {
        let mut f = fixture();
        f.suspended = vec!["t.toolkit".to_string()];
        let view = ModsMenu::default().view(&f.ctx());
        assert!(labels(&view).contains(&"  Toolkit 1.0.0 (off on this server)"));
        assert!(labels(&view).contains(&"  Hotbar 1.0.0"));
    }
}
