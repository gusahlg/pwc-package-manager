//! Dispatch, aliases, completion, `/help`, the unknown hint, `/op`, the `/` key, and the shipped
//! commands' behaviour (ported with them from the Developer Toolkit).

use super::*;
use pwc_mod_api::audio::GameEvent;
use pwc_mod_api::engine::DVec3;
use pwc_mod_api::input::intent::EditKey;
use pwc_mod_api::player::Player;
use pwc_mod_api::render_config::VrsChoice;
use pwc_mod_api::settings::{Settings, DEFAULT_AUTO_RENDER_SCALE};
use pwc_mod_api::sky::Sky;
use pwc_mod_api::world::World;
use pwc_mod_api::{GameBuild, PackageInfo, PackageKind, TextFrame, VisualMask};

use crate::options::time;

/// The game state a command gets.
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

    fn game(&mut self) -> GameContext<'_> {
        GameContext::new(&mut self.player, &mut self.world, &mut self.settings, &mut self.sky)
    }
}

/// The shipped commands alone.
fn builtins() -> CommandsHandle {
    let commands = CommandsHandle::new();
    add_builtins(&commands);
    commands
}

/// Run `line` through the shipped commands (no echo); `None` for an unknown command.
fn run_with(env: &mut Env, line: &str) -> Option<Vec<Line>> {
    builtins().run(line, &mut env.game())
}

fn run_settings(line: &str, s: &mut Settings) -> Vec<Line> {
    let mut env = Env::new();
    env.settings = s.clone();
    let out = run_with(&mut env, line).expect("a shipped command");
    *s = env.settings;
    out
}

fn joined(lines: &[Line]) -> String {
    lines.iter().map(|l| l.spans().map(|s| s.text.as_str()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn role(lines: &[Line]) -> Role {
    lines[0].spans().next().unwrap().role
}

/// A chat line handled the way the chat offers it: the handler's answer, or `None` for a pass.
fn offered(commands: &CommandsHandle, env: &mut Env, networked: bool, line: &str) -> (Option<Vec<Line>>, Vec<(Channel, String)>) {
    let mut game = env.game();
    game.networked = networked;
    let answer = match handle_line(commands, line, &mut game) {
        Handled::Consumed(lines) => Some(lines),
        Handled::Pass => None,
    };
    (answer, game.chat_out().to_vec())
}

const HELP_TEXT: &str = "commands (a leading '/' is optional):\n  \
     /gfx [setting value]  show or change graphics settings\n  \
     /time [set|length]    show or set the day/night clock\n  \
     /mute                 toggle master mute (this session)\n  \
     /deafen               toggle hearing incoming voice\n  \
     /audio <chan> <0-100> set master/effects/voice volume\n  \
     /voicetest            play a local voice test cue\n  \
     /op <secret>          log in as a server operator\n  \
     /help                 show this list";

#[test]
fn help_lists_every_command_in_registration_order() {
    let commands = builtins();
    let mut env = Env::new();
    assert_eq!(joined(&commands.run("help", &mut env.game()).unwrap()), HELP_TEXT);
    assert_eq!(joined(&commands.run("/?", &mut env.game()).unwrap()), HELP_TEXT, "an alias");
    fn wave(_: &mut GameContext, args: &[&str]) -> Vec<Line> {
        shown(vec![format!("waved {}", args.len())])
    }
    commands.add(Command { name: "wave", aliases: &[], args: "[n]", help: "wave at everyone" }, wave);
    commands.add(Command { name: "time", aliases: &[], args: "", help: "shadowed" }, wave);
    let text = joined(&commands.run("/help", &mut env.game()).unwrap());
    assert_eq!(text, format!("{HELP_TEXT}\n  /wave [n]             wave at everyone"), "a later command is listed, a shadowed name once");
    assert_eq!(joined(&commands.run("/wave a b", &mut env.game()).unwrap()), "waved 2");
    assert!(commands.run("/time", &mut env.game()).unwrap()[0].text().starts_with("time:"), "the first command with a name runs");
    assert!(commands.run("/nope", &mut env.game()).is_none());
    assert!(commands.run("/", &mut env.game()).is_none());
}

/// The chat handler: `/` lines are commands, echoed before their output; an unknown name gets
/// the hint. On a server other lines are chat; in single player every line is a command.
#[test]
fn the_chat_handler_echoes_runs_and_hints() {
    let commands = builtins();
    let mut env = Env::new();
    let (out, sent) = offered(&commands, &mut env, true, "/time set noon");
    let out = out.expect("taken");
    assert!(sent.is_empty());
    assert_eq!((out[0].text(), role(&out)), ("> /time set noon", Role::Accent));
    assert_eq!(out[1].text(), "time set to 12:00");
    let (out, _) = offered(&commands, &mut env, true, "/tp 1 2 3");
    assert_eq!(joined(&out.unwrap()), "> /tp 1 2 3\nunknown command 'tp' - type '/help'");
    assert!(offered(&commands, &mut env, true, "hello").0.is_none(), "chat on a server");
    let (out, _) = offered(&commands, &mut env, false, "mute");
    assert_eq!(joined(&out.unwrap()), "> mute\naudio muted", "single player: the slash is optional");
    let (out, _) = offered(&commands, &mut env, false, "hello there");
    assert_eq!(joined(&out.unwrap()), "> hello there\nunknown command 'hello' - type '/help'");
    let (out, _) = offered(&commands, &mut env, true, "/");
    assert_eq!(joined(&out.unwrap()), "> /", "a bare slash is echoed and does nothing");
}

/// `/op <secret>` goes to the server as global chat, verbatim and never echoed.
#[test]
fn op_is_sent_to_the_server_unechoed() {
    let commands = builtins();
    let mut env = Env::new();
    let (out, sent) = offered(&commands, &mut env, true, "/op hunter2");
    assert!(out.expect("taken").is_empty(), "nothing printed: the line carries a secret");
    assert_eq!(sent, [(Channel::Global, "/op hunter2".to_string())]);
    let (out, sent) = offered(&commands, &mut env, false, "/op hunter2");
    let out = out.unwrap();
    assert!(sent.is_empty() && !joined(&out).contains("hunter2"), "single player: no wire, no echo");
    assert_eq!(role(&out), Role::Danger);
}

#[test]
fn tab_completes_command_names() {
    let commands = builtins();
    let full = |line: &str| match commands.complete(line) {
        Completion::Full(s) => Some(s),
        _ => None,
    };
    assert_eq!(full("/vo").as_deref(), Some("/voicetest "));
    assert_eq!(full("he").as_deref(), Some("help "), "no slash, no slash added");
    match commands.complete("/t") {
        Completion::Full(s) => assert_eq!(s, "/time "),
        _ => panic!("one match"),
    }
    match commands.complete("/d") {
        Completion::Full(s) => assert_eq!(s, "/deafen "),
        _ => panic!("one match"),
    }
    commands.add(Command { name: "tp", aliases: &[], args: "", help: "" }, |_, _| Vec::new());
    match commands.complete("/t") {
        Completion::Ambiguous(prefix, candidates) => {
            assert_eq!(prefix, "/t");
            assert_eq!(candidates, ["time", "tp"]);
        }
        _ => panic!("candidates"),
    }
    assert!(matches!(commands.complete(""), Completion::None));
    assert!(matches!(commands.complete("/zzz"), Completion::None));
    assert!(matches!(commands.complete("/tp 1"), Completion::None), "no completion after a space");
    assert!(matches!(commands.complete("/graph"), Completion::None), "aliases are not completed");
}

#[test]
fn gfx_updates_settings_with_clamping() {
    let mut s = Settings::default();
    for line in ["gfx msaa 4", "gfx fps 144"] {
        run_settings(line, &mut s);
    }
    assert_eq!((s.msaa, s.max_fps), (4, 144));
    run_settings("gfx fps off", &mut s);
    assert_eq!(s.max_fps, 0);
    run_settings("gfx renderdist 99", &mut s);
    assert_eq!(s.render_distance, 20);
    run_settings("gfx fullscreen on", &mut s);
    assert!(s.fullscreen);
    run_settings("gfx lighting off", &mut s);
    assert!(!s.lighting);
    run_settings("gfx vrs on", &mut s);
    assert_eq!(s.vrs, VrsChoice::On);
    run_settings("graphics vrs auto", &mut s);
    assert_eq!(s.vrs, VrsChoice::Auto);
    let text = joined(&run_settings("gfx", &mut s));
    assert!(text.contains("fullscreen on"));
    assert!(text.contains("lighting off"));
    assert!(text.contains("vrs auto"));
    assert!(text.contains("ui scale"));
}

/// A changed value marks the settings for the core to apply and save; a listing does not.
#[test]
fn gfx_marks_the_settings_only_when_a_value_changes() {
    let commands = builtins();
    let mut env = Env::new();
    let mut game = env.game();
    commands.run("gfx", &mut game);
    assert!(!game.settings_changed());
    commands.run("gfx msaa lots", &mut game);
    assert!(!game.settings_changed(), "a bad value changes nothing");
    commands.run("gfx msaa 4", &mut game);
    assert!(game.settings_changed());
}

#[test]
fn gfx_lists_default_auto_render_scale() {
    let mut s = Settings::default();
    s.note_render_extent(1920, 1080, 1.0);
    let text = joined(&run_settings("gfx", &mut s));
    assert!(
        text.contains(&format!("render scale Auto ({:.1})", DEFAULT_AUTO_RENDER_SCALE)),
        "Default /gfx prints the effective Auto scale: {text}"
    );
}

#[test]
fn gfx_lists_effective_visual_lanes_when_a_mod_strips_them() {
    let commands = builtins();
    let mut env = Env::new();
    let mut game = env.game();
    game.visuals = VisualMask { atmosphere: true, post: false, lighting: true };
    let text = joined(&commands.run("gfx", &mut game).unwrap());
    assert!(text.contains(&format!("bloom on {}", forced_off_marker("Post"))), "effective /gfx must name the stripping mod: {text}");
    assert_eq!(forced_off_marker("Post"), "(off: Post mod)");
    assert!(!text.contains("shadows on (off:"), "Lighting is still enabled: {text}");
    let set = joined(&commands.run("gfx bloom off", &mut game).unwrap());
    assert!(set.contains(&format!("bloom off {}", forced_off_marker("Post"))), "a set confirmation must also show the strip: {set}");
}

#[test]
fn gfx_bad_input_prints_usage_and_changes_nothing() {
    let mut s = Settings::default();
    let before = s.clone();
    let out = run_settings("gfx msaa lots", &mut s);
    assert!(out[0].text().contains("usage"));
    let text = joined(&out);
    assert!(text.contains("lighting on|off"));
    assert!(text.contains("uiscale <50-200>"));
    assert_eq!(role(&out), Role::Danger);
    assert_eq!(s, before);
}

#[test]
fn mute_toggles_transient_and_survives_no_save() {
    let mut s = Settings::default();
    assert!(!s.muted);
    assert!(run_settings("mute", &mut s)[0].text().contains("muted"));
    assert!(s.muted);
    assert!(run_settings("mute", &mut s)[0].text().contains("unmuted"));
    assert!(!s.muted);
}

#[test]
fn deafen_flips_the_persisted_incoming_gate() {
    let mut s = Settings::default();
    assert!(s.voice_incoming);
    run_settings("deafen", &mut s);
    assert!(!s.voice_incoming);
    assert!(s.mix_change().deafen, "deafen is the inverse of voice_incoming");
    run_settings("deafen", &mut s);
    assert!(s.voice_incoming);
}

#[test]
fn audio_sets_and_clamps_each_channel() {
    let mut s = Settings::default();
    run_settings("audio master 45", &mut s);
    assert_eq!(s.master_volume, 45);
    run_settings("audio effects 200", &mut s); // over 100 clamps
    assert_eq!(s.effects_volume, 100);
    run_settings("volume voice 0", &mut s);
    assert_eq!(s.voice_volume, 0);

    // Bad channel or value is a Danger rejection that changes nothing.
    let before = s.clone();
    let out = run_settings("audio bass 50", &mut s);
    assert_eq!(role(&out), Role::Danger);
    let out = run_settings("audio master loud", &mut s);
    assert_eq!(role(&out), Role::Danger);
    assert_eq!(s, before);
}

/// `/voicetest` answers with a line and asks the core for the cue.
#[test]
fn voicetest_asks_the_core_for_the_cue() {
    let commands = builtins();
    let mut env = Env::new();
    let mut game = env.game();
    let out = commands.run("voicetest", &mut game).unwrap();
    assert!(out[0].text().contains("voice test"));
    assert_eq!(role(&out), Role::Dim);
    assert!(matches!(game.events.as_slice(), [GameEvent::VoiceTest]));
}

#[test]
fn time_set_accepts_names_fractions_and_hours() {
    let mut sky = Sky::new();
    assert!(time(&["set", "noon"], &mut sky)[0].text().contains("12:00"));
    assert!((sky.clock.day() - 0.5).abs() < 1e-9);
    time(&["set", "0.25"], &mut sky);
    assert!((sky.clock.day() - 0.25).abs() < 1e-9);
    time(&["set", "18"], &mut sky); // 18:00 → 0.75
    assert!((sky.clock.day() - 0.75).abs() < 1e-9);
    // A bad value leaves the clock untouched.
    let before = sky.clock.day();
    assert!(time(&["set", "banana"], &mut sky)[0].text().contains("use"));
    assert_eq!(sky.clock.day(), before);
}

#[test]
fn time_length_clamps() {
    let mut sky = Sky::new();
    time(&["length", "1"], &mut sky); // below the 10s floor
    assert_eq!(sky.day_length.0, 10.0);
}

/// Built like a PWC build: the chat, then the commands on its handle. `/` opens the chat with a
/// slash; a typed command runs through the chat and prints to its scrollback.
#[test]
fn slash_opens_the_chat_and_a_command_runs_through_it() {
    static PACKAGES: &[PackageInfo] = &[
        PackageInfo { id: "pwc.chat", name: "Chat", version: "1.0.0", description: "", kind: PackageKind::Mod, dependencies: &[], register: Some(pwc_chat::register) },
        PackageInfo { id: "pwc.commands", name: "Commands", version: "1.0.0", description: "", kind: PackageKind::Mod, dependencies: &["pwc.chat"], register: Some(register) },
    ];
    let mut mods = GameBuild::from_static("sha256:00", PACKAGES).mods();
    assert_eq!((mods.len(), mods.id(0), mods.id(1)), (2, "chat", "commands"));
    let mut env = Env::new();
    let mut frame = |mods: &mut pwc_mod_api::Mods, action: Option<&'static str>, text: Option<TextFrame<'_>>| {
        let mut ctx = FrameContext::new(env.game());
        if let Some(action) = action {
            ctx.set_action(action);
        }
        if let Some(text) = text {
            ctx.set_text(text);
        }
        mods.on_frame(&mut ctx);
    };
    frame(&mut mods, Some(OPEN), None);
    assert!(!mods.text_captured(), "the chat opens on its next frame");
    frame(&mut mods, None, None);
    assert!(mods.text_captured(), "the chat holds the keyboard");
    let typed: Vec<char> = "mute".chars().collect();
    frame(&mut mods, None, Some(TextFrame { chars: &typed, edit: None, escape: false }));
    frame(&mut mods, None, Some(TextFrame { chars: &[], edit: Some(EditKey::Submit), escape: false }));
    assert!(!mods.text_captured(), "Enter closes the chat");
    assert!(env.settings.muted, "the command ran on the game state");
}
