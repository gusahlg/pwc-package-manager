# Architecture

A PWC installation with modules is a **reproducible build**: one PWC version plus one exact set of module
packages. Modules are Rust source compiled into the game executable. Nothing scans directories or
loads code at run time; when the set of modules changes, the executable is rebuilt.

This document describes how the pieces fit together. The formats and behaviour are specified
precisely in [`spec/`](spec/); where this document and a specification disagree, the
specification wins.

## Four ideas

The system keeps four ideas separate.

- **Module**: source code and assets that extend PWC through the PWC Module API. A module is what an author
  writes and what a player adds to an instance.
- **Package**: one immutable, distributable version of one module, a `.pwcmodule` file. Its identity is
  the SHA-256 of its canonical contents; the same id and version never have different contents.
- **Instance**: one PWC setup and its modules. `instance.toml` is the desired state ("the essentials,
  plus my cave module"); `pwc.lock` is the exact resolved state (every package, version and hash,
  plus the PWC source), summarised by an **environment hash**.
- **Build**: the transformation of one exact lock into a native executable.

## Three questions, three owners

| Question | Answered by |
|---|---|
| Which modules belong to this instance? | The package manager: manifests, packages, the store, the resolver, instances. |
| How do these exact modules become an executable? | The builder. |
| What is a module, and how does it interact with the game? | PWC itself: the `Module` trait, the registrar, `GameBuild`, the runtime. |

The package manager never compiles anything. The builder never chooses versions and never
interprets module code. The game knows nothing about packages, versions, stores or instances: it
receives a `GameBuild` listing the packages compiled into it and calls each one's `register`
function in order.

## Repositories

| Repository | Contents |
|---|---|
| [`project_watt_cubed`](https://github.com/gusahlg/project_watt_cubed) | The game: the runtime library (`project_watt_cubed::run(GameBuild)`), the `pwc-module-api` crate and example modules. It contains no installed modules; the vanilla executable is `run(GameBuild::vanilla())`. |
| `pwc-package-manager` (this repository) | The tooling: `pwc-manifest`, `pwc-package`, `pwc-store`, `pwc-resolver`, `pwc-instance`, `pwc-builder` and `pwc-cli`. A launcher will be another front end over the same libraries. The first-party module packages in `modules/` act as the first local repository. |
| `pwc-registry` (future) | Remote publishing and downloads. Not started. |

## Flow

```text
  Repositories            path / file entries        Package store
  (modules/, other dirs)     in instance.toml           (already installed)
          |                       |                          |
          +-----------------------+--------------------------+
                                  |  candidates: id, version, kind, hash,
                                  |  pwc-api, dependencies, conflicts
                                  v
  instance.toml ------->  +-----------------------+  <-------  PWC source: version,
  (desired state)         |  Dependency resolver  |            pwc-module-api version
  previous pwc.lock ----> +-----------------------+
                                  |  one exact set of packages
                                  v
                          +-----------------------+
                          |     Package store     |  verify hash, insert, read-only
                          +-----------------------+
                                  |
                                  v
                          pwc.lock  (exact state + environment hash)
                                  |
                                  v
                          +-----------------------+
                          |      PWC builder      |  Build ID = hash of all build inputs
                          +-----------------------+
                                  |  generates
                                  v
          $XDG_CACHE_HOME/pwc/builds/<build-id>/
            instance/   main() = project_watt_cubed::run(pwc_bundle::game_build())
            bundle/     the module bundle: a GameBuild listing the locked packages
            modules/*/     one Cargo.toml per package; sources stay in the store
                                  |
                                  v
                          cargo + rustc  (shared target dir: $XDG_CACHE_HOME/pwc/target)
                                  |
                                  v
                          Build cache: builds/<build-id>/bin/pwc-game
                                  |
                                  v  pwc run
                          Native PWC executable; data in instances/<name>/game/
```

**Locking** (`pwc module add`, `pwc module update`, `pwc lock`) runs the top half: collect candidates
from the configured repositories, the store and explicit `path`/`file` entries; resolve; insert
every selected package into the store, verifying its hash; write `pwc.lock`.

**Building** (`pwc build`, `pwc run`) runs the bottom half: compute the Build ID; if the build
cache already holds that build, use it; otherwise generate the workspace and run Cargo. The
generated crates reference the PWC source and the store directories by path. The PWC source is
never modified and no package file is copied.

## Components

| Component | Responsible for | Not responsible for |
|---|---|---|
| `pwc-manifest` | Parsing and validating `module.toml`, `instance.toml`, `pwc.lock`; the licence policy; the environment hash. | Archives, the store, resolution. |
| `pwc-package` | Package contents, the canonical tar, Zstandard framing, the package hash, verification. | Where packages live. |
| `pwc-store` | Atomic, content-addressed insertion; read-only entries; verification; deleting entries outside a given live set. | Choosing packages. |
| `pwc-resolver` | Choosing one version per id that satisfies every requirement, deterministically, with a useful explanation on failure. | Filesystem, network, clock. |
| `pwc-instance` | XDG directories, `config.toml`, repositories, instances; locking (`instance.toml` to `pwc.lock`); the live set for garbage collection. | Compiling. |
| `pwc-builder` | Build IDs, the generated workspace and module bundle, invoking Cargo, the build cache. | Choosing versions; interpreting module code. |
| `pwc-cli` | The `pwc` command: argument parsing and output. | Package logic of any kind. |
| PWC (`project_watt_cubed`, `pwc-module-api`) | What a module is: the `Module` trait and its hooks, `ModuleRegistrar`, `GameBuild`, `run`. | Packages, versions, instances. |

## Principles

**Source is canonical; compiled artifacts are disposable caches.** A package contains source and
assets, never compiled code, and there is no custom compiled intermediate format. Crate-level reuse
comes from Cargo itself, through one target directory shared by every build. Deleting the cache is
always safe: everything can be rebuilt from the store and the PWC source.

**State lives outside source trees**, following the XDG base directory conventions:

| Root | Contents | Lifetime |
|---|---|---|
| `$XDG_DATA_HOME/pwc/` | `store/` (immutable packages), `instances/` (manifests, locks, game data) | Persistent |
| `$XDG_CACHE_HOME/pwc/` | `builds/` (generated workspaces, executables), `target/` (shared Cargo target) | Disposable |
| `$XDG_CONFIG_HOME/pwc/` | `config.toml` | Persistent |

`PWC_HOME` overrides all three roots. On Windows, `HOME` is not consulted. See
[spec/filesystem.md](spec/filesystem.md).

**Everything is determined by its inputs.** The same files produce the same package bytes on every
machine. The resolver is a pure function. The same lock yields the same environment hash
everywhere, and the same build inputs yield the same Build ID.

| Identity | Hash of | Used by |
|---|---|---|
| Package hash | The package's canonical tar | The store (directory name), the lock, the future registry |
| Environment hash | The PWC version, revision and API version, plus every locked package's id, version, kind, hash and dependencies | The lock; the game (`PWC_ENVIRONMENT`, `GameBuild::environment`) |
| Build ID | The environment hash, the builder version, the exact PWC source state, the `rustc` version, the profile and the target | The build cache |

**Garbage collection walks instance locks.** A store entry is live while some instance's
`pwc.lock` references it; `pwc store gc` deletes the rest. Removing a module from an instance or
deleting an instance never deletes packages by itself.

## Security model

Format 1 is the **local, pre-registry stage** of the system: packages are native Rust, compiled
into the game, and **not sandboxed**. A module runs with the same privileges as the game: it can read
and write files, open network connections and start processes. Installing a package means trusting
its authors and reviewing what you install, as with any native program.

The format limits where code can hide and makes what you run identifiable:

- **No code runs at build time.** Format 1 forbids `build.rs`, proc-macro crates and crates.io
  dependencies, so packaging, locking and compiling execute nothing from a package. Module code runs
  only when the game runs.
- **The whole source is in the package.** A package may use only the Rust standard library,
  `pwc-module-api` and its declared module dependencies, which are packages themselves. What you review
  is what gets compiled.
- **Content addressing.** Every package is identified by its SHA-256. The lock records the hash of
  every package, the store refuses contents that do not match, entries are read-only after
  insertion, and `pwc store verify` re-checks them. Two machines with the same lock compile the
  same code.
- **Immutability.** A published version never changes. When re-locking finds a package with the
  same id and version as the previous lock but a different hash, locking fails ("changed contents
  without a version bump"). Only `path` entries, which are mutable development trees, may change
  under the same version; their new hash is recorded.
- **Policy.** Only free software is accepted and proprietary modules are not allowed: a package's
  licence must come from an allowlist of free licences, its code must be under a software licence
  compatible with the game's `AGPL-3.0-or-later` (modules are compiled into the AGPL game;
  `Apache-2.0 OR MIT` is recommended), assets and documentation may use free-content licences, and
  packages must not access the network, start processes or touch files outside the module API unless
  that is their documented purpose. See [POLICY.md](../POLICY.md).

What format 1 does not provide: isolation from a malicious module, or verification of who wrote a
package.

The **official registry** closes those gaps. As the module policy requires, official user-installable
modules will target the versioned, **sandboxed WebAssembly module ABI**: a package declares its
components (server, client, shared, tool) and the capabilities it needs, and the host grants only
those. The registry builds every accepted revision from source in an isolated environment, checks
reproducibility and signs the result. Format 1 native packages will not be published as official
registry modules; they remain the format for local repositories, development and direct sharing.

## Multiplayer and worlds

The environment hash identifies an exact build with modules. The builder embeds it in the executable
(`GameBuild::environment`) and `pwc run` passes it as `PWC_ENVIRONMENT`, so the game can compare
environments when a client joins a server or a world is opened. Using it for those checks is
future work in the game. A lock made from a PWC source with uncommitted changes (`dirty = true`) is
a development environment: its hash does not capture those changes and must not identify a release
environment.

## Terminology

- **Module**: source code and assets implementing an extension to PWC. A module package registers one or
  more values implementing the `Module` trait.
- **PWC Module API**: the `pwc-module-api` crate in the game repository; the surface module code compiles
  against (`Module`, `ModuleRegistrar`, `ModuleContext`, `GameBuild` and the types hooks use). Versioned
  with SemVer; packages declare the version they need with `pwc-api`.
- **Module manifest**: `module.toml`, a package's identity, licence, API requirement and dependencies.
  See [spec/module-manifest.md](spec/module-manifest.md).
- **Module package**: a `.pwcmodule` file, one immutable version of one module: a Zstandard-compressed
  canonical tar identified by its SHA-256. See [spec/package-format.md](spec/package-format.md).
- **Package store**: the content-addressed directory of unpacked packages under
  `$XDG_DATA_HOME/pwc/store/`.
- **Repository**: a local directory of package source trees or `.pwcmodule` files that `pwc`
  searches for candidates; the offline stand-in for the registry. This repository's `modules/` is
  the first-party repository.
- **Registry**: the future official service for publishing and downloading packages. It builds
  packages from source, signs them and accepts only modules for the sandboxed WebAssembly module ABI.
- **Instance**: one PWC setup and its modules, with its own game data directory.
- **`instance.toml`**: an instance's desired state: the PWC version requirement, the requested modules
  and the build profile. Edited by `pwc module add/remove` or by hand.
- **`pwc.lock`**: an instance's exact resolved state: the PWC source and every package with its
  version and hash. Generated; never edited by hand. See [spec/instances.md](spec/instances.md).
- **Environment hash**: the SHA-256 identifying a lock's exact contents.
- **Dependency resolver**: the pure function that selects one version per id satisfying every
  requirement. See [spec/resolver.md](spec/resolver.md).
- **PWC builder**: the component that turns a lock into an executable. See
  [spec/build.md](spec/build.md).
- **Module bundle**: the generated crate that lists the locked packages as a `GameBuild` in
  registration order (dependencies first).
- **Build ID**: the hash of every build input, naming one build directory.
- **Build cache**: finished builds under `$XDG_CACHE_HOME/pwc/builds/`, plus the shared Cargo target
  directory. Disposable.
- **Launcher**: a future graphical front end offering the same operations as `pwc` by calling the
  same library crates.

## Roadmap

Phases 1–10 make up release 0.1.0 (unreleased at the time of writing).

| Phase | Deliverable | Where | Status |
|---|---|---|---|
| 1 | Composable PWC: the game is a library entered through `run(GameBuild)`; the core installs no modules. | game | 0.1.0 |
| 2 | The `pwc-module-api` crate. | game | 0.1.0 |
| 3 | `module.toml` and `.pwcmodule` (format 1). | `pwc-manifest`, `pwc-package` | 0.1.0 |
| 4 | The tooling repository with local packages: the first-party modules as a repository. | this repository | 0.1.0 |
| 5 | The content-addressed store. | `pwc-store` | 0.1.0 |
| 6 | Instances: `instance.toml`, `pwc.lock`, the environment hash. | `pwc-instance` | 0.1.0 |
| 7 | The dependency resolver. | `pwc-resolver` | 0.1.0 |
| 8 | The builder: generated instance crate and module bundle. | `pwc-builder` | 0.1.0 |
| 9 | `pwc build` and `pwc run`. | `pwc-cli` | 0.1.0 |
| 10 | The build cache: Build IDs and the shared target directory. | `pwc-builder` | 0.1.0 |
| 11 | The registry: remote publishing and downloads of source-built, signed packages for the sandboxed WebAssembly module ABI. | `pwc-registry`, game | Not started |
| 12 | The launcher: a graphical front end. | new crate | Not started |
