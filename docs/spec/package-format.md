# `.pwcmod` — the PWC mod package (format 1)

A `.pwcmod` file is one **immutable** version of one mod: its canonical source, assets, licence
and manifest. It is a distribution artifact, not a development format — authors work in an
ordinary directory and run `pwc package build`.

Implemented by `crates/pwc-package`.

## File name

`<id>-<version>.pwcmod`, e.g. `pwc.inventory-1.0.0.pwcmod`.

## Container

A `.pwcmod` is a **Zstandard frame** whose decompressed content is a **canonical tar** stream.

The canonical tar is fully determined by the package's files, so the same files always produce
the same bytes on every machine:

- Only regular files, no directory entries. Entries are sorted by path, bytewise ascending, using
  `/` separators and package-relative paths (`mod.toml`, `src/lib.rs`, `assets/icon.svg`).
- Every header: POSIX `ustar` format, mode `0644`, uid/gid `0`, empty user/group names,
  mtime `0`. Paths longer than the ustar limit use a PAX `path` extended header, itself written
  deterministically.
- The stream ends with the two zero blocks tar requires.
- Paths must be valid UTF-8, relative, at most 1024 bytes, with `/` separators only. No leading
  `/`, no `\`, no empty, `.` or `..` components, no `:` in a component (so no `C:`, `C:/...` or
  `file:stream`), no `< > " | ? *`, no control characters, and no component ending in a dot or a
  space. A component whose text before the first `.` is a Windows reserved device name (`CON`,
  `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`, any ASCII case, so `con.txt` and `nul.tar.gz`
  too) is rejected. These rules apply on every platform. Two paths that differ only by case, or a
  file whose path is a directory prefix of another path ignoring case, are rejected before
  anything is written. Paths must also be part of the package contents defined in
  [mod-manifest.md](mod-manifest.md#package-contents).

The compression level is not part of the format: readers must accept any valid Zstandard frame.

## Package hash

The package's identity is

```text
sha256:<64 lowercase hex digits of SHA-256(canonical tar bytes)>
```

computed over the **decompressed** canonical tar. The same source directory therefore hashes the
same whether it is packed, re-compressed, or packaged on the fly from a repository directory, and
the store, the lockfile and the registry all use this one hash.

## Reading and verification

A reader must, in order:

1. Decompress; reject anything that is not a single Zstandard frame.
2. Parse the tar; reject non-file entries, unsorted or duplicate paths, unsafe paths, non-zero
   mtimes/uids, and any header not produced by the canonical writer (re-serialise and compare is
   an acceptable implementation).
3. Find `mod.toml`, parse and validate it (including the licence policy).
4. Check the contents rules (required files present, nothing forbidden, size limits).
5. Compute the hash. If the caller expected a hash (lockfile, registry), compare it.

Unpacking appends each path component on its own and refuses a result that is not inside the
destination directory. A drive-qualified path such as `C:/x` is rejected by the path rule above
and again at extraction, because joining it would replace the destination on Windows.

Any failure rejects the whole package; nothing is partially installed.
