# Changelog — pwc.dev-toolkit

## 2.0.0 (unreleased)

- The commands register through `pwc.commands` (a new dependency) and run from the chat:
  `/tp` (`/teleport`, `/setpos`), `/bodies`, `/noclip`, `/pos` (`/where`), `/inspect` (`/look`),
  `/reactions`, `/walkspeed`, `/flyspeed`, `/cruise` and `/gravity` (`/g`), with the same behaviour
  and output.
- `/gfx`, `/time`, `/mute`, `/deafen`, `/audio`, `/voicetest` and `/help` moved to `pwc.commands`.
- Flight on `F` is the toolkit's own immediate action (`toolkit.fly`): it works with mod logic off,
  and never flies the player behind a detached camera.
- Requires `pwc-api ^2.2`.
- No Tools group: mods are no longer grouped, switched or tuned in the game; the build decides
  what is in. A server that refuses the toolkit suspends it for the session.


## 1.0.1

- Requires `pwc-api ^2.0`. Behaviour unchanged.

## 1.0.0 — first release, moved out of the PWC core

- Every console command the base game used to have, with the same behaviour and output: `/tp`
  (`/teleport`, `/setpos`), `/bodies`, `/noclip`, `/pos` (`/where`), `/inspect` (`/look`),
  `/reactions`, `/gfx` (`/graphics`), `/time`, `/walkspeed`, `/flyspeed`, `/cruise`, `/mute`,
  `/deafen`, `/audio` (`/volume`), `/voicetest`, `/gravity` (`/g`) and `/help` (`/?`). `/help`
  now lists the commands of every enabled mod. The `/name` crafting stub is gone.
- Flight on `F` (walking and flying), which the core no longer has.
- Mod id `dev_toolkit`, in the new Tools group. Requires `pwc-api ^1.1` (`Mod::run_command`,
  `Mod::commands`, `Mod::on_toggle_fly`).
- Licensed `Apache-2.0 OR MIT` at your option; both texts are in `LICENSES/`.
