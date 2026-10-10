# Mod menu (`pwc.mod-menu`)

The packages compiled into this build, read-only. It offers "Mods" on the main menu and on the pause
screen.

Packages are grouped under the bundles that include them (`pwc.essentials`, …); what no bundle
includes is listed under "Other". Each row shows the package's name, version and description, and
"(off on this server)" while a server has suspended it for the session. The highlighted package's
dependencies and bundles are spelled out at the bottom. Nothing here switches anything: mods are
compiled in, and `pwc mod add` / `pwc mod remove` and `pwc build` change them.

The list is the one the builder generated for this build; the menu names no package.

## Controls

| Input | Action |
|---|---|
| Up/Down | Browse |
| Esc | Back |

## Settings and persisted state

None.

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
