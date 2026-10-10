# PWC Package Manager

`pwc` installs, resolves and builds mods for [Project Watt Cubed](https://github.com/gusahlg/project_watt_cubed)
(PWC). PWC mods are Rust source compiled into the game executable, so a modded PWC installation
is a **reproducible build**: one PWC version plus one exact set of mod packages. `pwc` manages
that set per *instance*, records it in a lockfile and turns it into a native executable.

This repository holds the tooling (a Rust workspace under [`crates/`](crates/)) and the
first-party mod packages (under [`mods/`](mods/)), which also serve as the first package
repository. Read [docs/architecture.md](docs/architecture.md) for the design.

## Quick start

You need Rust 1.90 or newer and a checkout of the game. By default `pwc setup` looks for it next to
this repository, as `../project_watt_cubed`.

```sh
cargo install --path crates/pwc-cli   # installs `pwc` into ~/.cargo/bin

pwc setup                             # find the game, register mods/ as a repository
pwc instance create default --use     # a new, empty instance, made active
pwc mod add pwc.essentials            # resolve, store and lock the essentials
pwc run                               # build the instance and start the game
```

What each step does:

1. `pwc setup` writes `~/.config/pwc/config.toml`: the game checkout to build against and the
   repositories to search, including this repository's `mods/`. Use `--pwc <dir>` if the game is
   elsewhere and `--repository <dir>` to add more repositories.
2. `pwc instance create` writes `~/.local/share/pwc/instances/default/instance.toml`, the
   instance's desired state. An instance with no mods builds the bare core.
3. `pwc mod add` adds the requirement, resolves it and its dependencies to exact versions, copies
   every selected package into the content-addressed store and writes `pwc.lock`.
4. `pwc run` generates a small Cargo workspace for the lock under `~/.cache/pwc/builds/`, builds
   it and runs the game with the instance's own data directory (worlds, settings).

On Windows, the equivalent state root is `%LOCALAPPDATA%\pwc`; no `HOME` or XDG environment
variables are required. The game build still needs the Rust GNU toolchain and Vulkan SDK available
to Cargo.

The first build compiles the whole game and takes as long as a normal release build of PWC.
Later builds share one Cargo target directory, so only changed crates are recompiled, and an
unchanged lock reuses the finished executable without invoking Cargo. When `nix` is installed and
the game checkout has a `flake.nix`, `pwc` runs Cargo inside the game's `nix develop` shell, which
provides the game's native libraries (`PWC_NO_WRAPPER=1` turns this off).

Other everyday commands:

```sh
pwc search                 # packages available from repositories and the store
pwc mod list               # the locked packages of the active instance
pwc mod update             # move to the newest compatible versions
pwc env                    # the instance's environment hash
pwc store gc               # delete packages no instance uses
pwc cache clean            # delete build outputs (always safe)
```

The full command reference is [docs/spec/cli.md](docs/spec/cli.md).

## Building and installing

```sh
cargo install --path crates/pwc-cli   # install the `pwc` binary
cargo run -p pwc-cli -- <command>     # or run it from the checkout without installing
```

With Nix, install the flake's package into your profile instead:

```sh
nix profile add .                     # or github:gusahlg/pwc-package-manager
nix profile upgrade pwc-package-manager   # after pulling a new version
```

A profile is a garbage-collection root. A `pwc` built inside `nix develop` and copied onto your
`PATH` links against store paths nothing keeps alive, and stops starting after the next garbage
collection ("Failed to execute process … Check the interpreter or linker").

For development, `nix develop` provides the toolchain (see [CONTRIBUTING.md](CONTRIBUTING.md)). The
tooling itself is pure Rust and needs no C compiler or system libraries; building a game instance
needs whatever the game needs (see the game's README).

## Repository layout

```text
crates/                 the tooling: one library crate per responsibility, plus the CLI
mods/                   first-party mod packages; the first local package repository
docs/architecture.md    how the system fits together
docs/writing-a-mod.md   tutorial for mod authors
docs/spec/              normative specifications of the formats and behaviour (format 1)
POLICY.md               which packages are accepted
LICENSE                 the tooling's licence, AGPL-3.0-or-later
LICENSES/, REUSE.toml   licence texts and per-file licensing (REUSE)
flake.nix               Nix development shell and the `pwc` package
```

## Crates

| Crate | Responsibility |
|---|---|
| [`pwc-manifest`](crates/pwc-manifest) | The data formats: `mod.toml`, `instance.toml` and `pwc.lock`, with validation and the licence policy. |
| [`pwc-package`](crates/pwc-package) | The `.pwcmod` format: collecting package contents, canonical archives, hashing and verification. |
| [`pwc-store`](crates/pwc-store) | The content-addressed, immutable package store. |
| [`pwc-resolver`](crates/pwc-resolver) | Deterministic dependency resolution, as a pure function. |
| [`pwc-instance`](crates/pwc-instance) | Instances, configuration, directories and repositories: turns `instance.toml` into `pwc.lock`. |
| [`pwc-builder`](crates/pwc-builder) | Turns an exact `pwc.lock` into a native executable: generated workspace, Build IDs, build cache. |
| [`pwc-cli`](crates/pwc-cli) | The `pwc` command, a thin front end over the libraries. |

## Specifications

The specifications in [`docs/spec/`](docs/spec/) are normative; the crates implement them.

- [mod-manifest.md](docs/spec/mod-manifest.md): `mod.toml`, the mod manifest
- [package-format.md](docs/spec/package-format.md): `.pwcmod`, the package file and its hash
- [instances.md](docs/spec/instances.md): `instance.toml`, `pwc.lock` and the environment hash
- [filesystem.md](docs/spec/filesystem.md): directories, `config.toml`, the store and repositories
- [resolver.md](docs/spec/resolver.md): dependency resolution
- [build.md](docs/spec/build.md): the builder, Build IDs and the build cache
- [cli.md](docs/spec/cli.md): the `pwc` command

## Mod policy

[POLICY.md](POLICY.md) governs every package `pwc` accepts. It is based on the game's official mod
policy, `MOD_POLICY.md`, which originally required all mod code to be `AGPL-3.0-or-later`; this
policy accepts AGPL-compatible free licences instead, and the game's `MOD_POLICY.md` and
`GOVERNANCE.md` are being updated to match. In short:

- **Free software only. Binary-only and proprietary mods are not accepted.** A package's licence
  is an SPDX expression over an allowlist of free licences: software licences compatible with the
  game's `AGPL-3.0-or-later` (mods are compiled into the AGPL game), permissive such as `MIT` and
  `Apache-2.0` or copyleft such as `MPL-2.0` and `AGPL-3.0-or-later`, and free-content licences
  for assets and documentation. Mod code must always be under a software licence, and assets
  compiled into the executable should use a licence that permits combination with AGPL software.
  `Apache-2.0 OR MIT` is recommended. A package ships the full licence texts and its complete
  source.
- **Format 1 packages are trusted native code.** They are compiled into the game, are not sandboxed
  and run with the game's privileges. They cannot contain `build.rs`, proc macros or crates.io
  dependencies, so compiling a package runs no code from it and its whole source is in the package.
  This is the local, pre-registry stage: the official registry will accept only mods for the
  sandboxed WebAssembly mod ABI.
- **Ids are permanent and versions are immutable.** `pwc` refuses a locked version whose contents
  changed without a version bump. The `pwc` namespace is reserved for the first-party packages in
  this repository.
- **Mods respect the game's contract**: matter is not a mod, names are presentation, world-affecting
  code is deterministic and the server is the authority.

`pwc package check`, `pwc package build` and `pwc mod add` enforce the machine-checkable rules.

## First-party packages

| Package | Description |
|---|---|
| `pwc.menus` | The standard menu look: title and panel screens, bars and toggles. |
| `pwc.start-screen` | The default start screen: main menu, Worlds page, host and join forms. |
| `pwc.hotbar` | Nine slots plus the bare hand; the selected slot is the held tool. |
| `pwc.inventory` | Your held materials as a list (press I); equip any of them into the hotbar. |
| `pwc.game-ui` | The in-world HUD: reticle, coordinates, frame rate, player count, loading lines and a facing indicator. |
| `pwc.chat` | The text chat on § (the key left of 1) and the scrollback of the game's messages. |
| `pwc.commands` | Slash commands in the chat (`/help`, `/gfx`, `/time`, the audio commands, `/op`) and a registry for other mods' commands. |
| `pwc.chat-commands` | Bundle of the chat and its commands. |
| `pwc.visuals` | The shipped look: atmosphere, post-processing and lighting. |
| `pwc.neural-textures` | Paints every material with a texture grown from its own elements. |
| `pwc.material-names` | Names every material and tool with a small model trained on mineral and element names. |
| `pwc.infinite-diffusion` | InfiniteDiffusion, the world generator: surface, underground and space. |
| `pwc.essentials` | Bundle of all of the above: the default set of mods. |
| `pwc.dev-toolkit` | Developer Toolkit: travel and inspection commands (`/tp`, `/cruise`, `/inspect`...) through `pwc.commands`, and flight on F. Not in the essentials. |

Every first-party package is licensed `Apache-2.0 OR MIT`, at your option: code, manifest,
documentation and assets alike.

See [mods/README.md](mods/README.md) for how the package repository is organised, and
[docs/writing-a-mod.md](docs/writing-a-mod.md) to write your own.

## Status

Version 0.1.0 (unreleased) implements phases 1–10 of the [roadmap](docs/architecture.md#roadmap):
the format 1 specifications, packaging, the store, instances, the resolver, the builder,
`pwc build` and `pwc run` with the build cache, and the first-party packages. Packages come from
local repositories, local source trees and `.pwcmod` files.

Next:

- **Registry** (phase 11): a separate `pwc-registry` service for publishing and downloading
  packages. Not started; local repositories stand in for it.
- **Launcher** (phase 12): a graphical front end over the same libraries as `pwc`.
- **Environment checks in the game**: multiplayer and world saves identifying an environment by
  the lock's environment hash.

## Licence

The package manager (the tooling in `crates/`, its specifications and documentation: everything in
this repository except the package directories under `mods/`) is licensed under the GNU Affero
General Public License, version 3 or (at your option) any later version, `AGPL-3.0-or-later`
([LICENSE](LICENSE)).

Packages carry their own licences, chosen by their authors within the
[mod policy](POLICY.md#1-licensing). The first-party packages in `mods/<package>/` are licensed
under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option, and each package carries its own copies of both texts in its `LICENSES/`
directory.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in a
first-party package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above,
without any additional terms or conditions. Contributions to the package manager are licensed
`AGPL-3.0-or-later`, the licence of the files they change (inbound = outbound). Every commit needs
a Developer Certificate of Origin sign-off (`git commit -s`); see
[CONTRIBUTING.md](CONTRIBUTING.md#commits-and-pull-requests).

The repository follows the [REUSE](https://reuse.software) specification: [REUSE.toml](REUSE.toml)
records the licence of every file and [LICENSES/](LICENSES/) holds every licence text. Check it with
`reuse lint`.

The game itself is licensed `AGPL-3.0-or-later`. A built instance compiles its mods into the game,
so distributing such an executable is governed by the AGPL, whatever free licences the individual
mods carry.
