# Settings menu (`pwc.settings-menu`)

The settings hub and its pages. It offers "Settings" on the main menu and on the pause screen (a
screen entry, so a start screen or a pause menu lists it without knowing this package).

The hub has one row per category: Performance, Video, World, Interface, Audio. Each page lists every
tunable on that page from the game's options registry: the core's settings first, then every
package's options (InfiniteDiffusion's terrain, neural textures, material names, proximity chat's
Voice Chat, …). Left/Right (or h/l, or Enter) steps a row; changes apply at once and are saved
shortly after the last step. A row that applies to new worlds says so. A graphics lane whose visual
group no installed package provides is marked "(unavailable in this build)".

## Controls

| Input | Action |
|---|---|
| Up/Down | Select a row |
| Left/Right, h/l, Enter | Change the value |
| Esc | Back |

## Settings and persisted state

None of its own. The values live in the game's `settings.cfg`.

## Dependencies

`pwc.ui-kit ^1.0` and the PWC mod API (`pwc-api ^2.2`, with the core screen host).

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
