# PWC mod policy

This policy governs every package the PWC package manager accepts: the first-party packages in
this repository's `mods/` directory, packages built with `pwc package build` and installed with
`pwc mod add`, and — once it exists — the official PWC registry. It is based on the project's
official mod policy (`MOD_POLICY.md` in the
[Project Watt Cubed repository](https://github.com/gusahlg/project_watt_cubed)), departs from it in
the licensing rules of §1 (see [Relation to the game's mod policy](#relation-to-the-games-mod-policy)),
and adds what the package format and the compile-in build model require.

`pwc` enforces the machine-checkable rules, marked **[enforced]**; maintainers review the rest.
Policy version 1, 2026-10-03.

> **Free software only. Proprietary mods are not allowed.** The official mod ecosystem exists to
> distribute software that every recipient can study, modify, rebuild and share. Commercial
> free-software mods are welcome. **Binary-only or proprietary mods are not accepted.**

This policy governs the package manager, its repositories and the official registry. It does not
restrict private activity or independent forks beyond the licences of the code and content
involved.

## 1. Licensing

Every package is free software: its `license` names only licences from the allowlists below, and
its code is always under a free software licence. Proprietary licences are not accepted, in any
form or combination.

### Relation to the game's mod policy

The game's `MOD_POLICY.md` originally required all original mod code to be licensed
`AGPL-3.0-or-later`. This policy accepts AGPL-compatible free licences instead: mod code may use
any free software licence on the allowlist below, permissive or copyleft. The game's
`MOD_POLICY.md` and `GOVERNANCE.md` are being updated to match. The rest of the licensing rules
carry over: only free software is accepted, proprietary mods are not allowed, and a package ships
its complete source and the full text of every licence it names.

**Recommended:** license new mods `Apache-2.0 OR MIT`, the usual choice in the Rust ecosystem and
the licence of every first-party package. Copyleft licences (`MPL-2.0`, `LGPL-3.0-or-later`,
`GPL-3.0-or-later`, `AGPL-3.0-or-later`, ...) are accepted as well, for authors who want modified
versions of their mod to stay free.

### Why only AGPL-compatible licences

Format 1 mods are compiled into the game, and the game is licensed `AGPL-3.0-or-later`. The
executable that combines the game and its mods is therefore covered by the AGPL as a whole, and
whoever distributes it must be able to offer it, with the complete corresponding source of every
mod in it, under the AGPL's terms. A mod's code must permit that, so every *software* licence on
the allowlist is a free licence compatible with `AGPL-3.0-or-later`; software licences that are not
(for example `GPL-2.0-only`, or anything proprietary) are rejected, because a build containing such
a mod could not be distributed lawfully. A mod's own source keeps its own licence: someone who
receives an `Apache-2.0 OR MIT` package may reuse its code under those terms anywhere, while the
built game is distributed under the AGPL.

The *content* licences are accepted for assets and documentation, not for code, and the allowlist
does not claim that each of them is compatible with the AGPL. What matters is how an asset reaches
the game:

- An asset compiled into the executable (for example with `include_bytes!` or `include_str!`)
  becomes part of the AGPL-covered build, so it should use a licence that permits combination with
  AGPL software: `CC0-1.0`, `CC-BY-4.0`, `CC-BY-SA-4.0`, or the package's software licence.
- Fonts under `OFL-1.1` may be bundled, compiled in or not; the font itself stays under the OFL.
- Assets under `LAL-1.3` should be kept as separate data files rather than compiled into the
  executable.
- Documentation is not part of the executable and may use any content licence on the list.

### Licence expressions

- **[enforced]** `license` in `mod.toml` is an [SPDX licence
  expression](https://spdx.github.io/spdx-spec/v2.3/SPDX-license-expressions/): licence ids
  combined with `AND`, `OR`, `WITH <exception>` and parentheses. Ids are case-sensitive and must
  be written exactly as listed below.
- **[enforced]** Every licence id must be on one of the allowlists below, and every exception on
  the exception list. Rejected outright: unknown ids, ids not on the allowlists, a trailing `+`
  (write the `-or-later` id instead), `LicenseRef-*`, `DocumentRef-*`, `NOASSERTION`, `NONE`, and
  exceptions not on the list.
- **[enforced]** Code packages carry a software licence on every branch (the per-kind rule below).
- **[enforced]** `license-files` lists at least one licence text and every listed file is in the
  package. Reviewers check that it holds the full text of every licence the expression names.

`pwc package check`, `pwc package build` and `pwc mod add` reject a package that breaks these
rules; the error points to this file and states that proprietary licences are not accepted.

#### Software licences

Free software licences compatible with `AGPL-3.0-or-later`, for code (and accepted for anything
else in a package):

| Kind | Licences |
|---|---|
| Permissive | `0BSD`, `Apache-2.0`, `BlueOak-1.0.0`, `BSD-1-Clause`, `BSD-2-Clause`, `BSD-2-Clause-Patent`, `BSD-3-Clause`, `BSL-1.0`, `ISC`, `MIT`, `MIT-0`, `NCSA`, `PostgreSQL`, `UPL-1.0`, `Unicode-3.0`, `Zlib` |
| Public-domain dedications | `CC0-1.0`, `Unlicense` |
| Weak copyleft | `LGPL-2.1-or-later`, `LGPL-3.0-only`, `LGPL-3.0-or-later`, `MPL-2.0` |
| Strong copyleft | `AGPL-3.0-only`, `AGPL-3.0-or-later` (the game's licence), `GPL-3.0-only`, `GPL-3.0-or-later` |

#### Content licences

Free-content licences for assets and documentation (for assets compiled into the executable, see
[Why only AGPL-compatible licences](#why-only-agpl-compatible-licences)):

| Licence | Notes |
|---|---|
| `CC-BY-SA-4.0` | |
| `CC-BY-4.0` | |
| `CC0-1.0` | also a software licence |
| `OFL-1.1` | fonts |
| `LAL-1.3` | the Free Art License 1.3 (SPDX lists it under its French name, Licence Art Libre; the game's `MOD_POLICY.md` originally wrote it as FAL-1.3, which is not an SPDX id) |

#### Exceptions

Allowed after `WITH`:

| Exception | What it is |
|---|---|
| `LLVM-exception` | the LLVM project's exception to `Apache-2.0` |
| `Classpath-exception-2.0` | the GNU Classpath linking exception |
| `GCC-exception-3.1` | the GCC Runtime Library Exception |

#### Rules per package kind

| `kind` | Rule |
|---|---|
| `mod`, `library` | Every `OR` alternative must contain at least one software licence, so the code is under a software licence whichever alternative a recipient chooses. Content licences may be combined with `AND` for assets and documentation. |
| `bundle` | Any expression that passes the allowlist check (a bundle contains no code). |

Precisely, an expression *carries a software licence* when:

- a licence id is on the software list;
- `X WITH e` carries one when `X` does;
- `A AND B` carries one when `A` or `B` does;
- `A OR B` carries one when both `A` and `B` do.

A `mod` or `library` package's expression must carry a software licence.

| Expression | `mod`, `library` | `bundle` |
|---|---|---|
| `Apache-2.0 OR MIT` | accepted (recommended) | accepted |
| `MIT` | accepted | accepted |
| `MPL-2.0` | accepted | accepted |
| `AGPL-3.0-or-later AND CC-BY-SA-4.0` | accepted | accepted |
| `Apache-2.0 WITH LLVM-exception` | accepted | accepted |
| `(Apache-2.0 OR MIT) AND CC-BY-4.0` | accepted | accepted |
| `CC-BY-SA-4.0` | rejected: no software licence | accepted |
| `MIT OR CC-BY-4.0` | rejected: the `CC-BY-4.0` alternative has no software licence | accepted |
| `GPL-2.0-only` | rejected: not on the allowlist | rejected |
| `GPL-3.0+` | rejected: trailing `+` (use `GPL-3.0-or-later`) | rejected |
| `LicenseRef-Proprietary` | rejected: proprietary licences are not accepted | rejected |

### Assets, documentation and dependencies

- Assets must have complete provenance and be under an allowlisted licence: a content licence, or
  the package's software licence (the first-party packages license their icons and documentation
  `Apache-2.0 OR MIT` along with their code). An asset compiled into the executable should use a
  licence that permits combination with AGPL software (see
  [Why only AGPL-compatible licences](#why-only-agpl-compatible-licences)).
- A work claimed to be in the public domain needs evidence of a valid dedication or expired
  copyright; a bare assertion is not enough.
- Non-commercial, no-derivatives, source-available-only and custom field-of-use terms are not
  accepted, for code or content.
- Source dependencies must be free software under allowlisted licences and must keep their
  notices. (Format 1 packages can depend only on other packages, which follow this policy
  themselves.)
- Further licences and exceptions may be added after a review of their notice requirements and,
  for software licences and exceptions, of their compatibility with `AGPL-3.0-or-later`, in a pull
  request that changes this file and `crates/pwc-manifest/src/license.rs` together.

Receiving payment for a mod, for support or for development is allowed. Recipients keep every
freedom and source right the licences grant.

### Licences in this repository

The package manager itself (the tooling in `crates/`, its specifications and documentation, and
everything else outside the package directories `mods/<package>/`) is licensed
`AGPL-3.0-or-later`. The first-party packages in `mods/` are licensed `Apache-2.0 OR MIT`, and
each carries its own licence texts, as every package does. The tooling's licence covers the `pwc`
program and its libraries; packages keep their own licences.

## 2. Complete source

A package carries everything needed to rebuild and modify it: the preferred source form of code
and assets, licence texts, copyright notices and attribution. Packages are source packages by
construction — the builder compiles them; there is nothing else to ship.

- **[enforced]** Format 1 packages contain no `build.rs`, no `Cargo.toml`, no proc-macro crates
  and no crates.io dependencies: compiling a package runs no code from it, and its entire source is
  in the package (docs/spec/mod-manifest.md).
- Obfuscated source, missing editable originals of assets, proprietary toolchain requirements,
  prebuilt binaries or object files, and generated code without its generator are grounds for
  rejection.
- For the official registry every submission must also identify a public, immutable source
  revision and provide exact dependency versions and hashes (the package hash and the `pwc.lock`
  format provide these), build and test instructions, an SPDX software bill of materials, and the
  build recipe and hashes the package represents.

## 3. Package and execution model

- **Today (format 1): native, compiled in, trusted.** A package is Rust source compiled into the
  PWC executable by the PWC builder. **It is not sandboxed** — it runs with the game's privileges,
  so installing a package means trusting its authors and reviewing what you install. Packages are
  content-addressed and hash-verified **[enforced]**, and attributable to their authors.
- **Official registry: sandboxed.** As the official mod policy states, official user-installable
  mods must target the versioned, sandboxed WebAssembly mod ABI once that ABI and the registry are
  available; native dynamic libraries and arbitrary native executables are not accepted as official
  mods. At that point a package declares whether each component is `server`, `client`, `shared`
  or `tool`, its ABI and game-version requirements, and every capability it requests; the host
  grants only declared capabilities. Format 1 native packages are the local, pre-registry stage of
  this system and will not be published as official registry mods.
- Gameplay changes require an authoritative server component; client-only code may provide
  presentation and accessibility features but may not determine authoritative outcomes. In
  multiplayer the server evaluates reactions and tool use; a mod must not try to do it client-side.
- A mod must not access the network, spawn processes or touch files outside what the mod API
  offers unless that is its documented central purpose, stated prominently in its README.
  Proprietary analytics, advertising SDKs, launchers, account requirements, undisclosed network
  behaviour, cryptocurrency mining, credential collection and malware are not accepted.

## 4. Playing well with PWC (the mod contract)

These rules come from how the game works; mods that break them break multiplayer, saves or
performance for everyone.

1. **Matter is not a mod.** PWC has no block types: a block is a configuration of elements, and
   one universal law (selective transfer) decides what happens when configurations touch. Mods
   present, place, hold and transform matter through the law; they must not add named
   special-case blocks, alternative physics, or behaviour keyed on a material's name.
2. **Names and looks are presentation.** Names, textures and descriptions a mod produces never
   flow back into the law, world generation, saves or the network protocol.
3. **Determinism.** Anything that affects the world — world generation, reactions a machine
   triggers, persisted state — is a pure function of its inputs (seed, coordinates, the law) and
   bit-identical on every machine: no wall-clock time, unseeded randomness, platform-dependent
   floating point or hash-map iteration order in world-affecting code.
4. **Hooks are not hot paths.** Mod hooks run at frame and event granularity. Never put per-voxel
   work in a hook; cache what you draw.
5. **Respect the mod API's arbitration rules** (fan-out versus first-enabled-wins hooks), keep your
   persisted keys stable, and version persisted payloads.
6. **Disabling must be safe.** Switching a mod off on the Mods screen never corrupts a world or the
   player's holdings; the core owns the stash, so a mod only adds views and actions on it.

## 5. Identity, versions, updates and multiplayer

- **[enforced]** Ids are permanent and namespaced (`<namespace>.<name>`). The `pwc` namespace is
  reserved for first-party packages maintained in this repository. A namespace belongs to whoever
  first published under it; do not publish under someone else's namespace or imitate their names.
- **[enforced]** Versions follow SemVer, and a published version is immutable: the same id and
  version never change contents. `pwc` refuses a locked version whose contents changed without a
  version bump. An update is a new version (and, for the registry, a new reviewed source revision
  and build; a signature never carries over to changed bytes).
- Breaking changes to a package's Rust API, saved state or settings payload require a new major
  version.
- Required mods participate in the multiplayer environment: an instance's `pwc.lock` environment
  hash identifies the PWC source and every package hash. Incompatible clients must be rejected
  before world entry with a specific explanation (the game-side check is future work).

## 6. Registry build and publication (future)

The official registry will build from source rather than accept an author's opaque executable. For
every accepted revision it verifies licences, provenance, dependencies and capabilities; builds in
an isolated environment from pinned inputs; runs tests, ABI and determinism checks; compares clean
rebuilds and rejects unexplained output drift; generates an SPDX SBOM, build log and content
hashes; signs the package; and publishes the package, the exact corresponding source, notices and
build metadata together. The official client installs registry-built, signed packages by default
and shows each mod's source and licence. Registry signing is a statement of provenance, not DRM:
users and forks remain free to use other distribution channels.

## 7. Package quality

- **[enforced]** `mod.toml` has an id, name, version, one-sentence description, authors, licence
  and licence files; mods and libraries declare the `pwc-api` they compile against.
- **[enforced]** A README is included: what the mod does, how to use it (keys, settings), what it
  depends on, what it persists, and anything a user must know before installing (see §3).
- Assets you did not create carry their own attribution in the package (`NOTICE` or
  `assets/ATTRIBUTION.md`).

## 8. Enforcement and appeals

`pwc package check` and `pwc package build` reject packages that fail an enforced rule, and
`pwc mod add` refuses to install them. Maintainers (and later the registry) may reject, quarantine
or remove a package that lacks source, misstates its licence or provenance, violates declared
capabilities, contains malware or cannot be reproduced. Security-sensitive details may be withheld
temporarily during coordinated remediation; corrected releases use the normal licences. Authors
receive a concrete reason and may submit a corrected version or ask for maintainer review (open an
issue in this repository). Trademark permission is separate from acceptance: listing a mod does not
make it an official project release.
