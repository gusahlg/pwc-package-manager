# Developer Toolkit (`pwc.dev-toolkit`)

The console commands and the flight key. The base game keeps the console (chat, messages from the
game and from mods) but has no commands and no way to fly of its own: this package adds them.

Mod id: `dev_toolkit` (display name "Developer Toolkit"), in the Tools group, enabled by default.
It is not part of `pwc.essentials`; add it to an instance with `pwc mod add pwc.dev-toolkit`.

## Controls

| Input | Action |
|---|---|
| `F` | Toggle walking and flying |
| `T`, or `/` | Open the console (the core's); type a command and press Enter |
| `Tab` | Complete a command name (from every enabled mod) |

## Commands

A leading `/` is optional in singleplayer. In multiplayer a line without `/` is chat.

| Command | Does |
|---|---|
| `/tp <x> <y> <z>` | Teleport to coordinates (clamped to the world border). Also `/teleport`, `/setpos`. |
| `/tp <name> [n]` | Land 2,000 blocks above the `n`th body of a kind (`home`, `twin`, `verdant`, `hollow`, `ember`, `moon`; a unique prefix is enough), standing up along its pull. |
| `/bodies` | List every body of the world, nearest first, with its distance, size and `/tp`. |
| `/noclip` | Toggle flight through geometry. |
| `/cruise [km/s\|off]` | Space travel at any speed you type in km/s, up to ten times the speed of light (default 100,000 km/s), forward along your view, through everything, with the world held still. `/cruise off` (or `/cruise` while cruising) stops, clear of the ground. |
| `/walkspeed [n]`, `/flyspeed [n]` | Show or set the walking or flying speed in blocks per second (at most 100 km/s). |
| `/pos` | Show your coordinates. Also `/where`. |
| `/inspect [x y z]` | Describe a block (the one under your feet by default): elements, readings, descriptor. Also `/look`. |
| `/reactions` | Show the reaction scheduler's active contacts, turns and operations. |
| `/gravity` | Show the pull of the matter around you: strength, direction, tilt, potential. Also `/g`. |
| `/gfx [setting value]` | Show every graphics setting, or change one (`/gfx msaa 4`). Also `/graphics`. |
| `/time [set <when> \| length <secs>]` | Show or set the day/night clock (`0..1`, `0..24` or `dawn`, `noon`, `dusk`, `night`...), or the length of a day. |
| `/mute` | Toggle the master mute for this session. |
| `/deafen` | Toggle hearing incoming voice. |
| `/audio <master\|effects\|voice> <0-100>` | Set a volume. Also `/volume`. |
| `/voicetest` | Play a local voice test cue. |
| `/help` | List the commands of every enabled mod. Also `/?`. |

The game follows up on what a command changed: changed settings are applied and saved, the audio
mix follows, a `/time` change goes to the server, and a teleport streams its destination at once.

## Settings

None. The toolkit has no knobs on the mods screen.

## Persisted state

Nothing besides `dev_toolkit=on|off` in `mods.cfg`. Graphics and audio commands edit the game's own
settings, which the game saves.

## Dependencies

None besides the PWC mod API (`pwc-api ^1.1`).

## Compatibility

- In multiplayer the server sets the length of a day (`/time length` is undone locally) and may
  refuse a teleport (it snaps you back).
- `run_command` is first-enabled-wins: another enabled mod that registers earlier and handles a
  command of the same name takes it.
- Disabling the toolkit removes the commands and the flight key. Whether the player flies is the
  game's state (it is saved with the world), so a player who is flying keeps flying until the
  toolkit is enabled again.

## Credits

The commands moved here from the PWC game, where gusahlg and Restitutor wrote them.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
