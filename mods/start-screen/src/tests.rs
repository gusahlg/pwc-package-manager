//! The start screen's pages, forms and requests, driven the way the core drives a root screen.

use super::*;
use pwc_mod_api::screen::{SaveError, SaveMeta, ScreenEntry, Slot};
use pwc_mod_api::{GameBuild, ModDescriptor, Mods};
use pwc_ui_kit::testing::Fixture;
use pwc_ui_kit::{Dir, RowKind, TextOp};

/// This package alone, registered the way a PWC build registers it.
fn build() -> Mods {
    GameBuild::new()
        .with_mod(ModDescriptor { id: "pwc.start-screen", name: "Start screen", version: "2.0.0", register })
        .mods()
}

fn nothing(_facts: &ScreenFacts) -> Box<dyn Screen> {
    unreachable!("the core opens entries; the start screen only names them")
}

/// Entries other packages offer: two for the main menu, one only for the pause screen.
const ENTRIES: &[ScreenEntry] = &[
    ScreenEntry { id: "test.mods", label: "Mods", places: Places::BOTH, order: 10, open: nothing },
    ScreenEntry { id: "test.settings", label: "Settings", places: Places::MAIN, order: 20, open: nothing },
    ScreenEntry { id: "test.photo", label: "Photo mode", places: Places::PAUSE, order: 5, open: nothing },
];

/// A readable save slot whose display name matches its id.
fn slot(name: &str, playtime_secs: u64, edit_count: u32) -> Slot {
    Slot {
        id: SlotId::new(name).expect("legal slot id"),
        meta: Ok(SaveMeta { name: name.to_string(), seed: 1, created: 0, last_played: 10, playtime_secs, edit_count }),
    }
}

fn damaged(id: &str) -> Slot {
    Slot { id: SlotId::new(id).expect("legal slot id"), meta: Err(SaveError::Corrupt("truncated")) }
}

fn remembered(address: &str, port: &str, name: &str) -> Session {
    Session { address: address.into(), port: port.into(), name: name.into() }
}

/// Facts with these saves, this session and notice, and the sample entries.
fn fixture(saves: Vec<Slot>, session: Session, notice: Option<&str>) -> Fixture {
    let mut f = Fixture::new();
    f.saves = saves;
    f.session = session;
    f.notice = notice.map(str::to_string);
    f.entries = ENTRIES.to_vec();
    f
}

fn open(f: &Fixture) -> DefaultStart {
    DefaultStart::open(&f.facts())
}

fn page(screen: &DefaultStart, f: &mut Fixture) -> PresentedView {
    screen.presented(&f.ctx()).0
}

fn labels(screen: &DefaultStart, f: &mut Fixture) -> Vec<String> {
    page(screen, f).rows.into_iter().map(|r| r.label).collect()
}

fn step(screen: &mut DefaultStart, f: &mut Fixture, intents: &[Intent]) -> ScreenOutcome {
    screen.update_intents(intents, &mut f.ctx())
}

fn request(outcome: ScreenOutcome) -> Option<AppRequest> {
    match outcome {
        ScreenOutcome::Request(request) => Some(request),
        _ => None,
    }
}

fn opened(outcome: ScreenOutcome) -> Option<&'static str> {
    match outcome {
        ScreenOutcome::Open(id) => Some(id),
        _ => None,
    }
}

fn down(n: usize) -> Vec<Intent> {
    vec![Intent::Nav(Dir::Next); n]
}

#[test]
fn the_main_menu_is_short_lists_worlds_behind_one_row_and_offers_the_main_entries() {
    let mut f = fixture(vec![slot("alpha", 3661, 7), damaged("broken")], Session::default(), Some("could not join: refused"));
    let screen = open(&f);
    assert_eq!(
        labels(&screen, &mut f),
        ["New World", "Worlds", "Host Server", "Join Server", "Mods", "Settings", "Quit"],
        "main-menu entries in their order, the pause-only one left out"
    );
    let main = page(&screen, &mut f);
    assert_eq!(main.rows[1].detail.as_deref(), Some("2 saved"));
    assert_eq!(main.notice.as_ref().map(|n| n.text.as_str()), Some("could not join: refused"));
    assert!(matches!(main.style, Style::Title { .. }));
    assert_eq!(main.title, "PROJECT WATT CUBED");
    let mut one = fixture(vec![slot("alpha", 0, 0)], Session::default(), None);
    assert_eq!(page(&open(&one), &mut one).rows[1].detail.as_deref(), Some("1 saved"));
    let mut none = fixture(Vec::new(), Session::default(), None);
    assert_eq!(page(&open(&none), &mut none).rows[1].detail.as_deref(), Some("none saved yet"));
    let mut bare = Fixture::new();
    assert_eq!(labels(&open(&bare), &mut bare), ["New World", "Worlds", "Host Server", "Join Server", "Quit"], "no entries, no rows");
}

#[test]
fn picks_ask_the_core_or_open_entries_by_id() {
    let mut f = fixture(vec![slot("alpha", 0, 0)], Session::default(), Some("old news"));
    let mut screen = open(&f);
    assert_eq!(request(step(&mut screen, &mut f, &[Intent::Confirm])), Some(AppRequest::NewWorld));
    assert!(page(&screen, &mut f).notice.is_none(), "any pick clears the notice");
    for (steps, entry) in [(4, "test.mods"), (5, "test.settings")] {
        let mut screen = open(&f);
        step(&mut screen, &mut f, &down(steps));
        assert_eq!(opened(step(&mut screen, &mut f, &[Intent::Confirm])), Some(entry));
    }
    let mut screen = open(&f);
    step(&mut screen, &mut f, &down(6));
    assert_eq!(request(step(&mut screen, &mut f, &[Intent::Confirm])), Some(AppRequest::Quit));
    let mut screen = open(&f);
    assert!(matches!(step(&mut screen, &mut f, &[Intent::Cancel]), ScreenOutcome::Stay), "Esc on the root does nothing");
}

#[test]
fn the_worlds_page_deletes_on_d_then_enter_and_any_other_key_cancels() {
    let saves = vec![slot("alpha", 60, 1), slot("beta", 60, 1)];
    let ids: Vec<SlotId> = saves.iter().map(|s| s.id.clone()).collect();
    let mut f = fixture(saves, Session::default(), None);
    let worlds = |screen: &mut DefaultStart, f: &mut Fixture| {
        step(screen, f, &down(1));
        step(screen, f, &[Intent::Confirm]);
        assert_eq!(page(screen, f).title, "WORLDS");
    };
    let d = || Intent::Edit(TextOp::Char('d'));

    // D asks, a short confirm line takes the row, Enter deletes that world.
    let mut screen = open(&f);
    worlds(&mut screen, &mut f);
    assert!(matches!(step(&mut screen, &mut f, &[d()]), ScreenOutcome::Stay));
    let worlds_page = page(&screen, &mut f);
    assert_eq!(labels(&screen, &mut f), ["Delete alpha?", "Load: beta", "Back"]);
    assert_eq!(worlds_page.rows[0].detail.as_deref(), Some("Enter to delete · any other key cancels"));
    assert_eq!(request(step(&mut screen, &mut f, &[Intent::Confirm])), Some(AppRequest::Delete(ids[0].clone())));

    // Esc cancels the question and stays on the page; Enter then loads.
    let mut screen = open(&f);
    worlds(&mut screen, &mut f);
    step(&mut screen, &mut f, &[d()]);
    assert!(matches!(step(&mut screen, &mut f, &[Intent::Cancel]), ScreenOutcome::Stay));
    assert_eq!(labels(&screen, &mut f), ["Load: alpha", "Load: beta", "Back"]);
    assert_eq!(request(step(&mut screen, &mut f, &[Intent::Confirm])), Some(AppRequest::Load(ids[0].clone())));

    // Moving away cancels; another letter cancels.
    let mut screen = open(&f);
    worlds(&mut screen, &mut f);
    step(&mut screen, &mut f, &[d()]);
    step(&mut screen, &mut f, &down(1));
    assert_eq!(labels(&screen, &mut f), ["Load: alpha", "Load: beta", "Back"]);
    step(&mut screen, &mut f, &[d()]);
    step(&mut screen, &mut f, &[Intent::Edit(TextOp::Char('x'))]);
    assert_eq!(request(step(&mut screen, &mut f, &[Intent::Confirm])), Some(AppRequest::Load(ids[1].clone())));
}

#[test]
fn the_worlds_page_lists_saves_loads_on_enter_and_backs_out() {
    let saves = vec![slot("alpha", 3661, 7), damaged("broken")];
    let ids: Vec<SlotId> = saves.iter().map(|s| s.id.clone()).collect();
    let mut f = fixture(saves, Session::default(), None);
    let mut screen = open(&f);
    // New World -> Worlds, open the page.
    step(&mut screen, &mut f, &down(1));
    assert!(matches!(step(&mut screen, &mut f, &[Intent::Confirm]), ScreenOutcome::Stay));
    let worlds_page = page(&screen, &mut f);
    assert_eq!(worlds_page.title, "WORLDS");
    assert_eq!(labels(&screen, &mut f), ["Load: alpha", "Load: broken (damaged)", "Back"]);
    assert_eq!(worlds_page.rows[0].detail.as_deref(), Some("1h 1m played · 7 edits"));
    assert_eq!(worlds_page.rows[1].detail.as_deref(), Some("unreadable — a backup may still load"));
    assert!(matches!(worlds_page.style, Style::Panel));
    // Esc returns to the main menu without a request.
    assert!(matches!(step(&mut screen, &mut f, &[Intent::Cancel]), ScreenOutcome::Stay));
    assert_eq!(page(&screen, &mut f).title, "PROJECT WATT CUBED");
    // Back in, Enter on the first world loads it.
    step(&mut screen, &mut f, &[Intent::Confirm]);
    assert_eq!(page(&screen, &mut f).title, "WORLDS");
    assert_eq!(request(step(&mut screen, &mut f, &[Intent::Confirm])), Some(AppRequest::Load(ids[0].clone())));
    // The second world, and the Back row.
    let mut screen = open(&f);
    step(&mut screen, &mut f, &down(1));
    step(&mut screen, &mut f, &[Intent::Confirm]);
    step(&mut screen, &mut f, &down(1));
    assert_eq!(request(step(&mut screen, &mut f, &[Intent::Confirm])), Some(AppRequest::Load(ids[1].clone())));
    let mut screen = open(&f);
    step(&mut screen, &mut f, &down(1));
    step(&mut screen, &mut f, &[Intent::Confirm]);
    step(&mut screen, &mut f, &down(2));
    assert!(matches!(step(&mut screen, &mut f, &[Intent::Confirm]), ScreenOutcome::Stay), "Back pops the page");
    assert_eq!(page(&screen, &mut f).title, "PROJECT WATT CUBED");
}

#[test]
fn the_worlds_page_without_saves_explains_and_backs_out() {
    let mut f = fixture(Vec::new(), Session::default(), None);
    let mut screen = open(&f);
    step(&mut screen, &mut f, &down(1));
    step(&mut screen, &mut f, &[Intent::Confirm]);
    let worlds_page = page(&screen, &mut f);
    assert_eq!(worlds_page.title, "WORLDS");
    assert_eq!(labels(&screen, &mut f), ["No saved worlds yet — pick New World on the main menu", "Back"]);
    assert!(!worlds_page.rows[0].selectable);
    // The cursor lands on Back; Enter pops.
    step(&mut screen, &mut f, &[Intent::Confirm]);
    assert_eq!(page(&screen, &mut f).title, "PROJECT WATT CUBED");
}

#[test]
fn the_host_form_uses_the_remembered_session() {
    let mut f = fixture(Vec::new(), remembered("ignored-for-host", "6000", "Sam"), None);
    let mut screen = open(&f);
    step(&mut screen, &mut f, &down(2));
    step(&mut screen, &mut f, &[Intent::Confirm]);
    assert_eq!(page(&screen, &mut f).title, "HOST SERVER");
    assert_eq!(
        request(step(&mut screen, &mut f, &[Intent::Confirm])),
        Some(AppRequest::Host(HostInfo { port: 6000, password: String::new(), name: "Sam".into() }))
    );
}

#[test]
fn the_join_form_uses_the_remembered_session() {
    let mut f = fixture(Vec::new(), remembered("8.8.8.8", "6000", "Sam"), None);
    let mut screen = open(&f);
    step(&mut screen, &mut f, &down(3));
    step(&mut screen, &mut f, &[Intent::Confirm]);
    assert_eq!(page(&screen, &mut f).title, "JOIN SERVER");
    assert_eq!(
        request(step(&mut screen, &mut f, &[Intent::Confirm])),
        Some(AppRequest::Join(JoinInfo { host: "8.8.8.8".into(), port: 6000, password: String::new(), name: "Sam".into() }))
    );
}

#[test]
fn an_invalid_port_stays_on_the_form_and_an_empty_one_is_the_default() {
    let mut f = fixture(Vec::new(), Session::default(), None);
    let mut menu = HostMenu::new(&f.session);
    // Prefill is the default port; delete it and type "0".
    for _ in 0..5 {
        menu.update(Msg::Edited(ConnectionAction::Port, TextOp::Backspace), &mut f.ctx());
    }
    menu.update(Msg::Edited(ConnectionAction::Port, TextOp::Char('0')), &mut f.ctx());
    assert!(matches!(menu.update(Msg::Pick(ConnectionAction::Submit), &mut f.ctx()), ScreenOutcome::Stay));
    assert_eq!(menu.view(&f.ctx()).notice.as_ref().map(|n| n.text.as_str()), Some(PORT_ERROR));

    let mut f = fixture(Vec::new(), remembered("", "", ""), None);
    let mut menu = HostMenu::new(&f.session);
    for _ in 0..5 {
        menu.update(Msg::Edited(ConnectionAction::Port, TextOp::Backspace), &mut f.ctx());
    }
    match request(menu.update(Msg::Pick(ConnectionAction::Submit), &mut f.ctx())) {
        Some(AppRequest::Host(info)) => {
            assert_eq!(info.port, DEFAULT_PORT);
            assert_eq!(info.name, "player");
        }
        other => panic!("expected Host with the default port, got {other:?}"),
    }
}

#[test]
fn the_form_masks_the_password_and_shows_the_caret() {
    let mut f = fixture(Vec::new(), remembered("10.0.0.2", "7777", "Ada"), None);
    let mut screen = open(&f);
    step(&mut screen, &mut f, &down(3));
    step(&mut screen, &mut f, &[Intent::Confirm]);
    step(&mut screen, &mut f, &down(2));
    step(&mut screen, &mut f, &[Intent::Edit(TextOp::Char('s'))]);
    let form = page(&screen, &mut f);
    match &form.rows[2].kind {
        RowKind::Text { content, masked, .. } => assert_eq!((content.as_str(), *masked), ("s", true)),
        _ => panic!("the password row is a text row"),
    }
}

/// While the core connects or loads, the root screen draws the waiting page; idle, the menu.
#[test]
fn the_root_screen_draws_the_waiting_page_while_the_core_works() {
    let mut f = fixture(Vec::new(), Session::default(), None);
    let screen = open(&f);
    let texts = |out: &[UiElement]| -> Vec<String> {
        out.iter()
            .filter_map(|e| match e {
                UiElement::Text { text, .. } => Some(text.to_string()),
                _ => None,
            })
            .collect()
    };
    let mut out = Vec::new();
    screen.draw(&f.ctx(), &mut out, (1280, 720));
    assert_eq!(texts(&out)[0], "PROJECT WATT CUBED");
    for (phase, title) in [(Phase::Connecting, "Connecting…"), (Phase::Loading, "Loading…")] {
        f.phase = phase;
        out.clear();
        screen.draw(&f.ctx(), &mut out, (1280, 720));
        assert_eq!(texts(&out), [title, "Cancel"]);
    }
}

#[test]
fn the_package_answers_the_root_slot_and_a_suspended_one_does_not() {
    let f = fixture(Vec::new(), Session::default(), None);
    let mut mods = build();
    assert_eq!((mods.id(0), mods.name(0)), ("start", "Start"));
    assert_eq!(mods.package(0), Some("pwc.start-screen"));
    assert!(mods.root_screen(&f.facts()).is_some());
    assert!(mods.pause_screen(&f.facts()).is_none(), "the start screen is not a pause screen");
    mods.suspend_packages(&["pwc.start-screen".to_string()]);
    assert!(mods.root_screen(&f.facts()).is_none());
}
