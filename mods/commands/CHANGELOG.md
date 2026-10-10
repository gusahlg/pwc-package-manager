# Changelog — pwc.commands

## 1.0.0 — first release

- Slash commands on top of `pwc.chat`: a `/` line is a command (any line in single player), echoed
  before its output; an unknown name points at `/help`. `/` opens the chat with a slash typed; Tab
  completes command names.
- Ships `/help`, `/gfx`, `/time`, `/mute`, `/deafen`, `/audio`, `/voicetest` (moved from the
  Developer Toolkit with their behaviour and output) and `/op <secret>`, the server's operator
  login, sent unechoed as before.
- `CommandsHandle` lets other mods add commands.
- Mod id `commands`, in the Essentials group. Requires `pwc-api ^2.2` and `pwc.chat ^1.0`.
- Licensed `Apache-2.0 OR MIT` at your option; both texts are in `LICENSES/`.
