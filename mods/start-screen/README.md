# Start screen (`pwc.start-screen`)

The default start screen of PWC: the root screen the game shows out of a world.

- **Main menu** — New World, Worlds, Host Server, Join Server, then the main-menu entries other
  packages offer (with the essentials: Mods and Settings), then Quit. The Worlds row shows how many
  worlds are saved; a notice from the game (for example why a join failed) is shown under the menu.
- **Worlds page** — one row per saved world, newest first, with its play time and edit count.
  A damaged save stays listed (marked "damaged") because a backup may still load.
- **Host Server / Join Server forms** — address (join only), port, password (optional when
  hosting) and your name, prefilled from the last session (port 5555 and the name "player" by
  default). An invalid port keeps the form open with an error; an empty port means the default.
- **Waiting page** — "Connecting…" or "Loading…" and "Cancel" while the game joins a server or
  builds a world; Esc cancels.

The start screen only builds pages and asks the game for what the player chose; the game performs
it (creating, loading, hosting, joining). Other screens (settings, the mod list) come from the
packages that registered them: the start screen lists their labels and opens them by id, without
knowing them.

Mod id: `start`. Part of `pwc.essentials`.

## Controls

| Input | Action |
|---|---|
| `↑` / `↓` | Select a row |
| `Enter` | Choose / load / submit the form |
| `Esc` | Back to the main menu |
| Typing | Edit the selected text field (forms) |

## Settings

None.

## Persisted state

- **World save:** nothing.
- The remembered address, port and name belong to the game's session file, not to this mod.

## Dependencies

`pwc.ui-kit ^1.0` (the menu framework and look) and the PWC mod API (`pwc-api ^2.0`, with the
core screen host).

## Compatibility

- `root_screen` is first-active-wins: a root-screen package registered earlier replaces this one.
- In a build without a root screen, the game enters the most recent world (or a new one) and Esc
  saves and quits.
- The look is the UI kit's default look.
- The mod id is unchanged from the start screen that shipped inside PWC 2.0.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
