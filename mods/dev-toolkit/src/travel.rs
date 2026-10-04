//! Getting around: `/tp`, `/bodies`, `/noclip`, `/cruise`, `/walkspeed` and `/flyspeed`.

use pwc_mod_api::engine::DVec3;
use pwc_mod_api::math::{BLOCK_METERS, WORLD_BORDER};
use pwc_mod_api::player::{Player, CRUISE_DEFAULT, CRUISE_MAX, LIGHT_SPEED, MAX_SPEED, STANDARD_GRAVITY};
use pwc_mod_api::ui::Line;
use pwc_mod_api::world::terrain::cosmos::{Body, Cosmos, Kind, Shape};
use pwc_mod_api::world::World;

use crate::{fmt_pos, rejected, shown};

/// `tp <x> <y> <z>` or `tp <name> [n]` — move the player, clamped to the
/// ±[`WORLD_BORDER`] cube (the same clamp movement applies, so no code path can
/// carry a position that would overflow i32 block math). The output reports the
/// position actually landed on, clamp included.
///
/// The discontinuity is transactional: collision data around the destination
/// is *requested* before the player lands there, and physics stays frozen
/// until [`World::spawn_ready`] is true, so the next physics step never runs
/// against unloaded air.
pub(crate) fn teleport(args: &[&str], player: &mut Player, world: &mut World) -> Vec<Line> {
    let numeric = args.first().is_some_and(|a| a.parse::<f64>().is_ok());
    if numeric || args.len() == 3 {
        return teleport_coords(args, player, world);
    }
    match args {
        [name] => teleport_named(name, 1, player, world),
        [name, n] => match n.parse::<usize>() {
            Ok(n) if n >= 1 => teleport_named(name, n, player, world),
            _ => tp_usage(),
        },
        _ => tp_usage(),
    }
}

fn tp_usage() -> Vec<Line> {
    rejected(vec!["usage: /tp <x> <y> <z>  or  /tp <name> [n]".to_string()])
}

fn teleport_coords(args: &[&str], player: &mut Player, world: &mut World) -> Vec<Line> {
    if args.len() != 3 {
        return rejected(vec!["usage: /tp <x> <y> <z>".to_string()]);
    }
    let parsed: Result<Vec<f64>, _> = args.iter().map(|a| a.parse::<f64>()).collect();
    match parsed.as_deref() {
        Ok([x, y, z]) if x.is_finite() && y.is_finite() && z.is_finite() => {
            place(DVec3::new(*x, *y, *z), player, world)
        }
        _ => rejected(vec!["/tp: x, y and z must be numbers".to_string()]),
    }
}

/// Land on the `n`th body of a kind (1-based, catalog order). A unique prefix of the kind name
/// is enough. Worlds without a cosmos are left untouched.
fn teleport_named(name: &str, n: usize, player: &mut Player, world: &mut World) -> Vec<Line> {
    let Some(cosmos) = world.terrain().cosmos() else {
        return no_cosmos();
    };
    let kind = match resolve_kind(name) {
        Ok(kind) => kind,
        Err(lines) => return lines,
    };
    match cosmos.bodies().iter().filter(|b| b.kind == kind).nth(n - 1) {
        Some(body) => {
            let at = landing_vec(world, body);
            place(at, player, world)
        }
        None => rejected(vec![format!("no {} {n}", kind.name())]),
    }
}

/// Stand the player at `target` the way coordinate `/tp` does.
fn place(target: DVec3, player: &mut Player, world: &mut World) -> Vec<Line> {
    let target = target.clamp(DVec3::splat(-WORLD_BORDER), DVec3::splat(WORLD_BORDER));
    world.prepare_around(target);
    player.position = target;
    // Stand up along the local gravity at once (no slow roll after a jump across the
    // universe); in weightlessness keep the current frame.
    // The body frame follows the last applied pull: make it the destination's at once.
    let pull = world.gravity_at(target);
    player.gravity = pull.accel;
    if let Some(up) = pull.up(0.02 * STANDARD_GRAVITY) {
        player.snap_up(up);
    }
    // Cancel any accumulated fall so the player doesn't rocket down on arrival.
    player.cancel_fall();
    let mut lines = vec![format!("teleported to {}", fmt_pos(player.position))];
    lines.extend(altitude_note(world, player.position));
    shown(lines)
}

/// How far above the nearest body's ground a teleport left the player, when that is far: a round
/// world curves away, so 1.4 million blocks from spawn its ground is some 50 km below y = 200.
fn altitude_note(world: &World, p: DVec3) -> Option<String> {
    let cosmos = world.terrain().cosmos()?;
    let body = cosmos.body_at(p)?;
    let alt = cosmos.altitude(body, p);
    if alt < 2_000.0 {
        return None;
    }
    let km = alt * BLOCK_METERS / 1000.0;
    let air = if world.in_air(p) { "in the air" } else { "in space" };
    let name = body.kind.name();
    Some(format!("{km:.1} km above the ground of {name} ({air}); /tp {name} lands on its surface"))
}

fn no_cosmos() -> Vec<Line> {
    rejected(vec!["this world has no cosmos".to_string()])
}

/// `/noclip` — from walking or ordinary flight into noclip flight, and from noclip back to walking.
pub(crate) fn noclip(player: &mut Player) -> Vec<Line> {
    player.toggle_noclip();
    shown(vec![format!("noclip {}", if player.noclip() { "on" } else { "off" })])
}

/// `/bodies` — every cosmos body, nearest first. The number is 1-based within the kind, in
/// catalog order. The `/tp` lands 2000 blocks above the +Y datum.
pub(crate) fn bodies(player: &Player, world: &World) -> Vec<Line> {
    let Some(cosmos) = world.terrain().cosmos() else {
        return no_cosmos();
    };
    let origin = player.position;
    let mut order: Vec<usize> = (0..cosmos.bodies().len()).collect();
    order.sort_by(|&i, &j| {
        let di = (cosmos.bodies()[i].centre_f() - origin).length_squared();
        let dj = (cosmos.bodies()[j].centre_f() - origin).length_squared();
        di.total_cmp(&dj).then(i.cmp(&j))
    });
    let lines = order
        .into_iter()
        .map(|i| {
            let body = &cosmos.bodies()[i];
            let n = kind_number(cosmos, body);
            let dist = (body.centre_f() - origin).length();
            let at = landing(world, body);
            format!(
                "{} {n}  {} away  {}  /tp {} {} {}",
                body.kind.name(),
                fmt_dist(dist),
                fmt_size(body),
                at[0],
                at[1],
                at[2],
            )
        })
        .collect();
    shown(lines)
}

/// 1-based index of `body` among bodies of its kind, in catalog order.
fn kind_number(cosmos: &Cosmos, body: &Body) -> usize {
    cosmos.bodies().iter().filter(|b| b.kind == body.kind).position(|b| b.id == body.id).unwrap() + 1
}

fn resolve_kind(prefix: &str) -> Result<Kind, Vec<Line>> {
    let key = prefix.to_ascii_lowercase();
    let mut hit = None;
    for kind in [Kind::Home, Kind::Twin, Kind::Verdant, Kind::Hollow, Kind::Ember, Kind::Moon] {
        if kind.name().starts_with(&key) {
            if hit.is_some() {
                return Err(rejected(vec![format!("'{prefix}' matches more than one kind")]));
            }
            hit = Some(kind);
        }
    }
    hit.ok_or_else(|| rejected(vec![format!("unknown body '{prefix}'")]))
}

/// Cube half-edge, ball radius, or shell outer radius, then 2000 blocks of clearance on +Y.
/// 2000 blocks above the body's real top along +Y: a relaxed round body's datum offset there, or a
/// warped cube's bowed face (the warp of its reference top).
pub(crate) fn landing(world: &World, body: &Body) -> [i64; 3] {
    let up = DVec3::Y;
    let top = match body.shape {
        Shape::Cube { half } => {
            let warp = world.atlases().iter().find_map(|a| a.grid.as_ref().is_some_and(|g| g.body == body.id).then(|| a.warp.as_ref()).flatten());
            match warp {
                Some(w) => (w.apply(body.centre_f() + up * half as f64) - body.centre_f()).y,
                None => half as f64,
            }
        }
        Shape::Ball { r } => {
            let off = world.terrain().cosmos().map_or(0.0, |c| c.surface_offset(body, body.centre_f() + up * r as f64));
            r as f64 + off
        }
        Shape::Shell { outer, .. } => outer as f64,
    };
    [body.centre[0], body.centre[1] + top.round() as i64 + 2_000, body.centre[2]]
}

pub(crate) fn landing_vec(world: &World, body: &Body) -> DVec3 {
    let at = landing(world, body);
    DVec3::new(at[0] as f64, at[1] as f64, at[2] as f64)
}

fn fmt_size(body: &Body) -> String {
    match body.shape {
        Shape::Cube { half } => format!("half {half}"),
        Shape::Ball { r } => format!("radius {r}"),
        Shape::Shell { outer, .. } => format!("radius {outer}"),
    }
}

/// Rounded distance with a `k` or `M` suffix.
fn fmt_dist(d: f64) -> String {
    let d = d.abs();
    if d >= 999_500.0 {
        format!("{}M", (d / 1_000_000.0).round() as i64)
    } else if d >= 999.5 {
        format!("{}k", (d / 1_000.0).round() as i64)
    } else {
        format!("{}", d.round() as i64)
    }
}

/// `cruise [n|off]` — start cruising (at `n` km/s, default [`CRUISE_DEFAULT`]), change
/// the speed of a cruise, or end it (`off`, or no argument while cruising). Ending lands like a
/// teleport at the current point, lifted clear of the ground when it is inside a world.
pub(crate) fn cruise(args: &[&str], player: &mut Player, world: &mut World) -> Vec<Line> {
    // Typed and shown in km/s; the player's cruise speed is in units/second.
    let to_units = |km_s: f64| km_s * 1000.0 / BLOCK_METERS;
    let to_km_s = |v: f64| v * BLOCK_METERS / 1000.0;
    let max_km_s = to_km_s(CRUISE_MAX);
    let speed = match args {
        [] if player.cruising() => None,
        [] => Some(CRUISE_DEFAULT),
        ["off"] => None,
        [value] => match value.replace(['_', ','], "").parse::<f64>() {
            Ok(km_s) if km_s.is_finite() && km_s > 0.0 && km_s <= max_km_s * (1.0 + 1e-9) => Some(to_units(km_s).min(CRUISE_MAX)),
            Ok(km_s) if km_s.is_finite() && km_s > 0.0 => {
                return rejected(vec![format!("/cruise: at most {max_km_s:.0} km/s (ten times the speed of light)")]);
            }
            _ => return rejected(vec!["/cruise: speed must be a positive number of km/s, or off".to_string()]),
        },
        _ => return rejected(vec!["usage: /cruise [<km/s>|off]".to_string()]),
    };
    match speed {
        Some(v) => {
            let started = !player.cruising();
            player.start_cruise(v);
            let c = v / LIGHT_SPEED;
            let mut lines = vec![format!("cruising at {:.0} km/s ({c:.3} c); /cruise off to stop", to_km_s(v))];
            if started {
                lines.push("the world holds still: forward flies where you look, through everything".into());
            }
            shown(lines)
        }
        None if player.end_cruise() => {
            let target = world.clear_of_ground(player.position);
            place(target, player, world)
        }
        None => rejected(vec!["/cruise: not cruising".to_string()]),
    }
}

/// `walkspeed [n]` — show or set the player's ground walk speed, units/second.
pub(crate) fn walkspeed(args: &[&str], player: &mut Player) -> Vec<Line> {
    set_speed(args, "/walkspeed", player, |p| &mut p.speed)
}

/// `flyspeed [n]` — show or set the player's flying speed, units/second.
pub(crate) fn flyspeed(args: &[&str], player: &mut Player) -> Vec<Line> {
    set_speed(args, "/flyspeed", player, |p| &mut p.fly_speed)
}

/// Shared show/set logic for `walkspeed`/`flyspeed`: both just target a different
/// intrinsic on [`Player`], so the parse-validate-write-confirm shape lives once.
fn set_speed(
    args: &[&str],
    name: &str,
    player: &mut Player,
    field: impl FnOnce(&mut Player) -> &mut f64,
) -> Vec<Line> {
    match args {
        [] => shown(vec![format!("{name}: {:.2}", *field(player))]),
        [value] => match value.parse::<f64>() {
            Ok(v) if v.is_finite() && v > 0.0 && v <= MAX_SPEED => {
                *field(player) = v;
                shown(vec![format!("{name} set to {v:.2}")])
            }
            Ok(v) if v.is_finite() && v > MAX_SPEED => rejected(vec![format!(
                "{name}: at most {:.0} (100 km/s); use /tp to travel farther",
                MAX_SPEED
            )]),
            _ => rejected(vec![format!("{name}: value must be a positive number")]),
        },
        _ => rejected(vec![format!("usage: {name} [<units/second>]")]),
    }
}
