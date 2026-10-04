# Essentials (`pwc.essentials`)

A bundle: no code of its own, just the first-party packages that make up PWC as it ships. Adding
`pwc.essentials` to an instance adds all of them:

| Package | Mods (ids) | What it does |
|---|---|---|
| `pwc.menus` | `menus` | The standard menu theme |
| `pwc.start-screen` | `start` | Main menu, saved-worlds page, host and join forms |
| `pwc.hotbar` | `hotbar` | Nine slots of held materials plus the bare hand |
| `pwc.inventory` | `inventory` | The held-materials list (press I); equips into the hotbar |
| `pwc.game-ui` | `game_ui` | In-world HUD pieces: which way you look along X, Y and Z |
| `pwc.visuals` | `atmosphere`, `post`, `lighting` | Sky, post-processing and lighting on top of the sunlight-only core |
| `pwc.neural-textures` | `neural_textures` | A unique generated texture per material |
| `pwc.material-names` | `material_names` | Names for every material and tool |
| `pwc.infinite-diffusion` | `diffusion` | Mountains, caves, mines and planets (world generation) |

Every mod is in the Essentials group on the mods screen, where the whole group can be switched on
or off at once (each member's choice is saved as its own `mods.cfg` line).

## Controls, settings and persisted state

See each package's README. The bundle adds none.

## Dependencies

The nine packages above, each `^1.0`. The resolver picks one version of each; `pwc.hotbar` always
registers before `pwc.inventory`, which depends on it.

## Compatibility

Without the bundle (or with every Essential switched off) PWC still runs: the core keeps the
world, the law, the player's held materials, the Settings and Mods screens, a plain start menu, a
built-in menu theme, flat block colours and a flat world.

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
