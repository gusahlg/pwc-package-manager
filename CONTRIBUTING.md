# Contributing

Thank you for helping improve the PWC package manager. This document covers the development
setup, the checks a change must pass, and how to change the first-party mod packages.

## Development setup

**With Nix**, enter the development shell from this repository's flake. It provides `rustc`,
`cargo`, `rustfmt`, `clippy`, `git` and `reuse`:

```sh
nix develop
```

**Without Nix**, install Rust 1.90 or newer (for example with rustup) with the `rustfmt` and
`clippy` components, and the [REUSE tool](https://reuse.software) (`pipx install reuse`).

The tooling is pure Rust: no C compiler or system libraries are needed. Unit and integration tests
do not need the game. For end-to-end work (`pwc build`, `pwc run`) you also need a checkout of
[project_watt_cubed](https://github.com/gusahlg/project_watt_cubed), by default next to this
repository, and whatever the game needs to build. When `nix` is available, `pwc` runs Cargo inside
the game's own dev shell, so building from inside this repository's shell works too. Use
`PWC_HOME=<dir>` to keep experiments out of your real store, instances and cache.

## Checks

Every change must pass what CI runs:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
for dir in mods/*/; do cargo run -q --locked -p pwc-cli -- package check "$dir"; done
reuse lint
```

Run `cargo fmt --all` to fix formatting. The workspace lints forbid `unsafe` code and warn on
missing documentation, and clippy treats warnings as errors, so every public item needs a doc
comment. `reuse lint` checks that every file has a known licence ([REUSE.toml](REUSE.toml)); a new
file is covered automatically when it falls under an existing annotation.

## Guidelines for the tooling

- **One responsibility per crate** (see [docs/architecture.md](docs/architecture.md)). `pwc-cli`
  stays a thin front end: logic belongs in the library crates, where a launcher can reuse it.
- **The specifications are normative.** A change to a format or to observable behaviour updates
  the matching file in [`docs/spec/`](docs/spec/) in the same pull request. An incompatible change
  to a file format needs a new format number, and the old format stays readable.
- **Determinism.** Package bytes, hashes, lockfiles, the resolver's output and Build IDs must not
  depend on the machine, the clock, the locale or hash-map iteration order. Test with fixed inputs
  and compare exact output.
- **Pure-Rust dependencies only**, added to `[workspace.dependencies]` in the root `Cargo.toml`.
  Prefer the standard library and the crates already in use.
- **Errors explain themselves.** A failure names what was being done and why it failed, in a form
  `pwc` can print as `error:` with `cause:` lines.

## First-party mod packages

The packages in [`mods/`](mods/) are a package repository that users consume directly
(`pwc setup` adds it by default), so a version that has reached `main` is published.

When you change a package:

1. **Bump its `version` in `mod.toml`.** Published versions are immutable: any change to the
   package contents, including the README, assets and licence files, needs a new version, and `pwc`
   refuses to re-lock a version whose contents changed. Follow SemVer. Breaking changes to the package's Rust API, persisted state or settings payload need a
   new major version.
2. Update requirements that should move with it, such as the ranges in the `pwc.essentials` bundle
   after a major bump.
3. Record the change in the package's `CHANGELOG.md`.
4. Run `pwc package check mods/<package>`, then try it in a test instance with
   `pwc mod add ./mods/<package>` and `pwc run`.

When you add a package:

- Give it an id in the reserved namespace, `pwc.<name>`, and a directory named `<name>`.
- License it `license = "Apache-2.0 OR MIT"` with
  `license-files = ["LICENSES/Apache-2.0.txt", "LICENSES/MIT.txt"]`, copying both texts from the
  repository's [LICENSES/](LICENSES/) into the package's own `LICENSES/` directory, and use the
  licence section of an existing package's README. [REUSE.toml](REUSE.toml) already marks every
  file in a package directory as `Apache-2.0 OR MIT`.
- Include a README written for players: what it does, keys and settings, dependencies.
- Follow the mod contract in [POLICY.md](POLICY.md#4-playing-well-with-pwc-the-mod-contract).
- Add it to the package table in [README.md](README.md), and to `pwc.essentials` if it belongs in
  the default set.

Changes to the licence allowlists go through a pull request that changes [POLICY.md](POLICY.md)
and `crates/pwc-manifest/src/license.rs` together. Only free licences can be added, and a software
licence or exception must be compatible with `AGPL-3.0-or-later` (POLICY.md §1 explains why).

## Commits and pull requests

Keep commits focused and describe what changed and why.

**Every commit must carry a [Developer Certificate of Origin 1.1](https://developercertificate.org/)
sign-off**, as the game's governance requires of its own contributions (principle 4 of the game's
`GOVERNANCE.md`). The sign-off is a line at the end of the commit message with your name and email
address:

```text
Signed-off-by: Your Name <you@example.org>
```

`git commit -s` (`--signoff`) adds it from your `user.name` and `user.email`. To sign off commits
you have already made on a branch, run `git rebase --signoff main`. By signing off you certify the
statements of the DCO: in short, that you wrote the contribution or otherwise have the right to
submit it under the licence that applies to it (below). Do not sign off on someone else's behalf.
Pull requests with commits that lack a sign-off are not merged.

## Licence of contributions

Contributions take the licence of the part of the repository they change (inbound = outbound):

- **First-party packages** (`mods/<package>/`) are licensed under either of the Apache License,
  Version 2.0 or the MIT licence, at your option (`Apache-2.0 OR MIT`). Unless you explicitly state
  otherwise, any contribution intentionally submitted for inclusion in a first-party package by
  you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without any
  additional terms or conditions.
- **The package manager** (the tooling, specifications, documentation and infrastructure:
  everything except the package directories under `mods/`) is licensed `AGPL-3.0-or-later`, the
  licence in [LICENSE](LICENSE). Contributions to it are licensed `AGPL-3.0-or-later` as well.

You keep your copyright; there is no copyright assignment or contributor licence agreement. The
DCO sign-off records that you have the right to make the contribution under these terms.
