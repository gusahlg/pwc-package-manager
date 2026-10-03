# Menus (`pwc.menus`)

Selects PWC's standard menu theme: the look of the title screen and of every other screen outside
a world (settings, the mods list, saved worlds, the server forms).

Themes are pure presentation: they place and paint the rows a screen hands them and never touch
input or menu state, so changing theme cannot affect how the menus work. PWC's built-in fallback
is this very theme, which means switching the package off looks identical. To restyle the menus,
install a package that offers another theme and either disable this one or have the other package
registered ahead of it.

Mod id: `menus` (display name "Menus"), in the Essentials group, enabled by default.

## Controls

None of its own. Menu navigation (Up/Down, Enter, Esc, Left/Right on values) is the game's.

## Settings

None. The menu scale is a game setting (on the Settings screen) that every theme applies.

## Persisted state

- **World save:** nothing.
- **`mods.cfg`:** only `menus=on|off`.

## Dependencies

None besides the PWC mod API (`pwc-api ^1.0`).

## Compatibility

- `menu_theme` is first-enabled-wins: the first enabled mod (in registration order) that offers a
  theme draws every menu.
- Disabling this mod changes nothing visible: the built-in fallback is the same theme.
- The mod id is unchanged from the menu mod that shipped inside PWC 2.0.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
