//! Looking at the world: `/inspect`, `/reactions` and `/gravity`.

use pwc_mod_api::math::BLOCK_METERS;
use pwc_mod_api::player::{Player, STANDARD_GRAVITY};
use pwc_mod_api::ui::Line;
use pwc_mod_api::world::World;

use crate::{rejected, shown};

/// `/inspect [x y z]` — a block's elements and readings: the one supporting the player, or the
/// one at the given cell.
pub(crate) fn inspect(args: &[&str], player: &Player, world: &World) -> Vec<Line> {
    let cell = match args {
        [] => {
            // The block supporting the player: 0.1 along −up, storage −Y on a chart.
            world.ground_cell(player.feet(), player.up_axis)
        }
        [x, y, z] => match (x.parse(), y.parse(), z.parse()) {
            (Ok(x), Ok(y), Ok(z)) => (x, y, z),
            _ => return rejected(vec!["/inspect: x, y and z must be integers".to_string()]),
        },
        _ => return rejected(vec!["usage: /inspect [<x> <y> <z>]".to_string()]),
    };

    let (x, y, z) = cell;
    let id = world.block_at(x, y, z);
    let registry = world.registry();
    let cfg = registry.configuration(id);
    let obs = registry.observation(id);
    let words = registry.display_name(id);
    // Labels are worldgen roles ("rock:1"), an internal annotation; the console shows them as such.
    let role = registry.label(id).map(|l| format!(", worldgen role {l}")).unwrap_or_default();
    let elems: Vec<String> = cfg
        .elements()
        .iter()
        .map(|e| format!("[{},{},{},{}]", e.0[0], e.0[1], e.0[2], e.0[3]))
        .collect();
    let made = if elems.is_empty() {
        "void".to_string()
    } else {
        elems.join(" + ")
    };
    shown(vec![
        format!("block at {x} {y} {z}: {words} (#{}{role})", id.0),
        format!("  made of: {made}"),
        format!(
            "  solid {}  transparency {}  emission {}",
            obs.solid as u8, obs.transparency, obs.emission
        ),
        format!(
            "  hardness {}  friction {}  cohesion {}",
            obs.hardness, obs.friction, obs.cohesion
        ),
        format!("  descriptor {}", registry.render_layer(id)),
    ])
}

/// `/reactions` — active contacts, turns run, law operations committed.
pub(crate) fn reactions(world: &World) -> Vec<Line> {
    let r = world.reactions();
    shown(vec![format!(
        "reactions: active={} turns={} operations={}",
        r.pending(),
        r.turns,
        r.operations
    )])
}

/// `/gravity` — the field at the player: strength, direction, tilt from the ground's grid axis, the
/// potential, the declared error and the source epoch.
pub(crate) fn gravity(player: &Player, world: &World) -> Vec<Line> {
    let s = world.gravity().sample_tidal(player.position);
    let g = s.accel.length();
    let metres = BLOCK_METERS;
    let mut out = vec![format!(
        "gravity: {:.3} m/s² ({:.1} % of the spawn pull)",
        g * metres,
        100.0 * g / STANDARD_GRAVITY
    )];
    match s.up(0.02 * STANDARD_GRAVITY) {
        Some(up) => {
            let n = player.up_axis.dvec();
            let tilt = up.dot(n).clamp(-1.0, 1.0).acos().to_degrees();
            out.push(format!("down: ({:.4}, {:.4}, {:.4}); {tilt:.3}° off the {:?} grid axis", -up.x, -up.y, -up.z, player.up_axis));
        }
        None => out.push("weightless: the body keeps its orientation".to_string()),
    }
    out.push(format!(
        "potential {:.4e} blocks²/s², error ≤ {:.2e} m/s², source epoch {}",
        s.potential,
        s.error * metres,
        s.epoch
    ));
    shown(out)
}
