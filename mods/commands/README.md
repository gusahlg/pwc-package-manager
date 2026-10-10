# Commands (`pwc.commands`)

Slash commands in the chat, and the registry other mods add their commands to. Built on
`pwc.chat`: it installs one line handler and a Tab completer through the chat's handle, and the
chat knows nothing of commands.

Mod id: `commands`. Part of
the `pwc.chat-commands` bundle, which `pwc.essentials` includes.

## Controls

| Input | Action |
|---|---|
| `/` | Open the chat with `/` typed |
| `Tab` | Complete a command name |

A line that starts with `/` is a command; in single player every line is, so the `/` is optional
there. The command is echoed (`> /time set noon`) before its output. An unknown name answers
"unknown command 'x' - type '/help'".

## Commands

| Command | Does |
|---|---|
| `/help` | List every command, from every mod. Also `/?`. |
| `/gfx [setting value]` | Show every graphics setting, or change one (`/gfx msaa 4`). A lane whose visual group no installed package provides is marked "(unavailable in this build)". Also `/graphics`. |
| `/time [set <when> \| length <secs>]` | Show or set the day/night clock (`0..1`, `0..24` or `dawn`, `noon`, `dusk`, `night`...), or the length of a day. |
| `/mute` | Toggle the master mute for this session. |
| `/deafen` | Toggle hearing incoming voice. |
| `/audio <master\|effects\|voice> <0-100>` | Set a volume. Also `/volume`. |
| `/voicetest` | Play a local voice test cue. |
| `/op <secret>` | Log in as the server's operator. Sent to the server as global chat and never echoed. |

The game follows up on what a command changed: changed settings are applied and saved, the audio
mix follows, a `/time` change goes to the server, and a teleport streams its destination at once.
On a server the length of a day is the server's (`/time length` is undone locally).

## For mod authors

`register` provides a `pwc_commands::CommandsHandle`. A package that depends on `pwc.commands`
gets it with `registrar.get::<pwc_commands::CommandsHandle>()` and adds commands with
`add(Command { name, aliases, args, help }, handler)`, where the handler gets the game state and
the arguments and answers with scrollback lines. A name an earlier command already has keeps going
to that one. `/help` and Tab list every added command in registration order.

## Settings and persisted state

None. The graphics and audio commands edit the game's own settings, which the game saves.

## Dependencies

`pwc.chat` (`^1.0`) and the PWC mod API (`pwc-api ^2.2`).

## Credits

The commands moved here from the Developer Toolkit, and before that from the PWC game, where
gusahlg and Restitutor wrote them.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
