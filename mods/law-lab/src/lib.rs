//! The Law Lab: run a candidate fundamental law on the real terrain around the player.
//!
//! `/law <id>` captures a box of the world centred on the player's feet into a
//! `pwc_law_core::Matter` (the same configurations the world holds), builds that candidate on it and
//! steps it every mod tick. The HUD draws the slice the player stands on as a heat map. `/poke`
//! (or `P`) acts on the looked-at cell in the law's own terms; `/law rewind` runs a reversible law
//! backwards. The lab owns a copy: it never writes a cell, so the world's own law and every other
//! mod are untouched. It is a window onto other universes, not an authority.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use pwc_commands::{Command, CommandsHandle};
use pwc_law_core::{Candidate, Entry};
use pwc_mod_api::engine::Key;
use pwc_mod_api::input::intent::Chord;
use pwc_mod_api::player::Player;
use pwc_mod_api::ui::{HudElement, Line, Role};
use pwc_mod_api::world::World;
use pwc_mod_api::{Action, FrameContext, HudFacts, Mod, ModContext, ModRegistrar};

mod capture;
mod view;

#[cfg(test)]
mod tests;

pub use capture::{capture, looked_at, Window};

/// Every candidate this build knows, in the order `/law` lists them.
pub const ENTRIES: &[Entry] = &[
    pwc_law_demon::ENTRY,
    pwc_law_belousov::ENTRY,
    pwc_law_toda::ENTRY,
    pwc_law_margolus::ENTRY,
    pwc_law_zeldovich::ENTRY,
    pwc_law_polyakov::ENTRY,
];

/// The id of the poke action.
pub const POKE: &str = "lab.poke";
const POKE_CHORDS: &[Chord] = &[Chord::key(Key::P)];
const ACTIONS: &[Action] =
    &[Action { id: POKE, label: "Poke the lab's law", default: POKE_CHORDS, repeat: false, held: false, immediate: true }];

/// Box side when `/law <id>` names none, and the largest allowed.
const DEFAULT_SIDE: u32 = 32;
const MAX_SIDE: u32 = 64;
/// Wall time the lab may spend stepping per mod tick: the law slows down, the game never does.
const STEP_BUDGET: Duration = Duration::from_millis(4);

/// One running experiment.
pub struct Run {
    /// Which law.
    pub entry: Entry,
    /// The law on its captured box.
    pub law: Box<dyn Candidate>,
    /// Where the box sits in the world.
    pub window: Window,
    /// Ticks run (net of rewinds).
    pub ticks: i64,
    /// Ticks wanted per mod tick; 0 is paused.
    pub speed: u32,
    /// Slice offset along the up axis from the cell under the feet.
    pub depth: i32,
    /// Strength of a poke.
    pub strength: i32,
}

/// The lab's shared state: the mod and its commands both hold it.
#[derive(Default)]
pub struct Lab {
    /// The running experiment, if any.
    pub run: Option<Run>,
    /// A poke asked for by the key, applied at the next update (which has the world).
    poke: bool,
}

/// Shared handle.
pub type LabHandle = Rc<RefCell<Lab>>;

/// The package entry point: adds `/law` and `/poke` and installs the lab.
pub fn register(registrar: &mut ModRegistrar) {
    let commands = registrar
        .get::<CommandsHandle>()
        .expect("pwc.law-lab needs the commands handle from pwc.commands (a declared dependency registered first)");
    let lab = LabHandle::default();
    add_commands(&commands, &lab);
    registrar.add(PhysicsLab { lab });
}

/// Add `/law` and `/poke` over a shared lab.
pub fn add_commands(commands: &CommandsHandle, lab: &LabHandle) {
    let shared = lab.clone();
    commands.add(
        Command {
            name: "law",
            aliases: &["lab"],
            args: "[id [side]|off|pause|speed n|rewind n|depth n|ledger]",
            help: "run a candidate fundamental law on the terrain around you",
        },
        move |ctx, args| law(&mut shared.borrow_mut(), ctx.player, ctx.world, args),
    );
    let shared = lab.clone();
    commands.add(
        Command { name: "poke", aliases: &[], args: "[strength]", help: "act on the looked-at cell in the lab law's terms" },
        move |ctx, args| poke(&mut shared.borrow_mut(), ctx.player, ctx.world, args),
    );
}

/// The lab mod (id `law_lab`).
pub struct PhysicsLab {
    lab: LabHandle,
}

impl Mod for PhysicsLab {
    fn name(&self) -> &str {
        "Law Lab"
    }

    fn id(&self) -> &'static str {
        "law_lab"
    }

    fn actions(&self) -> &[Action] {
        ACTIONS
    }

    fn reset(&mut self) {
        *self.lab.borrow_mut() = Lab::default();
    }

    fn on_frame(&mut self, ctx: &mut FrameContext) {
        if ctx.action(POKE) {
            self.lab.borrow_mut().poke = true;
        }
    }

    /// Step the law within the wall-time budget, and apply a pending key poke.
    fn update(&mut self, ctx: &mut ModContext) {
        let mut lab = self.lab.borrow_mut();
        if std::mem::take(&mut lab.poke) {
            let _ = poke(&mut lab, ctx.player, ctx.world, &[]);
        }
        let Some(run) = lab.run.as_mut() else { return };
        let start = Instant::now();
        for _ in 0..run.speed {
            run.law.step();
            run.ticks += 1;
            if start.elapsed() > STEP_BUDGET {
                break;
            }
        }
    }

    fn hud(&self, facts: &HudFacts, _world: &World, player: &Player, out: &mut Vec<HudElement>) {
        if !facts.hud_mode.shows_mod_hud() {
            return;
        }
        if let Some(run) = self.lab.borrow().run.as_ref() {
            view::paint(run, facts, player, out);
        }
    }
}

fn say(lines: Vec<String>) -> Vec<Line> {
    lines.into_iter().map(|l| Line::of(Role::Dim, l)).collect()
}

fn refuse(text: impl Into<String>) -> Vec<Line> {
    vec![Line::of(Role::Danger, text)]
}

/// `/law ...`.
fn law(lab: &mut Lab, player: &Player, world: &World, args: &[&str]) -> Vec<Line> {
    let number = |i: usize| args.get(i).and_then(|a| a.parse::<i64>().ok());
    match args.first().copied() {
        None => {
            let mut lines = vec!["candidate laws (/law <id> [side] runs one here):".to_string()];
            lines.extend(ENTRIES.iter().map(|e| format!("  {} — {}: {}", e.id, e.name, e.summary)));
            if let Some(run) = &lab.run {
                lines.push(status(run));
            }
            say(lines)
        }
        Some("off") => {
            lab.run = None;
            say(vec!["lab stopped".into()])
        }
        Some("pause") => match lab.run.as_mut() {
            Some(run) => {
                run.speed = u32::from(run.speed == 0);
                say(vec![if run.speed == 0 { "paused".into() } else { "running".into() }])
            }
            None => refuse("no law is running"),
        },
        Some("speed") => match (lab.run.as_mut(), number(1)) {
            (Some(run), Some(n)) => {
                run.speed = n.clamp(0, 64) as u32;
                say(vec![format!("{} ticks per mod tick", run.speed)])
            }
            _ => refuse("usage: /law speed <0..64> while a law runs"),
        },
        Some("depth") => match (lab.run.as_mut(), number(1)) {
            (Some(run), Some(n)) => {
                run.depth = n.clamp(-(MAX_SIDE as i64), MAX_SIDE as i64) as i32;
                say(vec![format!("showing the slice {} below the feet", -run.depth + 1)])
            }
            _ => refuse("usage: /law depth <n> while a law runs"),
        },
        Some("rewind") => match lab.run.as_mut() {
            Some(run) => {
                let want = number(1).unwrap_or(20).clamp(1, 100_000);
                let mut done = 0;
                while done < want && run.law.unstep() {
                    done += 1;
                }
                run.ticks -= done;
                if done == 0 {
                    refuse(format!("{} cannot run backwards: its law forgets", run.entry.name))
                } else {
                    run.speed = 0;
                    say(vec![format!("rewound {done} ticks exactly (paused); {}", status(run))])
                }
            }
            None => refuse("no law is running"),
        },
        Some("ledger") => match &lab.run {
            Some(run) => {
                let mut lines = vec![status(run)];
                lines.extend(run.law.ledger().entries.iter().map(|(n, v)| format!("  {n} = {v}")));
                say(lines)
            }
            None => refuse("no law is running"),
        },
        Some(id) => {
            let Some(entry) = ENTRIES.iter().find(|e| e.id == id) else {
                return refuse(format!("no law '{id}'; /law lists them"));
            };
            let side = number(1).map_or(DEFAULT_SIDE, |n| n.clamp(4, MAX_SIDE as i64) as u32);
            let (matter, window) = capture(world, player, side);
            let solid = matter.cells().iter().filter(|&&m| m != pwc_law_core::VOID).count();
            let law = (entry.build)(matter);
            let bytes = law.state_bytes_per_cell();
            lab.run = Some(Run { entry: *entry, law, window, ticks: 0, speed: 1, depth: 0, strength: 64 });
            say(vec![format!(
                "{} runs on a {side}³ copy of the world here ({solid} matter cells, {bytes} bytes of state per cell). /poke or P acts on it; the world itself is untouched.",
                entry.name
            )])
        }
    }
}

/// `/poke [strength]`: act on the looked-at cell, or the box centre when nothing in the box is
/// looked at.
fn poke(lab: &mut Lab, player: &Player, world: &World, args: &[&str]) -> Vec<Line> {
    let Some(run) = lab.run.as_mut() else { return refuse("no law is running; /law <id> starts one") };
    if let Some(s) = args.first().and_then(|a| a.parse::<i32>().ok()) {
        run.strength = s;
    }
    let target = looked_at(world, player).and_then(|p| run.window.index(p));
    let i = target.unwrap_or_else(|| run.window.centre());
    run.law.poke(i, run.strength);
    let (x, y, z) = run.window.world(i);
    say(vec![format!("poked {} at {x} {y} {z} with {}", run.entry.id, run.strength)])
}

fn status(run: &Run) -> String {
    let side = run.law.matter().grid.nx;
    format!(
        "{} on {side}³: tick {}, {} active cells, {}",
        run.entry.name,
        run.ticks,
        run.law.active(),
        if run.speed == 0 { "paused".to_string() } else { format!("{} ticks per mod tick", run.speed) }
    )
}
