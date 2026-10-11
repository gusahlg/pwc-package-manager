//! The commands' behaviour, ported with the commands from the game's own tests, plus the
//! package's registration with `pwc.commands` and the flight key.

use super::*;
use pwc_mod_api::testing::Harness;
use std::cell::RefCell;

use pwc_mod_api::player::{Motion, Player, CRUISE_DEFAULT, CRUISE_MAX, MAX_SPEED};
use pwc_mod_api::render_config::RenderConfig;
use pwc_mod_api::settings::Settings;
use pwc_mod_api::sky::Sky;
use pwc_mod_api::world::generation::WorldgenKind;
use pwc_mod_api::world::terrain::cosmos::{Form, Shape, RELIEF};
use pwc_mod_api::world::World;
use pwc_mod_api::{GameBuild, PackageInfo, PackageKind};

use crate::travel::{landing, landing_vec};

thread_local! {
    /// The commands registry the test build's stand-in for `pwc.commands` provided.
    static PROVIDED: RefCell<Option<CommandsHandle>> = const { RefCell::new(None) };
}

/// A stand-in for `pwc.commands`: an empty registry, provided like the real one.
fn provide_commands(r: &mut ModRegistrar) {
    let commands = CommandsHandle::new();
    PROVIDED.with(|p| *p.borrow_mut() = Some(commands.clone()));
    r.provide(commands);
}

/// This package registered the way a PWC build registers it, after a registry it fills.
fn build() -> (Harness, CommandsHandle) {
    static PACKAGES: &[PackageInfo] = &[
        PackageInfo { id: "pwc.commands", name: "Commands", version: "1.0.0", description: "", kind: PackageKind::Mod, dependencies: &[], register: Some(provide_commands) },
        PackageInfo { id: "pwc.dev-toolkit", name: "Developer Toolkit", version: "2.1.0", description: "", kind: PackageKind::Mod, dependencies: &["pwc.commands"], register: Some(register) },
    ];
    let mods = Harness::new(GameBuild::from_static("sha256:00", PACKAGES));
    (mods, PROVIDED.with(|p| p.borrow_mut().take()).expect("the registry was provided"))
}

/// The toolkit's commands alone.
fn toolkit() -> CommandsHandle {
    let commands = CommandsHandle::new();
    add_commands(&commands);
    commands
}

/// Run a line through the toolkit's commands (a leading `/` is optional), as the chat would.
fn execute(line: &str, player: &mut Player, world: &mut World, settings: &mut Settings, sky: &mut Sky) -> Vec<Line> {
    let mut game = GameContext::new(player, world, settings, sky);
    toolkit().run(line, &mut game).expect("the toolkit handles it")
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

/// All the lines' text joined — for asserting on multi-line output.
fn joined(lines: &[Line]) -> String {
    lines.iter().map(Line::text).collect::<Vec<_>>().join("\n")
}

fn role(lines: &[Line]) -> Role {
    lines[0].spans().next().unwrap().role
}

const COMMAND_NAMES: [&str; 10] =
    ["tp", "bodies", "noclip", "pos", "inspect", "reactions", "walkspeed", "flyspeed", "cruise", "gravity"];

/// Registered in a build: the toolkit fills the commands registry, in `/help` order, and installs
/// its mod in the tools group.
#[test]
fn register_adds_the_commands_and_installs_the_toolkit() {
    let (mods, commands) = build();
    let names: Vec<&str> = commands.commands().iter().map(|c| c.name).collect();
    assert_eq!(names, COMMAND_NAMES);
    assert_eq!(mods.len(), 1);
    assert_eq!((mods.id(0), mods.name(0)), ("dev_toolkit", "Developer Toolkit"));
    assert_eq!(mods.package(0), Some("pwc.dev-toolkit"));
    assert!(mods.is_active(0), "the toolkit runs");
    let help: Vec<&str> = COMMANDS.iter().map(|(c, _)| c.help).collect();
    assert!(help.iter().all(|h| !h.is_empty()), "every command explains itself for /help");
}

/// Run one frame of the toolkit with the flight key pressed (or not), the camera attached or not.
fn fly_frame(toolkit: &mut DevToolkit, p: &mut Player, w: &mut World, pressed: bool, detached: bool) {
    let (mut s, mut sky) = (Settings::default(), Sky::new());
    let mut game = GameContext::new(p, w, &mut s, &mut sky);
    game.detached = detached;
    let mut ctx = FrameContext::new(game);
    if pressed {
        ctx.set_action(FLY);
    }
    toolkit.on_frame(&mut ctx);
}

/// F toggles walking and ordinary flight, never noclip, as an immediate action (so it works with
/// mod logic off), and never flies the frozen player behind a detached camera.
#[test]
fn f_toggles_walking_and_flying() {
    let (mut p, mut w) = (Player::new(DVec3::new(0.5, 80.0, 0.5)), world());
    let mut toolkit = DevToolkit;
    let [action] = toolkit.actions() else { panic!("one action") };
    assert_eq!((action.id, action.immediate), (FLY, true));
    assert!(action.default == [Chord::key(Key::F)]);
    for flying in [true, false, true, false] {
        fly_frame(&mut toolkit, &mut p, &mut w, true, false);
        assert_eq!(p.flying(), flying);
        assert!(!p.noclip());
    }
    fly_frame(&mut toolkit, &mut p, &mut w, false, false);
    assert!(!p.flying(), "no key, no toggle");
    fly_frame(&mut toolkit, &mut p, &mut w, true, true);
    assert!(!p.flying(), "a detached camera does not fly the frozen player");
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

/// A command the toolkit does not know is left to the registry's other commands (and then the
/// commands package's hint).
#[test]
fn unknown_commands_are_left_to_others() {
    let (mut p, mut w, mut s, mut sky) = (player(), world(), Settings::default(), Sky::new());
    let mut game = GameContext::new(&mut p, &mut w, &mut s, &mut sky);
    assert!(toolkit().run("fly-to-moon", &mut game).is_none());
    assert!(toolkit().run("name 1 pick", &mut game).is_none(), "the crafting stub is gone");
    assert!(toolkit().run("gfx", &mut game).is_none(), "/gfx moved to pwc.commands");
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
    let cosmos = w.terrain().cosmos().expect("a cosmos");
    let home = *cosmos.home();
    let name = home.name().to_string();
    let note = out.get(1).expect("an altitude note").text();
    assert!(note.contains(&format!("km above the ground of {name} (in space)")), "{note}");
    assert!(note.contains(&format!("/tp {name}")), "{note}");
    let km: f64 = note.split(' ').next().unwrap().parse().unwrap();
    let want = w.terrain().cosmos().unwrap().altitude(&home, p.position) * pwc_mod_api::math::BLOCK_METERS / 1000.0;
    assert!((km - want).abs() < 0.11, "{km} km, {want} km above its relaxed ground");
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
fn bodies_lists_names_and_classes_and_tp_lands_by_name() {
    let (mut p, mut w) = (player(), diffusion());
    p.position = DVec3::new(0.5, 80.0, 0.5);
    let cosmos = w.terrain().cosmos().expect("diffusion has a cosmos");
    let home = *cosmos.home();
    let bodies: Vec<_> = cosmos.bodies().to_vec();
    let round = *bodies.iter().find(|b| b.id != 0 && matches!(b.shape, Shape::Ball { .. })).expect("a round world");

    let lines = run("bodies", &mut p, &mut w);
    assert_eq!(lines.len(), bodies.len(), "one line per body");
    let first = lines[0].text();
    assert!(first.starts_with(&format!("{} (start world)", home.name())), "{first}");
    let home_at = landing(&w, &home);
    assert!(first.contains(&format!("/tp {} {} {}", home_at[0], home_at[1], home_at[2])), "{first}");
    let text = joined(&lines);
    for b in &bodies {
        assert!(text.contains(&format!("{} ({})", b.name(), b.traits.class())), "{} missing from {text}", b.name());
    }

    // By name (any case), standing in its pull.
    run(&format!("tp {}", round.name().to_ascii_lowercase()), &mut p, &mut w);
    let want = landing_vec(&w, &round);
    assert_eq!(p.position, want);
    assert!((p.position - round.centre_f()).length() <= round.reach());
    let pull = w.gravity_at(p.position).accel;
    let toward = (round.centre_f() - p.position).normalize();
    assert!(pull.normalize().dot(toward) > 0.99, "standing in its pull: {pull:?}");
    assert!(p.up().dot(-pull.normalize()) > 0.99, "up faces away from the pull");

    // The old kind names stay one release: `home` is the start world, landing just above its top.
    run("tp home", &mut p, &mut w);
    let alt = w.terrain().cosmos().expect("a cosmos").altitude(&home, p.position);
    assert!((1_000.0..4_100.0).contains(&alt), "home landing altitude {alt}");

    // A class word lands on the nth body of that class, in catalog order.
    let word = round.traits.class().split(' ').next().expect("a word").to_string();
    let named = |key: &str| bodies.iter().any(|b| b.name().to_ascii_lowercase().starts_with(key));
    if !named(&word) {
        let of_class: Vec<_> = bodies.iter().filter(|b| b.traits.class().split(' ').any(|w| w == word)).collect();
        run(&format!("tp {word}"), &mut p, &mut w);
        assert_eq!(p.position, landing_vec(&w, of_class[0]), "/tp {word}");
        if let Some(second) = of_class.get(1) {
            run(&format!("tp {word} 2"), &mut p, &mut w);
            assert_eq!(p.position, landing_vec(&w, second), "/tp {word} 2");
        }
    }

    let at = p.position;
    let bad = run("tp nope", &mut p, &mut w);
    assert_eq!(p.position, at);
    assert_eq!(role(&bad), Role::Danger);
    assert!(bad[0].text().contains("nope"), "{}", bad[0].text());
    // A letter two names start with, that no class word or old kind begins with, is ambiguous.
    let classes: Vec<&str> = bodies.iter().flat_map(|b| b.traits.class().split(' ')).collect();
    let shared = ('a'..='z').map(String::from).find(|k| {
        bodies.iter().filter(|b| b.name().to_ascii_lowercase().starts_with(k.as_str())).count() > 1
            && !classes.iter().any(|w| w.starts_with(k.as_str()))
    });
    if let Some(k) = shared {
        let ambiguous = run(&format!("tp {k}"), &mut p, &mut w);
        assert_eq!(p.position, at);
        assert!(ambiguous[0].text().contains("more than one"), "{}", ambiguous[0].text());
    }
}

/// On a sagging cube the landing sits just above its bowed face.
#[test]
fn tp_lands_over_a_warped_cube() {
    let mut w = World::with_kind(240, RenderConfig::default(), WorldgenKind::Diffusion, false);
    let mut p = player();
    let cube = *w.terrain().cosmos().expect("a cosmos").bodies().iter().find(|b| b.traits.form == Form::Warped).expect("a warped cube");
    let Shape::Cube { half } = cube.shape else { panic!("a warped body is a cube") };
    run(&format!("tp {}", cube.name()), &mut p, &mut w);
    assert_eq!(p.position, landing_vec(&w, &cube));
    // The face bows out (by a seventh of the half-size on a cube as weak as this one's).
    let rise = (p.position - cube.centre_f()).y - half as f64;
    assert!(rise > 2_000.0 && rise < 2_000.0 + 0.25 * half as f64, "landing {rise} over the bowed face");
}
