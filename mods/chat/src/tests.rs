//! The chat line, its handlers, the scrollback and its HUD.

use super::*;
use pwc_mod_api::engine::DVec3;
use pwc_mod_api::settings::Settings;
use pwc_mod_api::sky::Sky;
use pwc_mod_api::testing::Harness;
use pwc_mod_api::{GameBuild, Notice, PackageInfo, PackageKind, TextFrame};

/// The game state a frame hook gets.
struct Env {
    player: Player,
    world: World,
    settings: Settings,
    sky: Sky,
}

impl Env {
    fn new() -> Self {
        Self { player: Player::new(DVec3::ZERO), world: World::new(1), settings: Settings::default(), sky: Sky::new() }
    }

    /// Run `f` on a frame context over this state; `networked` says whether a server is there.
    fn frame<'e, R>(&'e mut self, networked: bool, f: impl FnOnce(&mut FrameContext<'e>) -> R) -> R {
        let mut game = GameContext::new(&mut self.player, &mut self.world, &mut self.settings, &mut self.sky);
        game.networked = networked;
        let mut ctx = FrameContext::new(game);
        f(&mut ctx)
    }
}

/// Press the chat key on a frame; whether the chat took the keyboard.
fn press_open(chat: &mut Chat, env: &mut Env) -> bool {
    env.frame(false, |ctx| {
        ctx.set_action(OPEN);
        chat.on_frame(ctx);
        ctx.capturing()
    })
}

/// One typed frame while the chat holds the keyboard: the chat lines it queued, and whether it
/// still holds the keyboard.
fn typed(chat: &mut Chat, env: &mut Env, networked: bool, chars: &[char], edit: Option<EditKey>, escape: bool) -> (Vec<(Channel, String)>, bool) {
    env.frame(networked, |ctx| {
        ctx.set_text(TextFrame { chars, edit, escape });
        chat.on_frame(ctx);
        (ctx.game.chat_out().to_vec(), ctx.capturing())
    })
}

/// Open the chat, type `line` and press Enter.
fn send_line(chat: &mut Chat, env: &mut Env, networked: bool, line: &str) -> Vec<(Channel, String)> {
    assert!(press_open(chat, env));
    let chars: Vec<char> = line.chars().collect();
    typed(chat, env, networked, &chars, None, false);
    let (out, holding) = typed(chat, env, networked, &[], Some(EditKey::Submit), false);
    assert!(!holding && !chat.is_open(), "Enter sends and closes");
    out
}

fn roles(chat: &Chat, newest: usize) -> Vec<Role> {
    let shared = chat.shared.borrow();
    shared.log.iter_rev().nth(newest).expect("a line").spans().map(|s| s.role).collect()
}

#[test]
fn the_chat_key_is_the_physical_key_left_of_one_and_works_with_mod_logic_off() {
    let chat = Chat::default();
    let [action] = chat.actions() else { panic!("one action") };
    assert_eq!(action.id, OPEN);
    assert!(action.default == [Chord::key(Key::Backquote)]);
    assert!(action.immediate, "sampled every frame, mod logic or not");
    assert!(action.label.contains('§'));
}

#[test]
fn the_chat_key_opens_and_escape_closes() {
    let (mut chat, mut env) = (Chat::default(), Env::new());
    assert!(press_open(&mut chat, &mut env), "the chat takes the keyboard");
    assert!(chat.is_open());
    typed(&mut chat, &mut env, false, &['h', 'i'], None, false);
    assert_eq!(chat.line(), "hi");
    let (out, holding) = typed(&mut chat, &mut env, false, &[], None, true);
    assert!(out.is_empty() && !holding && !chat.is_open(), "Escape closes without sending");
    assert_eq!(chat.line(), "");
    assert!(chat.scrollback().is_empty());
}

/// § on an empty line closes the chat; inside a line it is typed.
#[test]
fn section_closes_an_empty_line_and_types_inside_one() {
    let (mut chat, mut env) = (Chat::default(), Env::new());
    press_open(&mut chat, &mut env);
    typed(&mut chat, &mut env, false, &['a'], None, false);
    typed(&mut chat, &mut env, false, &['§'], None, false);
    assert_eq!(chat.line(), "a§");
    typed(&mut chat, &mut env, false, &[], Some(EditKey::ClearLine), false);
    let (_, holding) = typed(&mut chat, &mut env, false, &['§'], None, false);
    assert!(!holding && !chat.is_open());
}

#[test]
fn enter_sends_local_or_global_chat_on_a_server() {
    let (mut chat, mut env) = (Chat::default(), Env::new());
    assert_eq!(send_line(&mut chat, &mut env, true, "hello"), [(Channel::Local, "hello".to_string())]);
    assert_eq!(send_line(&mut chat, &mut env, true, "! hi all"), [(Channel::Global, "hi all".to_string())]);
    assert!(send_line(&mut chat, &mut env, true, "!").is_empty(), "an empty global line sends nothing");
    assert!(chat.scrollback().is_empty(), "the server echoes chat back; nothing is printed on send");
}

#[test]
fn single_player_echoes_a_line_nobody_takes() {
    let (mut chat, mut env) = (Chat::default(), Env::new());
    assert!(send_line(&mut chat, &mut env, false, "hello").is_empty(), "no wire");
    assert_eq!(chat.scrollback(), ["> hello"]);
    assert_eq!(roles(&chat, 0), [Role::Accent]);
}

/// Handlers see every sent line in registration order; the first to take it answers and the line
/// is not chat. A handler may print through its own handle while it runs.
#[test]
fn handlers_take_lines_in_order() {
    let (mut chat, mut env) = (Chat::default(), Env::new());
    let handle = chat.handle();
    let printer = chat.handle();
    handle.add_handler(move |line, _game| match line.strip_prefix('/') {
        Some(cmd) => {
            printer.print(Line::of(Role::Dim, "running"));
            Handled::Consumed(vec![Line::of(Role::Dim, format!("ran {cmd}"))])
        }
        None => Handled::Pass,
    });
    handle.add_handler(|line, game| {
        if line == "move" {
            game.player.position.x += 1.0;
            return Handled::Consumed(Vec::new());
        }
        Handled::Pass
    });
    assert!(send_line(&mut chat, &mut env, true, "/tp").is_empty(), "taken, not sent");
    assert_eq!(chat.scrollback(), ["running", "ran tp"]);
    assert!(send_line(&mut chat, &mut env, true, "move").is_empty());
    assert_eq!(env.player.position.x, 1.0, "a handler edits the game through its context");
    assert_eq!(send_line(&mut chat, &mut env, true, "hi").len(), 1, "nobody took it: chat");
}

#[test]
fn open_with_opens_on_the_next_frame_with_the_prefix() {
    let (mut chat, mut env) = (Chat::default(), Env::new());
    chat.handle().open_with("/");
    let holding = env.frame(false, |ctx| {
        chat.on_frame(ctx);
        ctx.capturing()
    });
    assert!(holding && chat.is_open());
    assert_eq!(chat.line(), "/");
    typed(&mut chat, &mut env, false, &['t'], None, false);
    assert_eq!(chat.line(), "/t");
}

#[test]
fn tab_completes_through_the_completer_and_lists_candidates() {
    let (mut chat, mut env) = (Chat::default(), Env::new());
    chat.handle().set_completer(|line| match line {
        "/t" => Completion::Ambiguous("/t".into(), vec!["tp".into(), "time".into()]),
        "/p" => Completion::Full("/pos ".into()),
        _ => Completion::None,
    });
    press_open(&mut chat, &mut env);
    typed(&mut chat, &mut env, false, &['/', 'p'], None, false);
    typed(&mut chat, &mut env, false, &[], Some(EditKey::Complete), false);
    assert_eq!(chat.line(), "/pos ");
    typed(&mut chat, &mut env, false, &[], Some(EditKey::ClearLine), false);
    typed(&mut chat, &mut env, false, &['/', 't'], Some(EditKey::Complete), false);
    assert_eq!(chat.scrollback(), ["tp   time"]);
}

/// Chat, joins, leaves and notices land in the scrollback with today's colours.
#[test]
fn messages_land_in_the_scrollback_with_their_roles() {
    let mut chat = Chat::default();
    assert!(chat.on_message(&Message::Chat { from: "ada", channel: Channel::Global, text: "hi" }));
    assert_eq!(roles(&chat, 0), [Role::Warning, Role::Accent, Role::Muted]);
    chat.on_message(&Message::Chat { from: "bo", channel: Channel::Local, text: "yo" });
    assert_eq!(roles(&chat, 0), [Role::Accent, Role::Muted]);
    chat.on_message(&Message::Joined { name: "cy" });
    chat.on_message(&Message::Left { name: "cy" });
    assert_eq!((roles(&chat, 1), roles(&chat, 0)), (vec![Role::Positive], vec![Role::Muted]));
    for (level, role) in [(NoticeLevel::Info, Role::Dim), (NoticeLevel::Warning, Role::Warning), (NoticeLevel::Error, Role::Danger)] {
        assert!(chat.on_message(&Message::Notice(&Notice { level, text: "* note".into() })), "the chat shows notices");
        assert_eq!(roles(&chat, 0), [role]);
    }
    assert_eq!(
        chat.scrollback()[..4],
        ["[global] <ada> hi", "<bo> yo", "* cy joined", "* cy left"]
    );
    chat.reset();
    assert!(chat.scrollback().is_empty(), "a new world starts with an empty scrollback");
}

fn hud(chat: &Chat, mode: HudMode) -> Vec<HudElement> {
    let mut facts = HudFacts::new((1280, 720));
    facts.hud_mode = mode;
    let mut out = Vec::new();
    chat.hud(&facts, &World::new(1), &Player::new(DVec3::ZERO), &mut out);
    out
}

fn label_texts(out: &[HudElement]) -> Vec<String> {
    out.iter()
        .filter_map(|e| match e {
            HudElement::Label { text, .. } => Some(text.to_string()),
            _ => None,
        })
        .collect()
}

/// The newest six lines show in the Full HUD, and in any mode while the chat is open, with the
/// chat line and its caret; the spans of a line sit side by side.
#[test]
fn the_hud_shows_the_newest_lines_and_the_open_line() {
    let (mut chat, mut env) = (Chat::default(), Env::new());
    for i in 0..8 {
        chat.on_message(&Message::Joined { name: &format!("p{i}") });
    }
    chat.on_message(&Message::Chat { from: "ada", channel: Channel::Global, text: "hi" });
    let full = hud(&chat, HudMode::Full);
    let texts = label_texts(&full);
    assert_eq!(texts.len(), 5 + 3, "six lines, the newest in three spans");
    assert_eq!(texts[..3], ["[global] ", "<ada> ", "hi"]);
    let offs: Vec<(i32, i32)> = full
        .iter()
        .filter_map(|e| match e {
            HudElement::Label { off, .. } => Some(*off),
            _ => None,
        })
        .collect();
    assert_eq!((offs[0].0, offs[1].0, offs[2].0), (12, 12 + 9 * 20, 12 + 15 * 20), "spans advance by their glyphs");
    assert!(offs[3].1 < offs[0].1, "older lines sit higher");
    assert!(hud(&chat, HudMode::Minimal).is_empty() && hud(&chat, HudMode::Off).is_empty(), "closed: Full only");

    press_open(&mut chat, &mut env);
    typed(&mut chat, &mut env, false, &['y', 'o'], None, false);
    let open = hud(&chat, HudMode::Off);
    assert_eq!(label_texts(&open).last().map(String::as_str), Some("> yo"), "an open chat shows with the HUD off");
    let rects = open.iter().filter(|e| matches!(e, HudElement::Rect { .. })).count();
    assert_eq!(rects, 2, "the line's backdrop and the caret");
}

/// An unchanged chat reuses its elements: nothing is formatted on a quiet frame.
#[test]
fn an_unchanged_chat_reuses_its_hud() {
    let mut chat = Chat::default();
    chat.on_message(&Message::Joined { name: "ada" });
    let first = |out: &[HudElement]| match &out[0] {
        HudElement::Label { text, .. } => text.clone(),
        _ => panic!("a label"),
    };
    let (a, b) = (first(&hud(&chat, HudMode::Full)), first(&hud(&chat, HudMode::Full)));
    assert!(Arc::ptr_eq(&a, &b));
    chat.on_message(&Message::Left { name: "ada" });
    assert!(!Arc::ptr_eq(&a, &first(&hud(&chat, HudMode::Full))));
}

thread_local! {
    static HANDLE: RefCell<Option<ChatHandle>> = const { RefCell::new(None) };
}

fn keep_handle(r: &mut ModRegistrar) {
    let handle = r.get::<ChatHandle>().expect("pwc.chat registered first and provided its handle");
    HANDLE.with(|h| *h.borrow_mut() = Some(handle));
}

/// Registered like a build registers it: the chat is installed and a dependent package gets its
/// handle; through the host, the keyboard capture follows the chat.
#[test]
fn register_installs_the_chat_and_provides_its_handle() {
    static PACKAGES: &[PackageInfo] = &[
        PackageInfo { id: "pwc.chat", name: "Chat", version: "1.0.0", description: "", kind: PackageKind::Mod, dependencies: &[], register: Some(register) },
        PackageInfo { id: "test.user", name: "User", version: "1.0.0", description: "", kind: PackageKind::Mod, dependencies: &["pwc.chat"], register: Some(keep_handle) },
    ];
    let mut mods = Harness::new(GameBuild::from_static("sha256:00", PACKAGES));
    assert_eq!((mods.len(), mods.id(0)), (1, "chat"));
    let handle = HANDLE.with(|h| h.borrow_mut().take()).expect("the dependent got the handle");
    let mut env = Env::new();
    env.frame(false, |ctx| {
        ctx.set_action(OPEN);
        mods.frame(ctx);
    });
    assert!(mods.text_captured(), "the chat holds the keyboard");
    env.frame(false, |ctx| {
        ctx.set_text(TextFrame { chars: &[], edit: None, escape: true });
        mods.frame(ctx);
    });
    assert!(!mods.text_captured(), "Escape gave it back");
    handle.print(Line::of(Role::Dim, "printed"));
    assert!(mods.message(&Message::Joined { name: "ada" }), "the chat shows messages");
}
