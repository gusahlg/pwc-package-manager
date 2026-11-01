# Changelog — pwc.commands

## 1.0.1 — modules, not mods

- The package says module everywhere: `module.toml`, `kind = "module"`, `pwc_module_api` and the `Module*` names, `pwc-api ^3.4`. Nothing else changes; the id is the same.

## 1.0.0 — first release

- Slash commands on top of `pwc.chat`: a `/` line is a command (any line in single player), echoed
  before its output; an unknown name points at `/help`. `/` opens the chat with a slash typed; Tab
  completes command names.
- Ships `/help`, `/set` (every core setting and package option through the game's options
  registry; `/gfx` and `/graphics` are aliases), `/time`, `/mute`, `/deafen`, `/audio`, `/voicetest`
  (moved from the Developer Toolkit with their behaviour and output) and `/op <secret>`, the
  server's operator login, sent unechoed as before.
- `CommandsHandle` lets other mods add commands.
- Mod id `commands`. Requires `pwc-api ^3.0` and `pwc.chat ^1.0`.
- `/set` marks a lane no installed package provides "(unavailable in this build)"; it names no
  visual mod.
- Licensed `Apache-2.0 OR MIT` at your option; both texts are in `LICENSES/`.
