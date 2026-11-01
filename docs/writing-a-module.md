# Writing a module

This tutorial builds a small module, an altimeter that shows the player's height on the HUD, and
takes it from an empty directory to a `.pwcmodule` package. It assumes `pwc` is installed and set up
with an active instance ([README.md, Quick start](../README.md#quick-start)).

A PWC module is ordinary Rust compiled into the game. There is no `Cargo.toml` in a package: you
write a `module.toml`, and the PWC builder generates the Cargo manifests from it. The rules every
package must follow are in [POLICY.md](../POLICY.md); the manifest is specified in
[spec/module-manifest.md](spec/module-manifest.md).

The code and manifest examples in this tutorial may be copied into your own module under the terms of
`Apache-2.0 OR MIT`, whatever licence you choose for the rest of your module (each example is marked as
such for [REUSE](https://reuse.software)); the surrounding text is part of this repository's
documentation.

## 1. Create the package directory

```text
altimeter/
├── module.toml
├── README.md
├── LICENSES/
│   ├── Apache-2.0.txt
│   └── MIT.txt
├── assets/          optional: files embedded with include_bytes!/include_str!
└── src/
    └── lib.rs
```

Only `module.toml`, the README, the licence files, `CHANGELOG.md`, `NOTICE`, `src/`, `assets/` and
`data/` become part of the package. Anything else in the directory (`.git/`, `target/`, `dist/`,
editor files, a development `Cargo.toml`) is ignored when packaging. Inside the included paths,
hidden files, symlinks, `build.rs` and `Cargo.toml` are errors.

## 2. Write `module.toml`

<!-- SPDX-SnippetBegin -->
<!-- SPDX-SnippetCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->
```toml
format = 1

[package]
id = "alice.altimeter"
name = "Altimeter"
version = "0.1.0"
description = "Shows your height in the top-right corner of the screen."
authors = ["Alice Example <alice@example.org>"]
license = "Apache-2.0 OR MIT"
license-files = ["LICENSES/Apache-2.0.txt", "LICENSES/MIT.txt"]
pwc-api = "^3.0"
categories = ["interface"]
```
<!-- SPDX-SnippetEnd -->

- **`id`** is permanent: `<namespace>.<name>`, lowercase. Use your own namespace (a user or
  organisation name); `pwc` is reserved for first-party packages. The id also gives the Rust crate
  name, with `.` and `-` replaced by `_`: this package is the crate `alice_altimeter`.
- **`pwc-api`** is the version requirement on the `pwc-module-api` crate your code compiles against.
  This tutorial is written for API 3.0.
- **`name`** and **`description`** are what menus show for your package (the module list reads them
  from the build); the module itself has no display text.

### Licensing

PWC accepts only free software, and proprietary modules are not allowed
([POLICY.md, section 1](../POLICY.md#1-licensing)). The recommended licence is the one in the
example, **`Apache-2.0 OR MIT`**: users may take the code under either licence, it is the usual
choice for Rust code, and the first-party packages use it.

You may choose another licence from the policy's allowlist instead:

- another permissive licence, alone: `MIT`, `Apache-2.0`, `BSD-3-Clause`, `ISC`, `Zlib`, ...;
- a copyleft licence, so that modified versions of your module stay free: `MPL-2.0` (per file),
  `LGPL-3.0-or-later`, `GPL-3.0-or-later` or `AGPL-3.0-or-later` (the game's own licence, which
  also covers users of a modified version over a network);
- a content licence for assets or documentation, joined with `AND`, for example
  `(Apache-2.0 OR MIT) AND CC-BY-4.0` or `AGPL-3.0-or-later AND CC-BY-SA-4.0` (for assets compiled
  into the game, see [section 10](#10-assets)).

The rules `pwc` checks: the expression is SPDX (ids are case-sensitive; `AND`, `OR`, `WITH` and
parentheses); every id is on the allowlist, whose software licences are all compatible with the
game's `AGPL-3.0-or-later`, because your module is compiled into the AGPL game (its content licences
are for assets and documentation); and the code is under a software licence whichever `OR`
alternative a user picks, so a content licence alone, or `MIT OR CC-BY-4.0`, is rejected for a
`module` or `library`. Unlisted licences, `LicenseRef-*`, `NOASSERTION`, `NONE` and a trailing `+` are
rejected (write `GPL-3.0-or-later`, not `GPL-3.0+`).

<!-- REUSE-IgnoreStart -->
Include the full text of every licence you name and list the files in `license-files`. The
[REUSE](https://reuse.software) tool fetches them: `reuse download Apache-2.0 MIT` writes both into
`LICENSES/`; then replace the placeholder copyright line in `LICENSES/MIT.txt` with your own, such
as `Copyright (c) 2026 Alice Example`. The first-party packages in this repository's
[modules/](../modules/) directory show the finished layout.

It also helps to mark each file's licence, for example with a
`// SPDX-License-Identifier: Apache-2.0 OR MIT` line at the top of every source file.
<!-- REUSE-IgnoreEnd -->

Whatever licence you choose, a build of PWC that contains your module is distributed under the game's
AGPL as a whole, and its recipients are entitled to the source of everything in it. Published
packages already are that source.

## 3. Write the README

`README.md` is required. Say what the module does, how to use it (keys, settings), what it depends on,
what it persists, and anything a player must know before installing it, such as file or network
access ([POLICY.md, section 3](../POLICY.md#3-package-and-execution-model)).

## 4. Write the code

`src/lib.rs` defines the entry point, `register`. The builder calls it once at startup, after
every dependency has registered. It installs one or more values implementing `Module`.

<!-- SPDX-SnippetBegin -->
<!-- SPDX-SnippetCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->
<!-- REUSE-IgnoreStart -->
```rust
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Altimeter: shows the player's height in the top-right corner.

use pwc_module_api::prelude::*;

/// Entry point, called once by the generated module bundle.
pub fn register(registrar: &mut ModuleRegistrar) {
    registrar.add(Altimeter);
}

struct Altimeter;

impl Module for Altimeter {
    fn id(&self) -> &'static str {
        "alice.altimeter"
    }

    fn hud(&self, facts: &HudFacts, _world: &World, player: &Player, out: &mut Vec<HudElement>) {
        // The game asks in every HUD mode; show nothing with the HUD off.
        if !facts.hud_mode.shows_mod_hud() {
            return;
        }
        out.push(HudElement::Label {
            at: Anchor::TopRight,
            off: (-12, 40),
            base_fs: 14,
            role: Role::Muted,
            text: format!("{:.0} m", player.position.y).into(),
        });
    }
}
```
<!-- REUSE-IgnoreEnd -->
<!-- SPDX-SnippetEnd -->

`pwc_module_api::prelude` re-exports what most modules need: `Module`, `ModuleRegistrar`, `ModuleContext`,
`FrameContext`, `GameContext`, `HudFacts`, `Player`, `World`, `BlockId`, `BlockRegistry`, the HUD
types (`HudElement`, `Anchor`, `Role`, `Line`, `Panel`, `Row`) and the options types (`OptionSpec`,
`OptionId`, `Options`, `Category`). Everything else lives in modules such as `pwc_module_api::ui`,
`pwc_module_api::world`, `pwc_module_api::screen` and `pwc_module_api::block`.

`id` is the only required method. It is the stable key the game uses for the module's per-world
state, so it must never change; use your package id, or the package id plus a suffix when a package
installs several modules. `name` defaults to the id.

There is no switch for a module in the game: a module runs whenever the build has its package. A server
may refuse a package; the game then *suspends* it for that session, and its hooks do not run.

Every other method is an optional hook with a no-op default. The most common ones:

| Hook | Called | Use it to |
|---|---|---|
| `update(&mut self, ctx)` | Every module tick | React to input and change state through `ModuleContext`. |
| `on_frame(&mut self, ctx)` | Every in-world frame | Read immediate actions, take the keyboard (`ctx.capture_text`), change game state through `ctx.game`. Runs with module logic off too. |
| `hud(&self, facts, world, player, out)` | Every frame, every HUD mode | Push `HudElement`s to draw; `facts.hud_mode` says whether the HUD is shown. |
| `on_message(&mut self, msg) -> bool` | A chat line, join, leave or notice | Show it; return `true` when you did. |
| `on_options(&mut self, options)` | Options loaded or changed | Copy the values of your options (section 7). |
| `save_state`, `load_state` | World save and load | Persist per-world state, with a payload version. |
| `reset` | Entering a world | Clear per-world state. |
| `tool`, `on_block_break`, `on_tool_changed` | Gameplay events | Take part in holding and using matter. |
| `reaction_model` (API 3.2) | World/server selection | Supply a native authoritative `sim::model::ModelSpec`; the model owns its budgeted law, fields and persistence. |
| `appearance`, `namer`, `worldgen` | When the game needs one | Replace a core default (first active module wins). |
| `root_screen`, `pause_screen` | Out of a world; Esc in a world | Be the start screen or the pause screen (first active module wins). |

To offer a screen on the main menu or the pause screen without being either, register an entry:
`registrar.screen_entry(ScreenEntry { id, label, places, order, open })`. The menu framework and
default look are in the `pwc.ui-kit` library package; `pwc.settings-menu` and `pwc.pause-menu` show
how to use it. Commands come from `pwc.commands`: depend on it and add yours through its
`CommandsHandle` (see its README).

This list is not complete; see the `pwc-module-api` documentation for every hook, its arguments and
how the game arbitrates when several modules implement it (fan-out in install order, or first active
module wins). To read it, run `cargo doc -p pwc-module-api --open` in the game checkout.

Two rules matter from the first line of code:

- **Hooks are not hot paths.** `hud` and `update` run every frame or tick: keep them cheap and
  cache anything expensive. Never do per-voxel work in a hook.
- **Anything that affects the world must be deterministic** (no wall-clock time, unseeded
  randomness or hash-map iteration order), and names and looks must never flow back into the law,
  world generation, saves or the network. A HUD label is presentation, so the altimeter is free
  to format floats. See [POLICY.md, section 4](../POLICY.md#4-playing-well-with-pwc-the-module-contract).

## 5. Check the package

```sh
pwc package check ./altimeter
```

This validates the manifest, the licence policy and the package contents without compiling
anything. Fix everything it reports; `pwc module add` refuses packages that fail these checks.

## 6. Run it

Add the source directory to your instance as a local `path` entry and start the game:

```sh
pwc module add ./altimeter
pwc run
```

`pwc module add` records the directory's absolute path in `instance.toml`
(`"alice.altimeter" = { path = "/home/alice/src/altimeter" }`). A path entry is mutable: when you
edit the source, the next `pwc run` re-locks with the new hash and rebuilds. Only your crate, its
dependents and the generated crates recompile; the game is reused from the shared target
directory.

The compiler builds the snapshot of your sources taken at lock time, so its messages name files
under `~/.local/share/pwc/store/<hash>/`; the paths below that directory mirror your source tree.
For editor support you may keep a development `Cargo.toml` next to `module.toml` (for example one
that depends on `pwc-module-api` by path in your game checkout); packaging ignores it.

## 7. Options

A setting the player may tune is an *option* in the game's options registry. Declare it in
`register`; the game stores the value, saves it in `settings.cfg` as `<package-id>.<key>=`, and
every settings screen (such as `pwc.settings-menu`) and the `/set` command list it on its page.
Neither knows your package, and your package depends on neither. This version of the altimeter
lets the player choose how many decimals to show:

<!-- SPDX-SnippetBegin -->
<!-- SPDX-SnippetCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->
```rust
pub fn register(registrar: &mut ModuleRegistrar) {
    let decimals = registrar.option(OptionSpec::choice("decimals", "Altimeter Decimals", Category::Interface, &["0", "1", "2"], 0));
    registrar.add(Altimeter { decimals_id: decimals, decimals: 0 });
}

struct Altimeter {
    decimals_id: OptionId,
    decimals: usize,
}

impl Module for Altimeter {
    // id and hud as before; hud formats with
    // format!("{:.*} m", self.decimals, player.position.y)

    fn on_options(&mut self, options: &Options) {
        self.decimals = options.choice(self.decimals_id);
    }
}
```
<!-- SPDX-SnippetEnd -->

- Kinds: `OptionSpec::toggle`, `choice`, `percent` (whole percents with a range and a step) and
  `float`. `.next_world()` marks a value that applies to new worlds only.
- The game calls `on_options` after registration, once `settings.cfg` is read, and whenever a
  value changes. Reads are by the `OptionId` you got, so they cost nothing; copy what you need.
- Values are clamped to their range; a hand-edited or unparseable line keeps the default.
  `.legacy_key(k)` reads a value an older version saved under another key, once.
- Changing an option's key or kind incompatibly requires a major version bump.

Per-world state (`save_state` / `load_state`) is untrusted input too: an old or hand-edited save
must not crash the game.

## Testing

Unit tests drive your package the way a build registers it, through
`pwc_module_api::testing::Harness`:

<!-- SPDX-SnippetBegin -->
<!-- SPDX-SnippetCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->
```rust
#[cfg(test)]
module tests {
    use super::*;
    use pwc_module_api::settings::OptionValue;
    use pwc_module_api::testing::Harness;
    use pwc_module_api::{GameBuild, ModuleDescriptor};

    #[test]
    fn the_decimals_option_reaches_the_mod() {
        let mut harness = Harness::new(GameBuild::new().with_module(ModuleDescriptor {
            id: "alice.altimeter",
            name: "Altimeter",
            version: "0.1.0",
            register,
        }));
        assert_eq!(harness.id(0), "alice.altimeter");
        assert!(harness.set_option("alice.altimeter.decimals", OptionValue::Choice(2)));
    }
}
```
<!-- SPDX-SnippetEnd -->

The harness offers the hooks as the game arbitrates them (`update`, `frame`, `message`, `hud`,
`tool`, `appearance`, the screens), the options, and `suspend` / `resume`. Run the tests with
`cargo test` in a generated build workspace (`pwc build --print-dir` prints it).

## 8. Depend on another module

A module can use another package's Rust API. Declare the dependency in `module.toml`:

<!-- SPDX-SnippetBegin -->
<!-- SPDX-SnippetCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->
```toml
[dependencies]
"pwc.hotbar" = "^2.1"
```
<!-- SPDX-SnippetEnd -->

The resolver then puts exactly one compatible version of `pwc.hotbar` into every build that
contains your module, registers it before yours, and makes it available to your code as the crate
`pwc_hotbar`. Use the types `pwc.hotbar` exports with `use pwc_hotbar::…;`; its
[README](../modules/hotbar/README.md) and crate documentation describe them.

- Format 1 has no optional dependencies: whatever you list is always part of the build.
- Only dependencies you declare are visible to your code, and only through their public Rust API.
- A major version bump of a dependency signals an incompatible API change, so `^2.1` keeps you on
  compatible versions.
- Depending on a bundle such as `pwc.essentials` pulls in its packages but provides no crate; to
  use a package's API, depend on that package directly.

## 9. Share handles between packages

Packages often share live state. Registration has a small type-keyed store for this: a package
calls `registrar.provide(value)` in `register`, and every package registered after it can call
`registrar.get::<T>()` to receive a clone. Handles are usually `Rc` wrappers, so the clone refers
to the same state.

The altimeter can share the player's height with other packages:

<!-- SPDX-SnippetBegin -->
<!-- SPDX-SnippetCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->
```rust
use std::cell::Cell;
use std::rc::Rc;

use pwc_module_api::prelude::*;

/// The player's current height, shared with packages that depend on alice.altimeter.
#[derive(Clone)]
pub struct AltitudeHandle(Rc<Cell<f64>>);

impl AltitudeHandle {
    /// The height at the last module tick.
    pub fn get(&self) -> f64 {
        self.0.get()
    }
}

pub fn register(registrar: &mut ModuleRegistrar) {
    let altitude = AltitudeHandle(Rc::new(Cell::new(0.0)));
    registrar.provide(altitude.clone());
    registrar.add(Altimeter { altitude });
}

// In `impl Module for Altimeter`:
//     fn update(&mut self, ctx: &mut ModuleContext) {
//         self.altitude.0.set(ctx.player.position.y);
//     }
```
<!-- SPDX-SnippetEnd -->

A package that declares `"alice.altimeter" = "^0.1"` in its `[dependencies]` reads it back:

<!-- SPDX-SnippetBegin -->
<!-- SPDX-SnippetCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->
```rust
use alice_altimeter::AltitudeHandle;
use pwc_module_api::prelude::*;

pub fn register(registrar: &mut ModuleRegistrar) {
    // alice.altimeter registered before this package and provided the handle.
    let altitude = registrar
        .get::<AltitudeHandle>()
        .expect("alice.altimeter provides an AltitudeHandle");
    registrar.add(FlightLog { altitude });
}
```
<!-- SPDX-SnippetEnd -->

- Values are keyed by type. Always provide a type defined in your own crate, never a shared type
  such as `Rc<Cell<f64>>`, which another package could overwrite.
- The registrar's store is dropped once every package has registered. Keep the clones you need in
  your `Module` values.
- A provided type is part of your public API: changing it incompatibly requires a major version
  bump.

## 10. Assets

Put files under `assets/` (or `data/`) and embed them at compile time. Paths are relative to the
source file:

<!-- SPDX-SnippetBegin -->
<!-- SPDX-SnippetCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->
```rust
const COMPASS_ROSE: &[u8] = include_bytes!("../assets/compass-rose.png");
const NAMES: &str = include_str!("../data/names.txt");
```
<!-- SPDX-SnippetEnd -->

Do not read package files at run time; the package is compiled in. Assets need complete
provenance: keep their editable originals in the package, and give assets you did not create their
own licence and attribution (`NOTICE` or `assets/ATTRIBUTION.md`). The optional `icon` key in
`module.toml` names an `.svg` or `.png` under `assets/` for the package.

An embedded asset becomes part of the AGPL-licensed game executable, so give it a licence that
permits combination with AGPL software: your package's software licence, `CC0-1.0`, `CC-BY-4.0` or
`CC-BY-SA-4.0`. Fonts under `OFL-1.1` may be bundled too. Keep assets under `LAL-1.3` as separate
data files rather than compiling them in
([POLICY.md, section 1](../POLICY.md#why-only-agpl-compatible-licences)).

## 11. Build the package

When the module is ready to share:

```sh
pwc package build ./altimeter
```

This writes `altimeter/dist/alice.altimeter-0.1.0.pwcmodule` and prints its hash. The file is
deterministic: the same sources always give the same bytes and the same hash. Others install it
with `pwc module add <file>`, or by placing the file or the source directory in a directory they use
as a repository (`pwc repo add <dir>`), after which `pwc module add alice.altimeter` finds it.

A published version is immutable. Every change to the package, however small, needs a new
`version` in `module.toml`: when re-locking finds the same id and version with a different hash,
`pwc` refuses it. Breaking changes to your Rust API, persisted state or settings need a new major
version.

Format 1 packages are native code for local repositories and direct sharing. The official registry,
once it exists, will accept only modules built for the sandboxed WebAssembly module ABI
([POLICY.md, section 3](../POLICY.md#3-package-and-execution-model)).

## Libraries and bundles

`kind` in `module.toml` selects one of three package kinds:

- `"module"` (the default): has `src/lib.rs` with `register`, as above.
- `"library"`: Rust code for other packages to use. It has `src/lib.rs` without `register` and
  installs no modules. Like a module's, its code must be under a software licence.
- `"bundle"`: no code at all, only `[dependencies]`. A bundle is a curated set of packages, such as
  `pwc.essentials`. Bundles omit `pwc-api`.
