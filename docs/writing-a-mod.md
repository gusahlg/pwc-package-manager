# Writing a mod

This tutorial builds a small mod, an altimeter that shows the player's height on the HUD, and
takes it from an empty directory to a `.pwcmod` package. It assumes `pwc` is installed and set up
with an active instance ([README.md, Quick start](../README.md#quick-start)).

A PWC mod is ordinary Rust compiled into the game. There is no `Cargo.toml` in a package: you
write a `mod.toml`, and the PWC builder generates the Cargo manifests from it. The rules every
package must follow are in [POLICY.md](../POLICY.md); the manifest is specified in
[spec/mod-manifest.md](spec/mod-manifest.md).

The code and manifest examples in this tutorial may be copied into your own mod under the terms of
`Apache-2.0 OR MIT`, whatever licence you choose for the rest of your mod (each example is marked as
such for [REUSE](https://reuse.software)); the surrounding text is part of this repository's
documentation.

## 1. Create the package directory

```text
altimeter/
├── mod.toml
├── README.md
├── LICENSES/
│   ├── Apache-2.0.txt
│   └── MIT.txt
├── assets/          optional: files embedded with include_bytes!/include_str!
└── src/
    └── lib.rs
```

Only `mod.toml`, the README, the licence files, `CHANGELOG.md`, `NOTICE`, `src/`, `assets/` and
`data/` become part of the package. Anything else in the directory (`.git/`, `target/`, `dist/`,
editor files, a development `Cargo.toml`) is ignored when packaging. Inside the included paths,
hidden files, symlinks, `build.rs` and `Cargo.toml` are errors.

## 2. Write `mod.toml`

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
pwc-api = "^1.0"
categories = ["interface"]
```
<!-- SPDX-SnippetEnd -->

- **`id`** is permanent: `<namespace>.<name>`, lowercase. Use your own namespace (a user or
  organisation name); `pwc` is reserved for first-party packages. The id also gives the Rust crate
  name, with `.` and `-` replaced by `_`: this package is the crate `alice_altimeter`.
- **`pwc-api`** is the version requirement on the `pwc-mod-api` crate your code compiles against.

### Licensing

PWC accepts only free software, and proprietary mods are not allowed
([POLICY.md, section 1](../POLICY.md#1-licensing)). The recommended licence is the one in the
example, **`Apache-2.0 OR MIT`**: users may take the code under either licence, it is the usual
choice for Rust code, and the first-party packages use it.

You may choose another licence from the policy's allowlist instead:

- another permissive licence, alone: `MIT`, `Apache-2.0`, `BSD-3-Clause`, `ISC`, `Zlib`, ...;
- a copyleft licence, so that modified versions of your mod stay free: `MPL-2.0` (per file),
  `LGPL-3.0-or-later`, `GPL-3.0-or-later` or `AGPL-3.0-or-later` (the game's own licence, which
  also covers users of a modified version over a network);
- a content licence for assets or documentation, joined with `AND`, for example
  `(Apache-2.0 OR MIT) AND CC-BY-4.0` or `AGPL-3.0-or-later AND CC-BY-SA-4.0` (for assets compiled
  into the game, see [section 10](#10-assets)).

The rules `pwc` checks: the expression is SPDX (ids are case-sensitive; `AND`, `OR`, `WITH` and
parentheses); every id is on the allowlist, whose software licences are all compatible with the
game's `AGPL-3.0-or-later`, because your mod is compiled into the AGPL game (its content licences
are for assets and documentation); and the code is under a software licence whichever `OR`
alternative a user picks, so a content licence alone, or `MIT OR CC-BY-4.0`, is rejected for a
`mod` or `library`. Unlisted licences, `LicenseRef-*`, `NOASSERTION`, `NONE` and a trailing `+` are
rejected (write `GPL-3.0-or-later`, not `GPL-3.0+`).

<!-- REUSE-IgnoreStart -->
Include the full text of every licence you name and list the files in `license-files`. The
[REUSE](https://reuse.software) tool fetches them: `reuse download Apache-2.0 MIT` writes both into
`LICENSES/`; then replace the placeholder copyright line in `LICENSES/MIT.txt` with your own, such
as `Copyright (c) 2026 Alice Example`. The first-party packages in this repository's
[mods/](../mods/) directory show the finished layout.

It also helps to mark each file's licence, for example with a
`// SPDX-License-Identifier: Apache-2.0 OR MIT` line at the top of every source file.
<!-- REUSE-IgnoreEnd -->

Whatever licence you choose, a build of PWC that contains your mod is distributed under the game's
AGPL as a whole, and its recipients are entitled to the source of everything in it. Published
packages already are that source.

## 3. Write the README

`README.md` is required. Say what the mod does, how to use it (keys, settings), what it depends on,
what it persists, and anything a player must know before installing it, such as file or network
access ([POLICY.md, section 3](../POLICY.md#3-package-and-execution-model)).

## 4. Write the code

`src/lib.rs` defines the entry point, `register`. The builder calls it once at startup, after
every dependency has registered. It installs one or more values implementing `Mod`.

<!-- SPDX-SnippetBegin -->
<!-- SPDX-SnippetCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->
<!-- REUSE-IgnoreStart -->
```rust
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Altimeter: shows the player's height in the top-right corner.

use pwc_mod_api::prelude::*;

/// Entry point, called once by the generated mod bundle.
pub fn register(registrar: &mut ModRegistrar) {
    registrar.add(Altimeter);
}

struct Altimeter;

impl Mod for Altimeter {
    fn name(&self) -> &str {
        "Altimeter"
    }

    fn id(&self) -> &'static str {
        "alice.altimeter"
    }

    fn description(&self) -> &str {
        "Your height in the top-right corner."
    }

    fn hud(&self, _world: &World, player: &Player, _screen: (i32, i32), out: &mut Vec<HudElement>) {
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

`pwc_mod_api::prelude` re-exports what most mods need: `Mod`, `ModRegistrar`, `ModContext`,
`Player`, `World`, `BlockId`, `BlockRegistry`, the HUD types (`HudElement`, `Anchor`, `Role`,
`Line`, `Panel`, `Row`), `Knob`, `Group` and `ESSENTIALS`. Everything else lives in modules such as
`pwc_mod_api::ui`, `pwc_mod_api::world` and `pwc_mod_api::block`.

`name` and `id` are the only required methods. `name` is the label on the Mods screen and may
change between versions. `id` is the stable key the game uses to persist the mod's on/off choice,
settings and per-world state, so it must never change; use your package id, or the package id plus
a suffix when a package installs several mods.

Every other method is an optional hook with a no-op default. The most common ones:

| Hook | Called | Use it to |
|---|---|---|
| `description`, `group` | Mods screen | Describe the mod; place it in a group. |
| `update(&mut self, ctx)` | Every mod tick | React to input and change state through `ModContext`. |
| `hud(&self, world, player, screen, out)` | Every frame | Push `HudElement`s to draw. |
| `command(&mut self, cmd, args)` | Console line | Handle a `/command`. |
| `knobs`, `step_knob` | Mods screen | Offer settings the player can change. |
| `save_choice_state`, `load_choice_state` | `mods.cfg` | Persist those settings. |
| `save_state`, `load_state` | World save and load | Persist per-world state, with a payload version. |
| `on_enable`, `on_disable`, `reset` | Toggle, world change | Set up and clear state. |
| `held`, `on_block_break`, `on_tool_changed` | Gameplay events | Take part in holding and using matter. |
| `appearance`, `namer`, `worldgen`, `start_screen`, `menu_theme` | When the game needs one | Replace a core default (first enabled mod wins). |

This list is not complete; see the `pwc-mod-api` documentation for every hook, its arguments and
how the game arbitrates when several mods implement it (fan-out in install order, or first enabled
mod wins). To read it, run `cargo doc -p pwc-mod-api --open` in the game checkout.

Two rules matter from the first line of code:

- **Hooks are not hot paths.** `hud` and `update` run every frame or tick: keep them cheap and
  cache anything expensive. Never do per-voxel work in a hook.
- **Anything that affects the world must be deterministic** (no wall-clock time, unseeded
  randomness or hash-map iteration order), and names and looks must never flow back into the law,
  world generation, saves or the network. A HUD label is presentation, so the altimeter is free
  to format floats. See [POLICY.md, section 4](../POLICY.md#4-playing-well-with-pwc-the-mod-contract).

## 5. Check the package

```sh
pwc package check ./altimeter
```

This validates the manifest, the licence policy and the package contents without compiling
anything. Fix everything it reports; `pwc mod add` refuses packages that fail these checks.

## 6. Run it

Add the source directory to your instance as a local `path` entry and start the game:

```sh
pwc mod add ./altimeter
pwc run
```

`pwc mod add` records the directory's absolute path in `instance.toml`
(`"alice.altimeter" = { path = "/home/alice/src/altimeter" }`). A path entry is mutable: when you
edit the source, the next `pwc run` re-locks with the new hash and rebuilds. Only your crate, its
dependents and the generated crates recompile; the game is reused from the shared target
directory.

The compiler builds the snapshot of your sources taken at lock time, so its messages name files
under `~/.local/share/pwc/store/<hash>/`; the paths below that directory mirror your source tree.
For editor support you may keep a development `Cargo.toml` next to `mod.toml` (for example one
that depends on `pwc-mod-api` by path in your game checkout); packaging ignores it.

## 7. Settings

A knob is a setting the player changes on the Mods screen. This version of the altimeter lets the
player choose how many decimals to show and remembers the choice in `mods.cfg`:

<!-- SPDX-SnippetBegin -->
<!-- SPDX-SnippetCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->
```rust
struct Altimeter {
    decimals: usize,
}

impl Mod for Altimeter {
    // name, id, description and hud as before; hud formats with
    // format!("{:.*} m", self.decimals, player.position.y)

    fn knobs(&self) -> Vec<Knob> {
        vec![Knob { label: "decimals", value: self.decimals.to_string(), hint: "0-2".into() }]
    }

    fn step_knob(&mut self, _index: usize, delta: i32) {
        self.decimals = (self.decimals as i32 + delta).clamp(0, 2) as usize;
    }

    fn save_choice_state(&self) -> Option<String> {
        Some(self.decimals.to_string())
    }

    fn load_choice_state(&mut self, data: &str) {
        if let Ok(decimals) = data.parse::<usize>() {
            self.decimals = decimals.min(2);
        }
    }
}
```
<!-- SPDX-SnippetEnd -->

Treat persisted data as untrusted input: an old or hand-edited file must not crash the game.
Changing what you persist in an incompatible way requires a major version bump.

## 8. Depend on another mod

A mod can use another package's Rust API. Declare the dependency in `mod.toml`:

<!-- SPDX-SnippetBegin -->
<!-- SPDX-SnippetCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->
```toml
[dependencies]
"pwc.hotbar" = "^1.0"
```
<!-- SPDX-SnippetEnd -->

The resolver then puts exactly one compatible version of `pwc.hotbar` into every build that
contains your mod, registers it before yours, and makes it available to your code as the crate
`pwc_hotbar`. Use the types `pwc.hotbar` exports with `use pwc_hotbar::…;`; its
[README](../mods/hotbar/README.md) and crate documentation describe them.

- Format 1 has no optional dependencies: whatever you list is always part of the build.
- Only dependencies you declare are visible to your code, and only through their public Rust API.
- A major version bump of a dependency signals an incompatible API change, so `^1.0` keeps you on
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

use pwc_mod_api::prelude::*;

/// The player's current height, shared with packages that depend on alice.altimeter.
#[derive(Clone)]
pub struct AltitudeHandle(Rc<Cell<f64>>);

impl AltitudeHandle {
    /// The height at the last mod tick.
    pub fn get(&self) -> f64 {
        self.0.get()
    }
}

pub fn register(registrar: &mut ModRegistrar) {
    let altitude = AltitudeHandle(Rc::new(Cell::new(0.0)));
    registrar.provide(altitude.clone());
    registrar.add(Altimeter { altitude, decimals: 0 });
}

// In `impl Mod for Altimeter`:
//     fn update(&mut self, ctx: &mut ModContext) {
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
use pwc_mod_api::prelude::*;

pub fn register(registrar: &mut ModRegistrar) {
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
  your `Mod` values.
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
`mod.toml` names an `.svg` or `.png` under `assets/` for the package.

An embedded asset becomes part of the AGPL-licensed game executable, so give it a licence that
permits combination with AGPL software: your package's software licence, `CC0-1.0`, `CC-BY-4.0` or
`CC-BY-SA-4.0`. Fonts under `OFL-1.1` may be bundled too. Keep assets under `LAL-1.3` as separate
data files rather than compiling them in
([POLICY.md, section 1](../POLICY.md#why-only-agpl-compatible-licences)).

## 11. Build the package

When the mod is ready to share:

```sh
pwc package build ./altimeter
```

This writes `altimeter/dist/alice.altimeter-0.1.0.pwcmod` and prints its hash. The file is
deterministic: the same sources always give the same bytes and the same hash. Others install it
with `pwc mod add <file>`, or by placing the file or the source directory in a directory they use
as a repository (`pwc repo add <dir>`), after which `pwc mod add alice.altimeter` finds it.

A published version is immutable. Every change to the package, however small, needs a new
`version` in `mod.toml`: when re-locking finds the same id and version with a different hash,
`pwc` refuses it. Breaking changes to your Rust API, persisted state or settings need a new major
version.

Format 1 packages are native code for local repositories and direct sharing. The official registry,
once it exists, will accept only mods built for the sandboxed WebAssembly mod ABI
([POLICY.md, section 3](../POLICY.md#3-package-and-execution-model)).

## Libraries and bundles

`kind` in `mod.toml` selects one of three package kinds:

- `"mod"` (the default): has `src/lib.rs` with `register`, as above.
- `"library"`: Rust code for other packages to use. It has `src/lib.rs` without `register` and
  installs no mods. Like a mod's, its code must be under a software licence.
- `"bundle"`: no code at all, only `[dependencies]`. A bundle is a curated set of packages, such as
  `pwc.essentials`. Bundles omit `pwc-api`.
