# `mod.toml` — the PWC mod manifest (format 1)

Every mod package has exactly one `mod.toml` at its root. It plays the role `Cargo.toml` plays for
a crate, but it describes a *PWC mod*: identity, authorship, licence, the PWC API it compiles
against, and the other mods it needs. The PWC builder generates the Cargo manifests from it; a
package never ships a `Cargo.toml` (one in a development tree is ignored by packaging).

Implemented by `crates/pwc-manifest` (`ModManifest`). Unknown keys are **errors** (typos must not
silently change meaning); free-form data goes under `[metadata]`.

## Example

```toml
format = 1

[package]
id = "pwc.inventory"
name = "Inventory"
version = "1.0.0"
description = "Your held materials as a list (press I); equip any of them into the hotbar."
authors = ["Project Watt Cubed contributors"]
license = "Apache-2.0 OR MIT"
license-files = ["LICENSES/Apache-2.0.txt", "LICENSES/MIT.txt"]
kind = "mod"
pwc-api = "^1.0"
readme = "README.md"
icon = "assets/icon.svg"
repository = "https://github.com/gusahlg/pwc-package-manager"
keywords = ["inventory", "ui"]
categories = ["interface"]

[dependencies]
"pwc.hotbar" = "^1.0"

[conflicts]
"someone.other-inventory" = "*"

[metadata]
anything = "tools may read this; pwc ignores it"
```

## Top level

| Key | Required | Meaning |
|---|---|---|
| `format` | yes | Manifest format. Must be `1`. A newer format is rejected with "upgrade pwc". |
| `[package]` | yes | Identity and metadata (below). |
| `[dependencies]` | no | Other packages this one needs. |
| `[conflicts]` | no | Packages that must not be in the same build. |
| `[metadata]` | no | Free-form table, ignored by `pwc`. |

## `[package]`

| Key | Required | Rule |
|---|---|---|
| `id` | yes | Permanent, namespaced id: two or more dot-separated segments, each `[a-z0-9][a-z0-9-]*`, total ≤ 64 chars, no `--`, no trailing `-`. The first segment is the **namespace** (a person or organisation). `pwc` is reserved for first-party packages. The id never changes; the display name may. |
| `name` | yes | Display name, 1–64 chars, no control characters. |
| `version` | yes | [SemVer 2.0](https://semver.org) version (`1.4.2`, `2.0.0-beta.1`). Build metadata (`+…`) is not allowed. |
| `description` | yes | One sentence, 1–280 chars, no newlines. |
| `authors` | yes | Non-empty list of `"Name"` or `"Name <email>"`. |
| `license` | yes | SPDX licence expression ([POLICY.md](../../POLICY.md#1-licensing) §1): licence ids with `AND`, `OR`, `WITH <exception>` and parentheses; ids are case-sensitive. Every id must be on the policy's allowlists of free licences (software licences compatible with `AGPL-3.0-or-later`, such as `Apache-2.0`, `MIT`, `MPL-2.0`, `GPL-3.0-or-later` and `AGPL-3.0-or-later`; content licences for assets and documentation `CC-BY-SA-4.0`, `CC-BY-4.0`, `CC0-1.0`, `OFL-1.1`, `LAL-1.3`), and every exception on its list (`LLVM-exception`, `Classpath-exception-2.0`, `GCC-exception-3.1`). For `mod` and `library` packages every `OR` alternative must contain a software licence (`Apache-2.0 OR MIT` and `MIT AND CC-BY-4.0` pass; `CC-BY-SA-4.0` and `MIT OR CC-BY-4.0` do not); a `bundle` may use any allowlisted expression. Unknown or unlisted ids, a trailing `+`, `LicenseRef-*`, `DocumentRef-*`, `NOASSERTION` and `NONE` are rejected: **proprietary mods are not allowed**. First-party packages use `Apache-2.0 OR MIT`. |
| `license-files` | yes | Non-empty list of package-relative paths of the full licence texts. Each must exist in the package. |
| `kind` | no | `"mod"` (default): registers mods into the game. `"library"`: Rust code other packages use; registers nothing. `"bundle"`: no code at all, only `[dependencies]` (a curated set such as `pwc.essentials`). |
| `pwc-api` | mod, library | SemVer requirement on the `pwc-mod-api` crate version (`"^1.0"`). Bundles must omit it. |
| `edition` | no | Rust edition of the package's code: `"2021"` or `"2024"` (default `"2024"`). |
| `readme` | no | Default `"README.md"`. Must exist. |
| `icon` | no | Package-relative path under `assets/`, `.svg` or `.png`. |
| `repository`, `homepage`, `documentation` | no | `https://` URLs. |
| `keywords` | no | ≤ 5 entries, each `[a-z0-9-]{1,24}`. |
| `categories` | no | Subset of: `gameplay`, `interface`, `visuals`, `worldgen`, `audio`, `library`, `tools`, `bundle`. |

## `[dependencies]` and `[conflicts]`

Keys are package ids. Values are a SemVer requirement string (`"^1.0"`, `">=2, <3"`, `"=1.4.2"`,
`"*"`), or for dependencies a table `{ version = "^1.0" }` (reserved for future keys). Rules:

- A package may not depend on itself or list the same id in both tables.
- A **bundle** must have at least one dependency. Depending on a bundle is allowed: it pulls in the
  bundle's dependencies (as dependencies of the bundle, not of the dependent), and the bundle itself
  provides no crate.
- Every dependency is resolved to exactly one version per build (one version of an id per
  build — the runtime registers mods by id), registered *before* the dependent, and available
  to the dependent's Rust code as the crate named by the dependency's crate name.
- `[conflicts]` makes the resolver reject any build containing a matching version.

## Crate name and entry point

The Rust crate name of a package is its id with `.` and `-` replaced by `_` (`pwc.material-names` →
`pwc_material_names`). Dependents write `use pwc_hotbar::HotbarHandle;`.

- `kind = "mod"`: `src/lib.rs` must define
  `pub fn register(registrar: &mut pwc_mod_api::ModRegistrar)`. The builder calls it once at
  startup, after every dependency registered. `register` installs `Mod` values
  (`registrar.add(...)`), may declare groups and may `provide`/`get` shared handles.
- `kind = "library"`: `src/lib.rs`, no `register`.
- `kind = "bundle"`: no `src/` directory.

Every package's code may use `pwc_mod_api` (always linked) and its own dependencies. Format 1 does
not allow crates.io dependencies, `build.rs` or proc-macro crates (see
[POLICY.md, section 2](../../POLICY.md#2-complete-source)).

## Package contents

Only these paths are part of a package; everything else in the directory — a development
`Cargo.toml`, `target/`, `dist/`, `.git/`, editor files — is not part of it and is ignored by
`pwc package build` (and listed by `pwc package build --verbose`):

```text
mod.toml            required
README.md           required (or the `readme` path)
license files       required (every `license-files` entry)
CHANGELOG.md        optional
NOTICE              optional
src/**              required for mod/library, forbidden for bundle
assets/**           optional (embed with include_bytes!/include_str!)
data/**             optional
```

Forbidden inside the included paths (an error, not ignored): `build.rs`, `Cargo.toml`,
`Cargo.lock`, symlinks, hidden files or directories (starting with `.`), files over 32 MiB, more
than 4096 files, more than 256 MiB in total.

## Development workflow

A mod is developed as an ordinary directory and added to an instance as a mutable `path` source
(`pwc mod add ./my-mod`), so every `pwc build` compiles the current tree. The builder compiles a
store snapshot of that tree, so compiler messages name store paths; the file names below
`<store>/<hash>/` mirror the source tree one to one. A development `Cargo.toml` (for an IDE) may
sit next to `mod.toml`; packaging ignores it.
