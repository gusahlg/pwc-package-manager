# Changelog

Notable changes to the `pwc` tooling and this repository. Each first-party package in `mods/`
carries its own version in its `mod.toml`.

## 0.1.0 — unreleased

The first release: local packages, instances and reproducible builds of modded PWC.

### Specifications (format 1)

- `mod.toml`, the mod manifest: namespaced permanent ids, SemVer versions, SPDX licences, the
  `pwc-api` requirement, dependencies and conflicts, and the `mod`, `library` and `bundle` kinds.
- `.pwcmod`, the package format: a Zstandard-compressed canonical tar identified by the SHA-256 of
  the tar.
- `instance.toml` and `pwc.lock`, with the environment hash and the immutability rule (re-locking
  refuses a version whose contents changed without a version bump).
- The filesystem layout: XDG data, cache and config roots, `PWC_HOME`, `config.toml`, the
  content-addressed store and local repositories.
- Dependency resolution: one version per id, deterministic backtracking, explanations on failure.
- The builder: generated Cargo workspace, mod bundle, Build IDs and the build cache.
- The `pwc` command-line interface.
- [POLICY.md](POLICY.md), mod policy version 1, based on the game's official mod policy: free
  software only, proprietary mods not allowed; SPDX licence expressions over an allowlist of free
  software licences compatible with `AGPL-3.0-or-later` (mods are compiled into the AGPL game),
  free-content licences for assets and documentation, and three `WITH` exceptions; code packages
  carry a software licence on every `OR` branch; `Apache-2.0 OR MIT` recommended. Where the game's
  `MOD_POLICY.md` originally required all mod code to be `AGPL-3.0-or-later`, this policy accepts
  AGPL-compatible free licences; the game's `MOD_POLICY.md` and `GOVERNANCE.md` are being updated
  to match. No `build.rs`, proc macros or crates.io dependencies in packages; format 1 native
  packages as the local stage before the sandboxed WebAssembly registry.

### Tooling

- `pwc-manifest`: the manifest, instance and lockfile formats with validation and the licence
  policy.
- `pwc-package`: packaging, deterministic archives, hashing and verification.
- `pwc-store`: the immutable package store, with verification and garbage collection.
- `pwc-resolver`: the dependency resolver.
- `pwc-instance`: configuration, repositories and instances; locking.
- `pwc-builder`: building an exact lock into a native executable, with a build cache.
- `pwc-cli`: the `pwc` command (`setup`, `repo`, `search`, `info`, `package`, `instance`, `mod`,
  `lock`, `env`, `build`, `run`, `store`, `cache`).

### First-party packages

- `pwc.menus`, `pwc.start-screen`, `pwc.hotbar`, `pwc.inventory`, `pwc.visuals`,
  `pwc.neural-textures`, `pwc.material-names` and `pwc.infinite-diffusion`, ported from the mods
  that were previously built into the game.
- `pwc.essentials`, a bundle of all of them.
- All first-party packages are licensed `Apache-2.0 OR MIT`.
- `mods/` serves as the first local package repository.

### Infrastructure

- Nix development shell (`flake.nix`).
- REUSE compliance: `LICENSES/` and `REUSE.toml`. The tooling and documentation are licensed
  `AGPL-3.0-or-later` ([LICENSE](LICENSE)); the first-party packages `Apache-2.0 OR MIT`.
- Continuous integration: formatting, clippy, tests (with `--locked`), `reuse lint` and
  `pwc package check` for every first-party package.
- Contributions require a Developer Certificate of Origin 1.1 sign-off
  ([CONTRIBUTING.md](CONTRIBUTING.md)).
