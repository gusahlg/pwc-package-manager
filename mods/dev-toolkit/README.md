# Developer Toolkit (`pwc.dev-toolkit`)

Travel and inspection commands, and the flight key. The base game has no commands and no way to
fly of its own: the chat (`pwc.chat`) and its commands (`pwc.commands`) are mods, and this package
adds its commands to them.

Mod id: `dev_toolkit`. It runs whenever a build has it. It is not part of `pwc.essentials`; add it
to an instance with `pwc mod add pwc.dev-toolkit`.

## Controls

| Input | Action |
|---|---|
| `F` | Toggle walking and flying (works under every graphics preset; not while the free camera is detached) |
| `§`, or `/` | Open the chat (`pwc.chat`, `pwc.commands`); type a command and press Enter |
| `Tab` | Complete a command name (from every active mod) |

## Commands

A leading `/` is optional in singleplayer. In multiplayer a line without `/` is chat. `/help`,
`/set` (alias `/gfx`), `/time`, `/mute`, `/deafen`, `/audio`, `/voicetest` and `/op` come from `pwc.commands`.

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

The game follows up on what a command changed: a teleport streams its destination at once and is
reported to the server.

## Settings

None.

## Persisted state

Nothing.

## Dependencies

`pwc.commands` (`^1.0`, which brings `pwc.chat`) and the PWC mod API (`pwc-api ^3.0`).

## Compatibility

- In multiplayer the server sets the length of a day (`/time length` is undone locally) and may
  refuse a teleport (it snaps you back).
- A command name another package added to `pwc.commands` earlier keeps going to that package
  (the shipped commands come first).
- A server may refuse `pwc.dev-toolkit`; the game then suspends it for that session, and the
  commands and the flight key are gone until you leave. Whether the player flies is the game's
  state (it is saved with the world), so a player who is flying keeps flying.

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
