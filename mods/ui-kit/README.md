# UI kit (`pwc.ui-kit`)

A library for menu packages: the menu framework, the default look of every screen outside a
world, and the text widgets. It registers nothing and holds no state; a package that draws menus
depends on it, and on no other menu package.

## What it offers

- **Menus.** A `Menu` builds a `View` (a title, rows, a hint and an optional notice) from the
  core's `ScreenContext` and answers one `Msg` at a time with a core `ScreenOutcome`. `Framed`
  adds the cursor and makes the menu a core `Screen`. Rows are actions, values (toggle, choice,
  bar), text fields or headings.
- **The look.** `DefaultTheme` lays a view out and draws it as the core's `UiElement`s (text and
  rectangles; mods never touch the frame). The font is monospace, so `text_width` is the layout
  metric. `draw_waiting` is the page shown while the game connects or loads a world.
- **Text widgets.** `TextInput` (an editable line with history and Tab completion), `EditBuf`
  (the caret-safe buffer form fields use), `Ring` (a bounded scrollback), `Completion` and
  `common_prefix`.
- **Tests.** `testing::Fixture` builds a `ScreenContext` with empty defaults.

## Controls, settings and persisted state

None of its own.

## Dependencies

None besides the PWC mod API (`pwc-api ^2.2`, with the core screen host).

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
