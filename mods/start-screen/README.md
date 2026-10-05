# Start screen (`pwc.start-screen`)

The default start screen of PWC:

- **Main menu** — New World, Worlds, Host Server, Join Server, Mods, Settings, Quit. The Worlds row
  shows how many worlds are saved; a notice from the game (for example why a join failed) is shown
  under the menu.
- **Worlds page** — one row per saved world, newest first, with its play time and edit count.
  A damaged save stays listed (marked "damaged") because a backup may still load.
- **Host Server / Join Server forms** — address (join only), port, password (optional when
  hosting) and your name, prefilled from the last session (port 5555 and the name "player" by
  default). An invalid port keeps the form open with an error; an empty port means the default.

The start screen only builds pages and reports what the player chose; the game performs it
(creating, loading, hosting, joining) and owns the Settings and Mods screens, so those exist even
with every mod off.

Mod id: `start` (display name "Start"), in the Essentials group, enabled by default.

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
- **`mods.cfg`:** only `start=on|off`.
- The remembered address, port and name belong to the game's session file, not to this mod.

## Dependencies

None besides the PWC mod API (`pwc-api ^2.0`).

## Compatibility

- `start_screen` is first-enabled-wins: a start-screen package registered earlier replaces this
  one while both are enabled.
- Disabled, the game uses its plain fallback list (New world / Load most recent / Settings / Mods /
  Quit), so the game is always startable.
- The look comes from the active menu theme (`pwc.menus` or the game's built-in theme).
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
