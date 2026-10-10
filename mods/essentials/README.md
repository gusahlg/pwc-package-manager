# Essentials (`pwc.essentials`)

A bundle: no code of its own, just the first-party packages that make up PWC as it ships. Adding
`pwc.essentials` to an instance adds all of them:

| Package | Mods (ids) | What it does |
|---|---|---|
| `pwc.start-screen` | `start` | Main menu, saved-worlds page, host and join forms, the loading page |
| `pwc.settings-menu` | — | The settings hub and pages (core settings and every package's options) |
| `pwc.mod-menu` | — | The packages of the build, read-only |
| `pwc.pause-menu` | `pause_menu` | What Esc opens in a world: Resume, Settings, Mods, Leave World |
| `pwc.hotbar` | `hotbar` | Nine slots of held materials plus the bare hand |
| `pwc.inventory` | `inventory` | The held-materials list (press I); equips into the hotbar |
| `pwc.game-ui` | `game_ui` | The in-world HUD: reticle, coordinates, frame rate, player count, loading lines, facing indicator |
| `pwc.chat-commands` | `chat`, `commands` | The text chat on § and its slash commands (a bundle of `pwc.chat` and `pwc.commands`) |
| `pwc.visuals` | `atmosphere`, `post`, `lighting` | Sky, post-processing and lighting on top of the sunlight-only core |
| `pwc.neural-textures` | `neural_textures` | A unique generated texture per material |
| `pwc.material-names` | `material_names` | Names for every material and tool |
| `pwc.infinite-diffusion` | `diffusion` | Mountains, caves, mines and planets (world generation) |
| `pwc.sounds` | `sounds` | Footsteps, blocks, tools, swings and menu clicks |
| `pwc.proximity-chat` | `proximity_chat` | Push-to-talk voice for people you can see |

The menus draw with `pwc.ui-kit`, a library the four menu packages depend on; it comes with them.
Mods are compiled in: the bundle is how these packages are installed together, and nothing in the
game switches them.

## Controls, settings and persisted state

See each package's README. The bundle adds none.

## Dependencies

`pwc.start-screen ^2.0`, the three new menus and `pwc.chat-commands` at `^1.0`, `pwc.hotbar` and
`pwc.inventory` at `^2.1`, `pwc.game-ui ^2.0`, and the other six packages at `^1.1`. The resolver
picks one version of each; `pwc.hotbar` always registers before `pwc.inventory`, which depends on it.

## Compatibility

Without the bundle PWC still runs: the core keeps the world, the law, the player's held materials,
flat block colours and a flat world, and with no start screen it enters the most recent world (or
a new one) and Esc saves and quits. Settings then live only in `settings.cfg`. Without
`pwc.pause-menu`, Esc leaves the world at once. Without `pwc.sounds` it plays no cues. Without
`pwc.chat-commands` there is no chat line and no commands (chat from other players still arrives,
unshown), and without `pwc.game-ui` no reticle or readouts. Without `pwc.proximity-chat` the
microphone stays closed and voice is neither sent nor played.

## Licence

This bundle (its manifest, README, changelog and icon) is licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

Each bundled package carries its own licence; every first-party package is licensed the same way,
`Apache-2.0 OR MIT`.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
