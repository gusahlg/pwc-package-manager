# Dependency resolution

The resolver turns requirements into one exact set of packages (`crates/pwc-resolver`). It is a
pure function: no filesystem, no network, no clock — candidates in, selection or explanation out.

## Input

- the PWC mod API version the build will provide (`pwc-mod-api`, e.g. `1.0.0`);
- the **root requirements**: `id → requirement`, where a requirement is a SemVer `VersionReq`, or
  a *pin* to one specific candidate (from a `path`/`file` instance entry);
- the **candidates**: every known `(id, version)` with its kind, hash, `pwc-api` requirement,
  dependencies (`id → VersionReq`), conflicts (`id → VersionReq`) and an opaque source token;
- optionally the **previous lock** (`id → version`) and the set of ids to **update**.

## Rules

1. At most one version per id.
2. Every root requirement and every dependency of a selected package is satisfied by the selected
   version of that id.
3. A candidate is eligible only if its `pwc-api` requirement accepts the API version (bundles have
   none and are always eligible).
4. No selected package's `[conflicts]` matches another selected package.
5. Pre-release versions are only chosen when a requirement explicitly mentions a pre-release
   (SemVer `VersionReq` semantics).

## Preference

Among valid solutions the resolver prefers, id by id in the order ids are first encountered
(root requirements sorted by id, then dependencies breadth-first in sorted order):

- the version from the previous lock if it is still eligible and the id is not being updated;
- otherwise the highest eligible version.

It explores with depth-first backtracking: choose the preferred version, add its dependencies,
continue; on a contradiction undo the most recent choice and try the next version. The search
is deterministic: the same input always gives the same output.

## Output

On success: the selected packages, sorted by id, each with its dependency ids, plus a
**registration order**: a topological order (dependencies first) with ties broken by id.
Dependency cycles are an error (`"dependency cycle: a → b → a"`).

On failure: a `ResolveError` naming the root cause, for example

```text
no version of `pwc.hotbar` satisfies `^2.0` (required by `pwc.inventory 1.0.0`);
available: 1.0.0, 1.1.0
```

or `"pwc.inventory 1.0.0 requires pwc-api ^2.0 but this PWC provides 1.0.0"`, or
`"foo.a 1.0.0 conflicts with foo.b 2.1.0"`, or `"unknown package `foo.x` (required by the
instance)"`.
