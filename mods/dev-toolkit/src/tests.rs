//! The commands' behaviour, ported with the commands from the game's own tests, plus the
//! package's registration, `/help` across mods and the flight key.

use super::*;
use pwc_mod_api::player::{Motion, Player, CRUISE_DEFAULT, CRUISE_MAX, MAX_SPEED};
use pwc_mod_api::render_config::{RenderConfig, VrsChoice};
use pwc_mod_api::settings::{Settings, DEFAULT_AUTO_RENDER_SCALE};
use pwc_mod_api::sky::Sky;
use pwc_mod_api::world::generation::WorldgenKind;
use pwc_mod_api::world::terrain::cosmos::{Kind, Shape, RELIEF};
use pwc_mod_api::world::World;
use pwc_mod_api::{GameBuild, ModDescriptor, Mods, VisualMask};

use crate::options::{time, UNAVAILABLE};
use crate::travel::{landing, landing_vec};

const PACKAGE: ModDescriptor = ModDescriptor { id: "pwc.dev-toolkit", name: "Developer Toolkit", version: "1.0.0", register };

/// This package alone, registered the way a PWC build registers it.
fn build() -> Mods {
    GameBuild::new().with_mod(PACKAGE).mods()
}

/// Split a console line the way the core does (a leading `/` is optional).
fn split(line: &str) -> (&str, Vec<&str>) {
    let mut parts = line.strip_prefix('/').unwrap_or(line).split_whitespace();
    let cmd = parts.next().expect("a command");
    (cmd, parts.collect())
}

/// Every enabled mod's commands, as the core lists them in a command's context.
fn listed(mods: &Mods) -> Vec<Command> {
    mods.commands().copied().collect()
}

/// Run a line through `mods` the way the core does.
fn run_in(mods: &mut Mods, ctx: &mut CommandContext<'_>, line: &str) -> Option<Vec<Line>> {
    let (cmd, args) = split(line);
    mods.run_command(ctx, cmd, &args)
}

fn execute_with_visuals(
    line: &str,
    player: &mut Player,
    world: &mut World,
    settings: &mut Settings,
    sky: &mut Sky,
    visuals: VisualMask,
) -> Vec<Line> {
    let mut mods = build();
    let commands = listed(&mods);
    let mut ctx = CommandContext::new(player, world, settings, sky);
    ctx.visuals = visuals;
    ctx.commands = &commands;
    run_in(&mut mods, &mut ctx, line).expect("the toolkit handles it")
}

fn execute(line: &str, player: &mut Player, world: &mut World, settings: &mut Settings, sky: &mut Sky) -> Vec<Line> {
    execute_with_visuals(line, player, world, settings, sky, VisualMask::default())
}

fn player() -> Player {
    Player::new(DVec3::new(0.0, 0.0, 0.0))
}

/// A real generated world; cheap and GPU-free (meshes are uploaded separately).
fn world() -> World {
    World::generate()
}

fn diffusion() -> World {
    World::with_kind(42, RenderConfig::default(), WorldgenKind::Diffusion, false)
}

fn run(line: &str, p: &mut Player, w: &mut World) -> Vec<Line> {
    let mut s = Settings::default();
    let mut sky = Sky::new();
    execute(line, p, w, &mut s, &mut sky)
}

/// Run a command against an explicit settings value (audio commands mutate it).
fn run_settings(line: &str, s: &mut Settings) -> Vec<Line> {
    let (mut p, mut w) = (player(), world());
    let mut sky = Sky::new();
    execute(line, &mut p, &mut w, s, &mut sky)
}

/// All the lines' text joined — for asserting on multi-line output.
fn joined(lines: &[Line]) -> String {
    lines.iter().map(Line::text).collect::<Vec<_>>().join("\n")
}

fn role(lines: &[Line]) -> Role {
    lines[0].spans().next().unwrap().role
}

const HELP: &str = "commands (a leading '/' is optional):\n  \
     /tp <x y z|name>      teleport to coordinates or a body\n  \
     /bodies               list the worlds, nearest first\n  \
     /noclip               toggle flight through geometry\n  \
     /pos                  show current coordinates\n  \
     /inspect [x y z]      describe a block's elements & properties\n  \
     /reactions            show pending reaction events\n  \
     /gfx [setting value]  show or change graphics settings\n  \
     /time [set|length]    show or set the day/night clock\n  \
     /walkspeed [n]        show or set ground walk speed\n  \
     /flyspeed [n]         show or set flying speed\n  \
     /cruise [km/s|off]    space travel (default 100000 km/s) with the world held still\n  \
     /mute                 toggle master mute (this session)\n  \
     /deafen               toggle hearing incoming voice\n  \
     /audio <chan> <0-100> set master/effects/voice volume\n  \
     /voicetest            play a local voice test cue\n  \
     /gravity              show the local pull of the matter around you\n  \
     /help                 show this list";

#[test]
fn help_text_is_stable() {
    assert_eq!(joined(&help(COMMANDS)), HELP);
    let (mut p, mut w) = (player(), world());
    assert_eq!(joined(&run("help", &mut p, &mut w)), HELP);
    assert_eq!(joined(&run("?", &mut p, &mut w)), HELP);
}

/// `/help` lists every enabled mod's commands after the toolkit's, in install order.
#[test]
fn help_lists_the_commands_of_every_enabled_mod() {
    struct Other;
    impl Mod for Other {
        fn name(&self) -> &str {
            "Other"
        }
        fn id(&self) -> &'static str {
            "other"
        }
        fn commands(&self) -> &[Command] {
            &[Command { name: "wave", args: "[n]", help: "wave at everyone" }]
        }
    }
    fn other(r: &mut ModRegistrar) {
        r.add(Other);
    }
    let mut mods = GameBuild::new()
        .with_mod(PACKAGE)
        .with_mod(ModDescriptor { id: "test.other", name: "Other", version: "1.0.0", register: other })
        .mods();
    let commands = listed(&mods);
    let (mut p, mut w, mut s, mut sky) = (player(), world(), Settings::default(), Sky::new());
    let mut ctx = CommandContext::new(&mut p, &mut w, &mut s, &mut sky);
    ctx.commands = &commands;
    let text = joined(&run_in(&mut mods, &mut ctx, "/help").unwrap());
    assert_eq!(text, format!("{HELP}\n  /wave [n]             wave at everyone"));
    assert!(run_in(&mut mods, &mut ctx, "wave").is_none(), "the toolkit leaves other mods' commands to them");
}

#[test]
fn register_installs_the_toolkit() {
    let mods = build();
    assert_eq!(mods.len(), 1);
    assert_eq!((mods.id(0), mods.name(0)), ("dev_toolkit", "Developer Toolkit"));
    assert_eq!(mods.package(0), Some("pwc.dev-toolkit"));
    assert!(mods.is_active(0), "the toolkit runs");
    let names: Vec<&str> = mods.commands().map(|c| c.name).collect();
    assert_eq!(names.len(), 17);
    assert!(!names.contains(&"name"), "the crafting stub is gone");
}

/// The flight key toggles walking and ordinary flight, never noclip, and says it took the key.
#[test]
fn f_toggles_walking_and_flying() {
    let (mut p, w) = (Player::new(DVec3::new(0.5, 80.0, 0.5)), world());
    let mut mods = build();
    for flying in [true, false, true, false] {
        assert!(mods.on_toggle_fly(&mut p, &w), "the toolkit takes the flight key");
        assert_eq!(p.flying(), flying);
        assert!(!p.noclip());
    }
}

#[test]
fn tp_sets_position_and_clears_fall() {
    let (mut p, mut w) = (player(), world());
    p.motion = Motion::Walking { velocity: DVec3::new(0.0, -50.0, 0.0), on_ground: false };
    let out = run("tp 1.5 2 3", &mut p, &mut w);
    assert_eq!(p.position, DVec3::new(1.5, 2.0, 3.0));
    assert_eq!(p.velocity().y, 0.0);
    assert!(out[0].text().contains("teleported"));
}

#[test]
fn tp_keeps_f64_precision_and_clamps_to_the_border() {
    let (mut p, mut w) = (player(), world());
    // Far coordinates parse as f64: no f32 quantisation on the way in.
    run("tp 100000000.5 60 -7", &mut p, &mut w);
    assert_eq!(p.position, DVec3::new(100_000_000.5, 60.0, -7.0));

    // Past the border: clamped, and the OUTPUT reports the clamped spot.
    let out = run("tp 99999999999 60 -99999999999", &mut p, &mut w);
    assert_eq!(p.position.x, 1.0e9);
    assert_eq!(p.position.z, -1.0e9);
    assert!(out[0].text().contains("1000000000.0"), "reports the clamped position");

    // Non-finite input is refused outright.
    let before = p.position;
    run("tp inf 0 0", &mut p, &mut w);
    assert_eq!(p.position, before);
}

#[test]
fn tp_prepares_collision_data_at_the_destination() {
    let (mut p, mut w) = (player(), world());
    // Far outside the pre-generated spawn region: without the prepare, the
    // ground under the destination would be unloaded air and the next
    // physics step would fall straight through.
    let (x, z) = (5_000, 5_000);
    let surface = w.surface_y(x, z);
    let line = format!("tp {x} {} {z}", surface + 2);
    run(&line, &mut p, &mut w);
    assert!(!w.spawn_ready(), "a far teleport must wait on the async spawn slab");
    // The request is the destination's slab, ground included: once that is loaded, the same
    // teleport has nothing to wait for. (The game's own tests drive the slab in.)
    let mut w = world();
    w.ensure_around(p.position);
    assert!(w.is_solid(x, surface - 1, z), "the destination's slab holds its ground");
    run(&line, &mut p, &mut w);
    assert!(w.spawn_ready(), "the outstanding request was exactly the destination's slab");
}

#[test]
fn aliases_reach_the_same_command() {
    let (mut p, mut w) = (player(), world());
    run("teleport 4 5 6", &mut p, &mut w);
    assert_eq!(p.position, DVec3::new(4.0, 5.0, 6.0));
    run("setpos 7 8 9", &mut p, &mut w);
    assert_eq!(p.position, DVec3::new(7.0, 8.0, 9.0));
    assert_eq!(joined(&run("where", &mut p, &mut w)), "position: X 7.0  Y 8.0  Z 9.0");
    assert_eq!(joined(&run("pos", &mut p, &mut w)), "position: X 7.0  Y 8.0  Z 9.0");
}

#[test]
fn bad_args_do_not_move_the_player() {
    let (mut p, mut w) = (player(), world());
    run("tp 1 two 3", &mut p, &mut w);
    assert_eq!(p.position, DVec3::new(0.0, 0.0, 0.0));
    run("tp 1 2", &mut p, &mut w);
    assert_eq!(p.position, DVec3::new(0.0, 0.0, 0.0));
}

/// A command the toolkit does not know is left to the other mods (and then the core's hint).
#[test]
fn unknown_commands_are_left_to_others() {
    let (mut p, mut w, mut s, mut sky) = (player(), world(), Settings::default(), Sky::new());
    let mut ctx = CommandContext::new(&mut p, &mut w, &mut s, &mut sky);
    assert!(run_in(&mut build(), &mut ctx, "fly-to-moon").is_none());
    assert!(run_in(&mut build(), &mut ctx, "name 1 pick").is_none(), "the crafting stub is gone");
}

#[test]
fn inspect_reports_elements_and_properties() {
    let (mut p, mut w) = (player(), world());
    // Deep underground is rock: a labelled configuration with observation readings.
    let out = run("inspect 8 0 8", &mut p, &mut w);
    let text = joined(&out);
    assert!(text.contains("rock"), "should name the block: {text}");
    assert!(text.contains("made of:"), "should list elements: {text}");
    assert!(text.contains("hardness"), "should show observation readings: {text}");
    assert!(text.contains("descriptor"), "should show the render descriptor: {text}");
}

#[test]
fn inspect_above_world_is_air() {
    let (mut p, mut w) = (player(), world());
    let out = run("inspect 8 60 8", &mut p, &mut w);
    assert!(joined(&out).contains("air"));
}

#[test]
fn reactions_prints_active_turns_operations() {
    let (mut p, mut w) = (player(), world());
    let out = run("reactions", &mut p, &mut w);
    assert_eq!(joined(&out), "reactions: active=0 turns=0 operations=0");
}

#[test]
fn gfx_updates_settings_with_clamping() {
    let (mut p, mut w) = (player(), world());
    let mut s = Settings::default();
    let mut sky = Sky::new();
    execute("gfx msaa 4", &mut p, &mut w, &mut s, &mut sky);
    assert_eq!(s.msaa, 4);
    execute("gfx fps 144", &mut p, &mut w, &mut s, &mut sky);
    assert_eq!(s.max_fps, 144);
    execute("gfx fps off", &mut p, &mut w, &mut s, &mut sky);
    assert_eq!(s.max_fps, 0);
    execute("gfx renderdist 99", &mut p, &mut w, &mut s, &mut sky);
    assert_eq!(s.render_distance, 20);
    execute("gfx fullscreen on", &mut p, &mut w, &mut s, &mut sky);
    assert!(s.fullscreen);
    execute("gfx lighting off", &mut p, &mut w, &mut s, &mut sky);
    assert!(!s.lighting);
    execute("gfx vrs on", &mut p, &mut w, &mut s, &mut sky);
    assert_eq!(s.vrs, VrsChoice::On);
    execute("graphics vrs auto", &mut p, &mut w, &mut s, &mut sky);
    assert_eq!(s.vrs, VrsChoice::Auto);
    let out = execute("gfx", &mut p, &mut w, &mut s, &mut sky);
    let text = joined(&out);
    assert!(text.contains("fullscreen on"));
    assert!(text.contains("lighting off"));
    assert!(text.contains("vrs auto"));
    assert!(text.contains("ui scale"));
}

#[test]
fn gfx_lists_default_auto_render_scale() {
    let (mut p, mut w) = (player(), world());
    let mut s = Settings::default();
    let mut sky = Sky::new();
    s.note_render_extent(1920, 1080, 1.0);
    let text = joined(&execute("gfx", &mut p, &mut w, &mut s, &mut sky));
    assert!(
        text.contains(&format!("render scale Auto ({:.1})", DEFAULT_AUTO_RENDER_SCALE)),
        "Default /gfx prints the effective Auto scale: {text}"
    );
}

#[test]
fn gfx_lists_effective_visual_lanes_when_a_mod_strips_them() {
    let (mut p, mut w) = (player(), world());
    let mut s = Settings::default();
    let mut sky = Sky::new();
    let mask = VisualMask { atmosphere: true, post: false, lighting: true };
    let out = execute_with_visuals("gfx", &mut p, &mut w, &mut s, &mut sky, mask);
    let text = joined(&out);
    assert!(
        text.contains(&format!("bloom on {UNAVAILABLE}")),
        "effective /gfx must mark the stripped lane: {text}"
    );
    assert!(!text.contains(&format!("shadows on {UNAVAILABLE}")), "Lighting is still provided: {text}");
    let set = execute_with_visuals("gfx bloom off", &mut p, &mut w, &mut s, &mut sky, mask);
    assert!(
        joined(&set).contains(&format!("bloom off {UNAVAILABLE}")),
        "a set confirmation must also show the strip: {}",
        joined(&set)
    );
}

#[test]
fn gfx_bad_input_prints_usage_and_changes_nothing() {
    let (mut p, mut w) = (player(), world());
    let mut s = Settings::default();
    let mut sky = Sky::new();
    let before = s.clone();
    let out = execute("gfx msaa lots", &mut p, &mut w, &mut s, &mut sky);
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
    let (mut p, mut w, mut s, mut sky) = (player(), world(), Settings::default(), Sky::new());
    let mut ctx = CommandContext::new(&mut p, &mut w, &mut s, &mut sky);
    let out = run_in(&mut build(), &mut ctx, "voicetest").unwrap();
    assert!(out[0].text().contains("voice test"));
    assert_eq!(role(&out), Role::Dim);
    assert!(ctx.voice_test);
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

#[test]
fn walkspeed_and_flyspeed_set_independently() {
    let (mut p, mut w) = (player(), world());
    let out = run("walkspeed 10", &mut p, &mut w);
    assert_eq!(p.speed, 10.0);
    assert!(out[0].text().contains("walkspeed set to 10.00"));

    run("flyspeed 25", &mut p, &mut w);
    assert_eq!(p.fly_speed, 25.0);
    // Setting one doesn't disturb the other.
    assert_eq!(p.speed, 10.0);

    let out = run("walkspeed", &mut p, &mut w);
    assert!(out[0].text().contains("walkspeed: 10.00"));
}

#[test]
fn speed_commands_reject_non_positive_and_non_finite() {
    let (mut p, mut w) = (player(), world());
    let before = p.speed;
    for bad in ["0", "-5", "inf", "nan", "banana"] {
        let out = run(&format!("walkspeed {bad}"), &mut p, &mut w);
        assert_eq!(p.speed, before, "{bad} should not change speed");
        assert_eq!(role(&out), Role::Danger);
    }
}

/// A speed past the limit would make one frame's collision walk billions of substeps (the game
/// froze, and stayed frozen on rejoin): it is refused, and the limit itself is accepted.
#[test]
fn speed_commands_stop_at_the_speed_limit() {
    let (mut p, mut w) = (player(), world());
    let before = p.fly_speed;
    for bad in ["1e12", "99999999999999999999", "1e308"] {
        let out = run(&format!("flyspeed {bad}"), &mut p, &mut w);
        assert_eq!(p.fly_speed, before, "{bad} must not change the fly speed");
        assert_eq!(role(&out), Role::Danger);
        assert!(out[0].text().contains("/tp"), "the refusal points at /tp: {}", out[0].text());
        run(&format!("walkspeed {bad}"), &mut p, &mut w);
        assert_ne!(p.speed, bad.parse::<f64>().unwrap());
    }
    run(&format!("flyspeed {MAX_SPEED}"), &mut p, &mut w);
    assert_eq!(p.fly_speed, MAX_SPEED);
}

/// A coordinate teleport far from spawn over the round start world says it left the player high
/// above the curved ground, in space, and how to land.
#[test]
fn a_far_coordinate_teleport_reports_the_altitude() {
    let mut w = diffusion();
    let mut p = player();
    let out = run("tp -1000000 200 1000000", &mut p, &mut w);
    let note = out.get(1).expect("an altitude note").text();
    assert!(note.contains("km above the ground of home (in space)"), "{note}");
    assert!(note.contains("/tp home"), "{note}");
    let km: f64 = note.split(' ').next().unwrap().parse().unwrap();
    assert!((35.0..60.0).contains(&km), "about 52,600 blocks up: {km} km");
    // Near the ground: no note.
    let out = run("tp 0.5 60 0.5", &mut p, &mut w);
    assert_eq!(out.len(), 1, "{:?}", out.iter().map(|l| l.text()).collect::<Vec<_>>());
}

/// `/cruise` starts a noclip flight at 100,000 km/s, takes speeds in km/s up to ten times light
/// speed, refuses more, and ends at rest in flight, lifted out of the ground inside a world.
#[test]
fn cruise_starts_changes_and_ends_clear_of_the_ground() {
    let mut w = diffusion();
    let mut p = player();
    run("cruise", &mut p, &mut w);
    assert_eq!(p.cruise.map(|c| c.speed), Some(CRUISE_DEFAULT));
    assert!(p.noclip());
    let out = run("cruise 250000", &mut p, &mut w);
    let want = 250_000.0 * 1000.0 / pwc_mod_api::math::BLOCK_METERS;
    assert!((p.cruise.unwrap().speed - want).abs() < 1e-3, "typed in km/s");
    assert!(out[0].text().starts_with("cruising at 250000 km/s (0.834 c)"), "{}", out[0].text());
    run("cruise 1_000_000", &mut p, &mut w);
    assert!((p.cruise.unwrap().speed - 1.0e9 / pwc_mod_api::math::BLOCK_METERS).abs() < 1e-3, "past light speed");
    run("cruise 5e4", &mut p, &mut w);
    let out = run("cruise 1e9", &mut p, &mut w);
    assert_eq!(role(&out), Role::Danger);
    assert!((p.cruise.unwrap().speed - 5.0e7 / pwc_mod_api::math::BLOCK_METERS).abs() < 1e-3, "a refused speed changes nothing");
    assert!(p.speed_limit() >= CRUISE_MAX);

    // Deep inside the start world (100 km under spawn): ending lifts the player above its ground.
    p.position = DVec3::new(0.5, -100_000.0, 0.5);
    match &mut p.motion {
        Motion::Walking { velocity, .. } | Motion::Flying { velocity, .. } => *velocity = DVec3::new(3.0e8, 0.0, 0.0),
    }
    run("cruise off", &mut p, &mut w);
    assert!(!p.cruising());
    assert!(p.flying() && !p.noclip(), "in ordinary flight");
    assert_eq!(p.velocity(), DVec3::ZERO);
    let cosmos = w.terrain().cosmos().unwrap();
    let alt = cosmos.altitude(cosmos.home(), p.position);
    assert!(alt > RELIEF as f64, "above the ground: {alt}");
    assert!(alt < 3_000.0, "but not far above: {alt}");
    let out = run("cruise off", &mut p, &mut w);
    assert_eq!(role(&out), Role::Danger);
}

#[test]
fn noclip_toggles_walk_to_noclip_and_back() {
    let (mut p, mut w) = (player(), world());
    assert!(!p.flying());
    let on = run("noclip", &mut p, &mut w);
    assert!(p.noclip());
    assert!(on[0].text().contains("on"));
    // From ordinary flight, too.
    p.toggle_noclip();
    assert!(!p.flying());
    p.set_flying(true);
    run("noclip", &mut p, &mut w);
    assert!(p.noclip());
    let off = run("noclip", &mut p, &mut w);
    assert!(!p.flying());
    assert!(!p.noclip());
    assert!(off[0].text().contains("off"));
}

#[test]
fn a_world_without_a_cosmos_has_no_bodies_to_find() {
    let (mut p, mut w) = (player(), world());
    let at = p.position;
    let listed = joined(&run("bodies", &mut p, &mut w));
    assert!(listed.contains("no cosmos"), "{listed}");
    let tp = run("tp verdant", &mut p, &mut w);
    assert!(joined(&tp).contains("no cosmos"), "{}", joined(&tp));
    assert_eq!(role(&tp), Role::Danger);
    assert_eq!(p.position, at);
    assert!(w.in_air(DVec3::splat(1.0e8)));
}

#[test]
fn bodies_lists_home_first_and_named_tp_lands_on_it() {
    let (mut p, mut w) = (player(), diffusion());
    p.position = DVec3::new(0.5, 80.0, 0.5);
    let cosmos = w.terrain().cosmos().expect("diffusion has a cosmos");
    let home = cosmos.bodies().iter().copied().find(|b| b.kind == Kind::Home).unwrap();
    let verdant = cosmos.bodies().iter().copied().find(|b| b.kind == Kind::Verdant).unwrap();
    let moons: Vec<_> = cosmos.bodies().iter().copied().filter(|b| b.kind == Kind::Moon).collect();
    assert!(moons.len() >= 2, "catalog order has a second moon");

    let lines = run("bodies", &mut p, &mut w);
    let first = lines[0].text();
    assert!(first.starts_with("home 1"), "{first}");
    let home_at = landing(&w, &home);
    assert!(first.contains(&format!("/tp {} {} {}", home_at[0], home_at[1], home_at[2])), "{first}");
    let text = joined(&lines);
    assert!(text.contains("verdant 1"), "{text}");
    assert!(text.contains("moon 2"), "{text}");

    // The relaxed start world and a warped twin: the landing sits just above the real top.
    run("tp home", &mut p, &mut w);
    let alt = w.terrain().cosmos().expect("a cosmos").altitude(&home, p.position);
    assert!((1_000.0..4_100.0).contains(&alt), "home landing altitude {alt}");
    run("tp twin", &mut p, &mut w);
    let twin = w.terrain().cosmos().expect("a cosmos").bodies().iter().copied().find(|b| b.kind == Kind::Twin).unwrap();
    let Shape::Cube { half } = twin.shape else { panic!("a twin is a cube") };
    let rise = (p.position - twin.centre_f()).y / half as f64;
    assert!((1.03..1.12).contains(&rise), "twin landing over the bowed face: {rise}");
    run("tp verdant", &mut p, &mut w);
    let want = landing_vec(&w, &verdant);
    assert_eq!(p.position, want);
    assert!((p.position - verdant.centre_f()).length() <= verdant.reach());
    let pull = w.gravity_at(p.position).accel;
    let toward = (verdant.centre_f() - p.position).normalize();
    assert!(pull.normalize().dot(toward) > 0.99, "standing in Verdance's pull: {pull:?}");
    assert!(p.up().dot(-pull.normalize()) > 0.99, "up faces away from the pull");

    p.position = DVec3::ZERO;
    run("tp ver", &mut p, &mut w);
    assert_eq!(p.position, want, "a unique prefix selects the same body");

    run("tp moon 2", &mut p, &mut w);
    let moon = landing_vec(&w, &moons[1]);
    assert_eq!(p.position, moon);
    assert_ne!(moon, landing_vec(&w, &moons[0]));

    let at = p.position;
    let bad = run("tp nope", &mut p, &mut w);
    assert_eq!(p.position, at);
    assert_eq!(role(&bad), Role::Danger);
    assert!(bad[0].text().contains("nope"), "{}", bad[0].text());
    let ambiguous = run("tp h", &mut p, &mut w);
    assert_eq!(p.position, at);
    assert!(ambiguous[0].text().contains("more than one"), "{}", ambiguous[0].text());
}
