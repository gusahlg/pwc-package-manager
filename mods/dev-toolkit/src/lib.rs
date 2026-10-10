//! The Developer Toolkit: travel and inspection commands, registered with `pwc.commands`, and the
//! flight key.
//!
//! The commands keep their behaviour and output: travel (`/tp`, `/bodies`, `/noclip`, `/cruise`,
//! `/walkspeed`, `/flyspeed`, `/pos`) and looking at the world (`/inspect`, `/reactions`,
//! `/gravity`). The options commands (`/gfx`, `/time`, the audio commands) and `/help` moved to
//! `pwc.commands`. `F` toggles walking and flying, as this mod's own immediate action. A command
//! only edits the game state it is handed; the core follows up (streams a teleport's destination,
//! tells a server).

use pwc_commands::{Command, CommandFn, CommandsHandle};
use pwc_mod_api::engine::{DVec3, Key};
use pwc_mod_api::input::intent::Chord;
use pwc_mod_api::ui::{Line, Role};
use pwc_mod_api::{Action, FrameContext, GameContext, Group, Mod, ModRegistrar};

mod inspect;
mod travel;

#[cfg(test)]
mod tests;

/// The mods-screen group of developer tools.
pub const TOOLS: Group = Group { id: "tools", name: "Tools", description: "Developer tools: chat commands and flight." };

/// The id of the flight action.
pub const FLY: &str = "toolkit.fly";
const FLY_CHORDS: &[Chord] = &[Chord::key(Key::F)];
const ACTIONS: &[Action] = &[Action {
    id: FLY,
    label: "Fly (toggle)",
    default: FLY_CHORDS,
    repeat: false,
    held: false,
    immediate: true,
}];

/// The package entry point: declares the tools group, adds the toolkit's commands to
/// `pwc.commands` and installs the toolkit.
pub fn register(registrar: &mut ModRegistrar) {
    let commands = registrar
        .get::<CommandsHandle>()
        .expect("pwc.dev-toolkit needs the commands handle from pwc.commands (a declared dependency registered first)");
    add_commands(&commands);
    registrar.declare_group(TOOLS);
    registrar.add(DevToolkit);
}

/// Add every toolkit command to `commands`, in the order `/help` lists them.
pub fn add_commands(commands: &CommandsHandle) {
    for (command, handler) in COMMANDS {
        commands.add(*command, *handler);
    }
}

/// The toolkit mod (id `dev_toolkit`): the flight key.
pub struct DevToolkit;

impl Mod for DevToolkit {
    fn name(&self) -> &str {
        "Developer Toolkit"
    }

    fn id(&self) -> &'static str {
        "dev_toolkit"
    }

    fn description(&self) -> &str {
        "Chat commands (/tp, /cruise, /inspect...) and flight on F."
    }

    fn group(&self) -> &'static str {
        TOOLS.id
    }

    fn actions(&self) -> &[Action] {
        ACTIONS
    }

    /// `F` toggles walking and flying (never noclip), on the frame of the press whatever the mod
    /// cadence, but never while a detached camera holds the player.
    fn on_frame(&mut self, ctx: &mut FrameContext) {
        if ctx.action(FLY) && !ctx.game.detached {
            ctx.game.player.toggle_fly();
        }
    }
}

fn shown(lines: Vec<String>) -> Vec<Line> {
    lines.into_iter().map(|l| Line::of(Role::Dim, l)).collect()
}

fn rejected(lines: Vec<String>) -> Vec<Line> {
    lines.into_iter().map(|l| Line::of(Role::Danger, l)).collect()
}

/// The command table from one list, so a command is added in one place.
macro_rules! commands {
    (
        $ctx:ident, $args:ident;
        $($name:literal $(| $alias:literal)* , $usage:literal, $help:literal => $body:expr);+ $(;)?
    ) => {
        /// Every toolkit command with its handler, in the order `/help` lists them.
        pub const COMMANDS: &[(Command, CommandFn)] = &[$((
            Command { name: $name, aliases: &[$($alias),*], args: $usage, help: $help },
            {
                #[allow(unused_variables)]
                fn run($ctx: &mut GameContext, $args: &[&str]) -> Vec<Line> {
                    $body
                }
                run
            },
        )),+];
    };
}

commands! {
    ctx, args;
    "tp" | "teleport" | "setpos", "<x y z|name>", "teleport to coordinates or a body" => travel::teleport(args, ctx.player, ctx.world);
    "bodies", "", "list the worlds, nearest first" => travel::bodies(ctx.player, ctx.world);
    "noclip", "", "toggle flight through geometry" => travel::noclip(ctx.player);
    "pos" | "where", "", "show current coordinates" => shown(vec![format!("position: {}", fmt_pos(ctx.player.position))]);
    "inspect" | "look", "[x y z]", "describe a block's elements & properties" => inspect::inspect(args, ctx.player, ctx.world);
    "reactions", "", "show pending reaction events" => inspect::reactions(ctx.world);
    "walkspeed", "[n]", "show or set ground walk speed" => travel::walkspeed(args, ctx.player);
    "flyspeed", "[n]", "show or set flying speed" => travel::flyspeed(args, ctx.player);
    "cruise", "[km/s|off]", "space travel (default 100000 km/s) with the world held still" => travel::cruise(args, ctx.player, ctx.world);
    "gravity" | "g", "", "show the local pull of the matter around you" => inspect::gravity(ctx.player, ctx.world);
}

/// Format a position the same way the on-screen coordinate readout does.
fn fmt_pos(p: DVec3) -> String {
    format!("X {:.1}  Y {:.1}  Z {:.1}", p.x, p.y, p.z)
}
