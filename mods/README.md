# First-party packages

This directory is the first-party package repository: the PWC mods maintained in this repository
under the reserved `pwc` namespace. It is an ordinary `pwc` repository
([spec/filesystem.md](../docs/spec/filesystem.md#repositories)), the local stand-in for the future
registry.

## Layout

Each subdirectory is the source tree of one package, named after its id without the `pwc.` prefix
(`hotbar/` is `pwc.hotbar`). Other entries, such as this README, are ignored by `pwc`.

```text
hotbar/
├── mod.toml          the manifest (docs/spec/mod-manifest.md)
├── README.md         what the mod does, keys and settings, dependencies, what it persists
├── CHANGELOG.md
├── LICENSES/
│   ├── Apache-2.0.txt
│   └── MIT.txt
├── assets/           files embedded with include_bytes!/include_str!
└── src/
    └── lib.rs        pub fn register(registrar: &mut pwc_mod_api::ModRegistrar)
```

## Licensing

Each package here is licensed `Apache-2.0 OR MIT`, at your option: code, manifest, README,
changelog and assets alike. Mods may use any licence the [mod policy](../POLICY.md#1-licensing)
allows; the first-party packages use the dual licence it recommends. Each package carries both
licence texts in its own `LICENSES/` directory, and [REUSE.toml](../REUSE.toml) records the same
licence for every file in a package directory. This README is part of the package manager's
documentation and is licensed `AGPL-3.0-or-later`, like the rest of the tooling.

The packages are listed in the [repository README](../README.md#first-party-packages).

## Using this repository

`pwc setup` registers this directory as the repository `first-party` by default. To add it by
hand, or to a configuration written without it:

```sh
pwc repo add <path-to-pwc-package-manager>/mods
```

`pwc mod add pwc.essentials` then finds the packages here. `pwc` packages a selected source
directory on the fly, with the same hash `pwc package build` would produce, and copies it into the
store; the build always uses the store copy.

To work on one of these packages, add its directory to a test instance as a local path entry, so
that edits are picked up on the next `pwc run`:

```sh
pwc mod add ./mods/hotbar
```

## Changing a package

A version that has reached `main` is published and immutable: any change to a package needs a new
`version` in its `mod.toml`, and `pwc` refuses to re-lock a version whose contents changed. CI runs
`pwc package check` on every directory here. See
[CONTRIBUTING.md](../CONTRIBUTING.md#first-party-mod-packages) for the full procedure, and
[docs/writing-a-mod.md](../docs/writing-a-mod.md) for how mods are written.
