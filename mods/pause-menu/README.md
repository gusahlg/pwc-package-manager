# Pause menu (`pwc.pause-menu`)

What Esc opens in a world, once chat and every in-world panel have declined the key: Resume, the
entries other packages offer for the pause screen (such as Settings and Mods), and Leave World,
which saves and returns to the start screen.

The world keeps running underneath (in multiplayer and in single player): the menu only takes the
input. Without this package, Esc leaves the world at once, as it always did.

Mod id: `pause_menu`.

## Controls

| Input | Action |
|---|---|
| Up/Down | Select |
| Enter | Choose |
| Esc | Resume |

## Settings and persisted state

None.

## Dependencies

`pwc.ui-kit ^1.0` and the PWC mod API (`pwc-api ^3.0`).

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
