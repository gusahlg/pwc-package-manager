//! The Developer Toolkit: the console commands and the flight key, which the base game leaves to
//! mods.
//!
//! Every command the game used to ship moved here with its behaviour and output unchanged: travel
//! (`/tp`, `/bodies`, `/noclip`, `/cruise`, `/walkspeed`, `/flyspeed`, `/pos`), looking at the
//! world (`/inspect`, `/reactions`, `/gravity`), options (`/gfx`, `/time`, `/mute`, `/deafen`,
//! `/audio`, `/voicetest`) and `/help`, which lists the commands of every enabled mod. `F` toggles
//! walking and flying. A command only edits the game state it is handed; the core follows up
//! (applies and saves changed settings, streams a teleport's destination, tells a server).

use pwc_mod_api::engine::DVec3;
use pwc_mod_api::ui::{Line, Role};
use pwc_mod_api::player::Player;
use pwc_mod_api::world::World;
use pwc_mod_api::{Command, CommandContext, Group, Mod, ModRegistrar};

mod inspect;
mod options;
mod travel;

#[cfg(test)]
mod tests;

/// The mods-screen group of developer tools.
pub const TOOLS: Group = Group { id: "tools", name: "Tools", description: "Developer tools: console commands and flight." };

/// The package entry point: declares the tools group and installs the toolkit.
pub fn register(registrar: &mut ModRegistrar) {
    registrar.declare_group(TOOLS);
    registrar.add(DevToolkit);
}

/// The toolkit mod (id `dev_toolkit`).
pub struct DevToolkit;

impl Mod for DevToolkit {
    fn name(&self) -> &str {
        "Developer Toolkit"
    }

    fn id(&self) -> &'static str {
        "dev_toolkit"
    }

    fn description(&self) -> &str {
        "Console commands (/tp, /gfx, /time, /help...) and flight on F."
    }

    fn group(&self) -> &'static str {
        TOOLS.id
    }

    fn commands(&self) -> &[Command] {
        COMMANDS
    }

    fn run_command(&mut self, ctx: &mut CommandContext<'_>, cmd: &str, args: &[&str]) -> Option<Vec<Line>> {
        dispatch(ctx, cmd, args)
    }

    fn on_toggle_fly(&mut self, player: &mut Player, _world: &World) -> bool {
        player.toggle_fly();
        true
    }
}

fn shown(lines: Vec<String>) -> Vec<Line> {
    lines.into_iter().map(|l| Line::of(Role::Dim, l)).collect()
}

fn rejected(lines: Vec<String>) -> Vec<Line> {
    lines.into_iter().map(|l| Line::of(Role::Danger, l)).collect()
}

/// The command table and its dispatch from one list, so a command is added in one place.
macro_rules! commands {
    (
        $ctx:ident, $args:ident;
        $($name:literal $(| $alias:literal)* , $usage:literal, $help:literal => $body:expr);+ $(;)?
    ) => {
        /// Every command, in the order `/help` lists them.
        pub const COMMANDS: &[Command] = &[$(Command { name: $name, args: $usage, help: $help }),+];

        /// Run `cmd` (a name or an alias); `None` leaves it to the other mods.
        fn dispatch($ctx: &mut CommandContext<'_>, cmd: &str, $args: &[&str]) -> Option<Vec<Line>> {
            Some(match cmd {
                $($name $(| $alias)* => $body,)+
                _ => return None,
            })
        }
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
    "gfx" | "graphics", "[setting value]", "show or change graphics settings" => options::gfx(args, ctx.settings, ctx.visuals);
    "time", "[set|length]", "show or set the day/night clock" => options::time(args, ctx.sky);
    "walkspeed", "[n]", "show or set ground walk speed" => travel::walkspeed(args, ctx.player);
    "flyspeed", "[n]", "show or set flying speed" => travel::flyspeed(args, ctx.player);
    "cruise", "[km/s|off]", "space travel (default 100000 km/s) with the world held still" => travel::cruise(args, ctx.player, ctx.world);
    "mute", "", "toggle master mute (this session)" => options::mute(ctx.settings);
    "deafen", "", "toggle hearing incoming voice" => options::deafen(ctx.settings);
    "audio" | "volume", "<chan> <0-100>", "set master/effects/voice volume" => options::audio(args, ctx.settings);
    "voicetest", "", "play a local voice test cue" => options::voicetest(ctx);
    "gravity" | "g", "", "show the local pull of the matter around you" => inspect::gravity(ctx.player, ctx.world);
    "help" | "?", "", "show this list" => help(ctx.commands);
}

/// `/help` — every enabled mod's commands, in install order.
fn help(commands: &[Command]) -> Vec<Line> {
    let mut lines = vec!["commands (a leading '/' is optional):".to_string()];
    lines.extend(commands.iter().map(|c| {
        let usage = if c.args.is_empty() { c.name.to_string() } else { format!("{} {}", c.name, c.args) };
        format!("  /{usage:<20} {}", c.help)
    }));
    shown(lines)
}

/// Format a position the same way the on-screen coordinate readout does.
fn fmt_pos(p: DVec3) -> String {
    format!("X {:.1}  Y {:.1}  Z {:.1}", p.x, p.y, p.z)
}
