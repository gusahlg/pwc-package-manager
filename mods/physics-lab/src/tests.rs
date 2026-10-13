//! The lab end to end on a real generated world: capture, every law stepping, poke, rewind, the
//! HUD, and the world left untouched.

use super::*;
use pwc_mod_api::engine::DVec3;
use pwc_mod_api::world::World;

/// A generated world with its ground loaded, and a player standing on it at the origin column.
fn standing() -> (Player, World) {
    let mut w = World::generate();
    w.ensure_around(DVec3::new(0.5, 40.0, 0.5));
    let ground = (-64..120).rev().find(|&y| w.is_solid(0, y, 0)).expect("the origin column has ground");
    let p = Player::new(DVec3::new(0.5, f64::from(ground) + 2.62, 0.5));
    (p, w)
}

fn text(lines: &[Line]) -> String {
    lines.iter().map(Line::text).collect::<Vec<_>>().join("\n")
}

#[test]
fn every_law_runs_on_the_terrain_and_the_world_is_untouched() {
    let (p, w) = standing();
    let before: Vec<_> = (-8..8).flat_map(|x| (-8..8).map(move |y| (x, y))).map(|(x, y)| w.block_at(x, y, 3)).collect();
    for entry in ENTRIES {
        let mut lab = Lab::default();
        let out = text(&law(&mut lab, &p, &w, &[entry.id, "16"]));
        assert!(out.contains(entry.name), "{out}");
        let run = lab.run.as_mut().expect("running");
        assert!(run.law.matter().cells().iter().any(|&m| m != pwc_law_core::VOID), "{}: the box holds ground", entry.id);
        let start = run.law.digest();
        assert!(text(&poke(&mut lab, &p, &w, &["200"])).contains("poked"));
        let run = lab.run.as_mut().expect("running");
        for _ in 0..30 {
            run.law.step();
        }
        let mut hud = Vec::new();
        view::paint(run, &HudFacts::new((1280, 720)), &p, &mut hud);
        assert!(hud.len() > 3, "{}: the HUD paints the slice", entry.id);
        let rewound = text(&law(&mut lab, &p, &w, &["rewind", "30"]));
        let run = lab.run.as_ref().expect("running");
        if rewound.starts_with("rewound") {
            assert_ne!(run.law.digest(), start, "{}: the poke stays, the 30 ticks are undone", entry.id);
        }
        println!("{:10} {} | {}", entry.id, run.law.active(), rewound);
    }
    let after: Vec<_> = (-8..8).flat_map(|x| (-8..8).map(move |y| (x, y))).map(|(x, y)| w.block_at(x, y, 3)).collect();
    assert_eq!(before, after, "the lab never writes a cell");
}

#[test]
fn rewind_is_exact_for_the_reversible_laws() {
    let (p, w) = standing();
    for (id, strength) in [("toda", "500"), ("margolus", "3")] {
        let mut lab = Lab::default();
        law(&mut lab, &p, &w, &[id, "12"]);
        poke(&mut lab, &p, &w, &[strength]);
        let run = lab.run.as_mut().expect("running");
        let poked = run.law.digest();
        for _ in 0..50 {
            run.law.step();
        }
        assert_ne!(run.law.digest(), poked, "{id}: the poke moves");
        law(&mut lab, &p, &w, &["rewind", "50"]);
        assert_eq!(lab.run.as_ref().expect("running").law.digest(), poked, "{id}: rewind is exact");
    }
}

#[test]
fn commands_refuse_cleanly_without_a_run() {
    let (p, w) = standing();
    let mut lab = Lab::default();
    for args in [&["poke"][..], &["rewind"], &["ledger"], &["speed", "3"], &["nosuch"]] {
        let lines = if args[0] == "poke" { poke(&mut lab, &p, &w, &[]) } else { law(&mut lab, &p, &w, args) };
        assert_eq!(lines[0].spans().next().map(|s| s.role), Some(Role::Danger), "{args:?}");
    }
    assert!(text(&law(&mut lab, &p, &w, &[])).contains("zeldovich"));
}
