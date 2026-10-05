# Changelog — pwc.dev-toolkit

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
