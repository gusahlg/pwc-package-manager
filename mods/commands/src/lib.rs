//! Commands: `/` commands on top of the chat.
//!
//! - A sent chat line starting with `/` is a command: `/name args…`. In single player every line
//!   is one, so the leading `/` is optional there. An unknown name answers
//!   "unknown command 'x' - type '/help'". The command line is echoed (`> /tp 1 2 3`) before its
//!   output, except `/op`, which carries a secret.
//! - **`/`** opens the chat with a `/` already typed. Tab completes command names.
//! - Shipped commands: `/help`, `/set` (every core setting and package option, through the game's
//!   options registry; `/gfx` is an alias), `/time`, `/mute`, `/deafen`, `/audio`, `/voicetest`
//!   and `/op <secret>` (the server's operator login, sent as global chat and never echoed).
//! - Other mods add commands through the [`CommandsHandle`] this package provides (the Developer
//!   Toolkit adds `/tp`, `/cruise`, `/noclip` and the rest). `/help` lists them all, in
//!   registration order.
//!
//! The chat knows nothing of commands; this package installs one line handler and a completer
//! through `pwc.chat`'s handle.

use std::cell::RefCell;
use std::rc::Rc;

use pwc_chat::{common_prefix, ChatHandle, Completion, Handled};
use pwc_mod_api::engine::Key;
use pwc_mod_api::input::intent::Chord;
use pwc_mod_api::ui::{Line, Role};
use pwc_mod_api::{Action, Channel, FrameContext, GameContext, Mod, ModRegistrar};

mod options;

pub use options::UNAVAILABLE;

#[cfg(test)]
mod tests;

/// The id of the action that opens the chat with a `/` typed.
pub const OPEN: &str = "commands.open";
const OPEN_CHORDS: &[Chord] = &[Chord::key(Key::Slash)];
const ACTIONS: &[Action] = &[Action {
    id: OPEN,
    label: "Type a command (/)",
    default: OPEN_CHORDS,
    repeat: false,
    held: false,
    immediate: true,
}];

/// One command: what `/help` lists and Tab completes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    /// The name typed after the `/`.
    pub name: &'static str,
    /// Other names for the same command (not listed, not completed).
    pub aliases: &'static [&'static str],
    /// The arguments as `/help` shows them (`<x y z|name>`), or `""`.
    pub args: &'static str,
    /// What the command does, in a few words.
    pub help: &'static str,
}

impl Command {
    /// Whether `name` is this command's name or one of its aliases.
    pub fn answers(&self, name: &str) -> bool {
        self.name == name || self.aliases.contains(&name)
    }
}

/// A command's handler: the game state and the arguments, answering with scrollback lines.
pub type CommandFn = fn(&mut GameContext, &[&str]) -> Vec<Line>;

type Handler = Box<dyn FnMut(&mut GameContext, &[&str]) -> Vec<Line>>;

#[derive(Default)]
struct Registry {
    commands: Vec<(Command, Option<Handler>)>,
}

/// The command registry: add commands, list them, run a line. Provided at registration; get it
/// with `registrar.get::<pwc_commands::CommandsHandle>()` from a package that depends on
/// `pwc.commands`.
#[derive(Clone, Default)]
pub struct CommandsHandle {
    registry: Rc<RefCell<Registry>>,
}

impl CommandsHandle {
    /// An empty registry, for tests and tools. The package's own handle comes with the shipped
    /// commands ([`add_builtins`]).
    pub fn new() -> Self {
        Self::default()
    }

    /// Add `command`, run by `handler`. A name an earlier command already answers keeps going to
    /// that one (and `/help` lists it once).
    pub fn add(&self, command: Command, handler: impl FnMut(&mut GameContext, &[&str]) -> Vec<Line> + 'static) {
        self.registry.borrow_mut().commands.push((command, Some(Box::new(handler))));
    }

    /// Every command `/help` lists, in registration order, a shadowed name once.
    pub fn commands(&self) -> Vec<Command> {
        let registry = self.registry.borrow();
        let mut listed: Vec<Command> = Vec::new();
        for (command, _) in &registry.commands {
            if !listed.iter().any(|c| c.name == command.name) {
                listed.push(*command);
            }
        }
        listed
    }

    /// Run one command line (the leading `/` optional) and return its output; the chat handler
    /// adds the echo. `None` when the line is empty or names no command.
    pub fn run(&self, line: &str, game: &mut GameContext) -> Option<Vec<Line>> {
        let body = line.strip_prefix('/').unwrap_or(line);
        let mut parts = body.split_whitespace();
        let name = parts.next()?;
        let args: Vec<&str> = parts.collect();
        // Run outside the borrow, so a handler may use the handle (`/help` lists the commands).
        let (index, mut handler) = {
            let mut registry = self.registry.borrow_mut();
            let index = registry.commands.iter().position(|(c, _)| c.answers(name))?;
            (index, registry.commands[index].1.take()?)
        };
        let out = handler(game, &args);
        self.registry.borrow_mut().commands[index].1 = Some(handler);
        Some(out)
    }

    /// Tab completion over the command names (`/po` → `/pos `).
    pub fn complete(&self, line: &str) -> Completion {
        complete_command(line, &self.commands())
    }
}

/// The package entry point: installs the commands mod on the chat's handle, with the shipped
/// commands, and provides the [`CommandsHandle`].
pub fn register(registrar: &mut ModRegistrar) {
    let chat = registrar
        .get::<ChatHandle>()
        .expect("pwc.commands needs the chat handle from pwc.chat (a declared dependency registered first)");
    let commands = CommandsHandle::new();
    add_builtins(&commands);
    attach(&chat, &commands);
    registrar.provide(commands);
    registrar.add(Commands { chat });
}

/// Install the commands' line handler and Tab completer on `chat`.
pub fn attach(chat: &ChatHandle, commands: &CommandsHandle) {
    let runner = commands.clone();
    chat.add_handler(move |line, game| handle_line(&runner, line, game));
    let completer = commands.clone();
    chat.set_completer(move |line| completer.complete(line));
}

/// The chat line handler: a `/` line (any line in single player) is a command.
fn handle_line(commands: &CommandsHandle, line: &str, game: &mut GameContext) -> Handled {
    if !line.starts_with('/') && game.networked {
        return Handled::Pass;
    }
    let body = line.strip_prefix('/').unwrap_or(line);
    let name = body.split_whitespace().next();
    // `/op <secret>` is the server's operator login: sent as global chat, never echoed.
    if name == Some("op") {
        if game.networked {
            game.send_chat(Channel::Global, line);
            return Handled::Consumed(Vec::new());
        }
        return Handled::Consumed(rejected(vec!["/op: operators exist only on a server".to_string()]));
    }
    let mut out = vec![Line::of(Role::Accent, format!("> {line}"))];
    let Some(name) = name else {
        return Handled::Consumed(out);
    };
    match commands.run(line, game) {
        Some(lines) => out.extend(lines),
        None => out.push(Line::of(Role::Danger, format!("unknown command '{name}' - type '/help'"))),
    }
    Handled::Consumed(out)
}

/// The commands mod (id `commands`): the `/` key.
pub struct Commands {
    chat: ChatHandle,
}

impl Mod for Commands {
    fn name(&self) -> &str {
        "Commands"
    }

    fn id(&self) -> &'static str {
        "commands"
    }

    fn actions(&self) -> &[Action] {
        ACTIONS
    }

    fn on_frame(&mut self, ctx: &mut FrameContext) {
        if ctx.action(OPEN) {
            self.chat.open_with("/");
        }
    }
}

pub(crate) fn shown(lines: Vec<String>) -> Vec<Line> {
    lines.into_iter().map(|l| Line::of(Role::Dim, l)).collect()
}

pub(crate) fn rejected(lines: Vec<String>) -> Vec<Line> {
    lines.into_iter().map(|l| Line::of(Role::Danger, l)).collect()
}

/// The commands this package ships, in the order `/help` lists them.
pub const BUILTINS: &[(Command, CommandFn)] = &[
    (Command { name: "set", aliases: &["gfx", "graphics"], args: "[key [value]]", help: "show or change settings and options" }, |g, a| {
        options::set(a, g)
    }),
    (Command { name: "time", aliases: &[], args: "[set|length]", help: "show or set the day/night clock" }, |g, a| {
        options::time(a, g.sky)
    }),
    (Command { name: "mute", aliases: &[], args: "", help: "toggle master mute (this session)" }, |g, _| {
        options::mute(g.settings_mut())
    }),
    (Command { name: "deafen", aliases: &[], args: "", help: "toggle hearing incoming voice" }, |g, _| {
        options::deafen(g.settings_mut())
    }),
    (Command { name: "audio", aliases: &["volume"], args: "<chan> <0-100>", help: "set master/effects/voice volume" }, |g, a| {
        options::audio(a, g)
    }),
    (Command { name: "voicetest", aliases: &[], args: "", help: "play a local voice test cue" }, |g, _| options::voicetest(g)),
    (Command { name: "op", aliases: &[], args: "<secret>", help: "log in as a server operator" }, |_, _| {
        rejected(vec!["/op: operators exist only on a server".to_string()])
    }),
];

/// `/help`'s entry, after the other shipped commands.
const HELP: Command = Command { name: "help", aliases: &["?"], args: "", help: "show this list" };

/// Add the shipped commands to `commands`. `/help` lists whatever the registry holds when it runs.
pub fn add_builtins(commands: &CommandsHandle) {
    for (command, handler) in BUILTINS {
        commands.add(*command, *handler);
    }
    let listing = commands.clone();
    commands.add(HELP, move |_, _| help(&listing.commands()));
}

/// `/help` — every command, in registration order.
pub fn help(commands: &[Command]) -> Vec<Line> {
    let mut lines = vec!["commands (a leading '/' is optional):".to_string()];
    lines.extend(commands.iter().map(|c| {
        let usage = if c.args.is_empty() { c.name.to_string() } else { format!("{} {}", c.name, c.args) };
        format!("  /{usage:<20} {}", c.help)
    }));
    shown(lines)
}

fn complete_command(input: &str, commands: &[Command]) -> Completion {
    let body = input.strip_prefix('/').unwrap_or(input);
    if body.is_empty() || body.contains(char::is_whitespace) {
        return Completion::None;
    }
    let lead = if input.starts_with('/') { "/" } else { "" };
    let matches: Vec<&str> = commands.iter().map(|c| c.name).filter(|n| n.starts_with(body)).collect();
    match matches.as_slice() {
        [] => Completion::None,
        [only] => Completion::Full(format!("{lead}{only} ")),
        many => Completion::Ambiguous(format!("{lead}{}", common_prefix(many)), many.iter().map(|s| s.to_string()).collect()),
    }
}
