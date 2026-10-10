# Filesystem layout, configuration, store and repositories

Installed PWC state lives outside every source repository. On Unix it follows the XDG base
directory conventions (environment variable, else the default below). On Windows, when neither
`HOME` nor the corresponding absolute XDG variable is set, it uses
`%LOCALAPPDATA%\pwc\{data,cache,config}`; `%USERPROFILE%\AppData\Local` is the fallback when
`LOCALAPPDATA` is unavailable. `PWC_HOME=<dir>` overrides all three roots on every platform
(`<dir>/data`, `<dir>/cache`, `<dir>/config`) — used by tests and portable setups.

```text
$XDG_DATA_HOME/pwc/          ~/.local/share/pwc/   persistent: survives cache deletion
├── store/<hex>/              immutable unpacked packages, content-addressed
└── instances/<name>/         instance.toml, pwc.lock, game/

$XDG_CACHE_HOME/pwc/          ~/.cache/pwc/          disposable: everything can be rebuilt
├── builds/<build-id>/        generated Cargo project, metadata, final executables
└── target/<profile>/…        shared Cargo target directory (crate-level reuse across builds)

$XDG_CONFIG_HOME/pwc/         ~/.config/pwc/
└── config.toml               tooling preferences
```

Implemented by `crates/pwc-instance` (`Dirs`, `Config`, repositories) and `crates/pwc-store`.

## `config.toml`

```toml
format = 1
pwc-source = "/home/alice/src/project_watt_cubed"       # the PWC game checkout to build against
active-instance = "survival"                             # set by `pwc instance use`

[[repository]]
name = "first-party"
path = "/home/alice/src/pwc-package-manager/mods"

[build]
# Command prefix for Cargo. "auto" (default): if the PWC source has a flake.nix and `nix` is on
# PATH, run `nix develop <pwc-source> --command` (nested shells are fine; the game's shell provides
# its native libraries). `PWC_NO_WRAPPER=1` disables it. [] runs cargo directly; any other list is
# used verbatim ("{pwc}" expands to pwc-source).
wrapper = "auto"
jobs = 0                                                  # 0 = Cargo's default
```

`pwc setup [--pwc <dir>] [--repository <dir>]` writes it. Defaults (computed from where the `pwc`
binary was built): without `--pwc`, a `project_watt_cubed` checkout next to the tooling
repository; without `--repository`, the tooling repository's `mods/` directory, registered as
`first-party`. Setup prints what it configured.

## The store

`store/<hex>/` holds the files of one package exactly as in its canonical tar, where `<hex>` is the
package hash without the `sha256:` prefix. Rules:

- Insertion is atomic: unpack into `store/.tmp-<random>/`, verify, then rename into place. If the
  target already exists the new copy is discarded (same hash ⇒ same contents).
- Entries are read-only after insertion (files `0444`, directories `0555`).
- `pwc store verify` re-hashes every entry and reports corruption.
- `pwc store gc` deletes entries no instance's `pwc.lock` references (`--dry-run` lists them).
- `pwc store list` prints `id version hash` for every entry.

## Repositories

A repository is a local directory whose immediate children are package **source directories**
(containing `mod.toml`) or **`.pwcmod` files**; any other child (a `README.md`, a directory without
`mod.toml`, hidden files) is ignored. An entry that fails validation is skipped with a warning. Repositories are the offline stand-in for the future
remote registry: `pwc mod add pwc.essentials` searches every configured repository and the store,
picks versions with the resolver, and packages selected source directories on the fly (the hash
is the same as `pwc package build` would produce). This repository's own `mods/` directory is the
first-party repository.

## Game data per instance

`pwc run` starts the instance's executable with:

| Variable | Value |
|---|---|
| `WATT_DATA_DIR` | `$XDG_DATA_HOME/pwc/instances/<name>/game` (worlds, `settings.cfg` with the packages' options) |
| `WATT_ASSET_DIR` | `<pwc-source>/assets` |
| `PWC_INSTANCE` | `<name>` |
| `PWC_ENVIRONMENT` | the lock's environment hash |
