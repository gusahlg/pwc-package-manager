# Changelog — pwc.dev-toolkit

## 2.1.1 — modules, not mods

- The package says module everywhere: `module.toml`, `kind = "module"`, `pwc_module_api` and the `Module*` names, `pwc-api ^3.4`. Nothing else changes; the id is the same.

## 2.1.0

- `/bodies` lists every body by its name and class (start world, living world, cube moon, glowing
  world, binary cube, ...), nearest first, with its distance, size and `/tp`: the game grows its
  bodies from the seed since worldgen 11, so the old fixed kinds are gone.
- `/tp <name>` lands on a body by name (any case, a unique prefix is enough); `/tp <class> [n]` on
  the `n`th body of a class word. The old kind names (`home`, `twin`, `verdant`, `hollow`, `ember`,
  `moon`) still work for one release.
- The altitude note after a coordinate teleport names the body.
- Requires `pwc-api ^3.1`.

## 2.0.0

- The commands register through `pwc.commands` (a new dependency) and run from the chat:
  `/tp` (`/teleport`, `/setpos`), `/bodies`, `/noclip`, `/pos` (`/where`), `/inspect` (`/look`),
  `/reactions`, `/walkspeed`, `/flyspeed`, `/cruise` and `/gravity` (`/g`), with the same behaviour
  and output.
- `/gfx` (now `/set`), `/time`, `/mute`, `/deafen`, `/audio`, `/voicetest` and `/help` moved to
  `pwc.commands`.
- Flight on `F` is the toolkit's own immediate action (`toolkit.fly`): it works with mod logic off,
  and never flies the player behind a detached camera.
- Requires `pwc-api ^3.0`.
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
