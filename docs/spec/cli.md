# The `pwc` command

`pwc` is the command-line front end (`crates/pwc-cli`). It contains no package logic of its own:
every command is a thin call into the library crates, so a graphical launcher can offer the same
operations by calling the same functions.

Global options: `--instance <name>` (default: the active instance), `-v/--verbose`, `-q/--quiet`.
Exit status `0` on success, `1` on a reported error, `2` on a usage error. Errors print one line
`error: <what>` followed by indented `cause:` lines.

| Command | Does |
|---|---|
| `pwc setup [--pwc <dir>] [--repository <dir>]…` | Write `config.toml` (see filesystem.md). PWC source: `--pwc`, else the sibling checkout, else the existing value. `--repository` (repeatable) replaces the list; without it existing repositories are kept and `first-party` is added if missing. |
| `pwc config` | Print the effective configuration and directories. |
| `pwc repo add <dir> [--name <n>]` / `repo remove <name>` / `repo list` | Manage repositories. |
| `pwc search [<text>]` | List packages available from repositories and the store: id, newest version, kind, description. |
| `pwc info <id>` | Every known version of a package with its metadata and dependencies. |
| `pwc package check [<dir>]` | Validate a source directory: manifest, licence policy, contents. |
| `pwc package build [<dir>] [-o <out-dir>]` | Write `<out-dir>/<id>-<version>.pwcmod` (default `<dir>/dist/`) and print its hash. |
| `pwc package verify <file> [--hash <sha256:…>]` | Fully verify a `.pwcmod`. |
| `pwc package inspect <file>` | Print the manifest summary, hash and file list. |
| `pwc instance create <name> [--pwc <req>] [--use]` | New empty instance. |
| `pwc instance list` | Instances, marking the active one. |
| `pwc instance use <name>` | Make it the active instance. |
| `pwc instance show [<name>]` | Its `instance.toml` requirements and locked packages. |
| `pwc instance remove <name> [--yes]` | Delete an instance (store entries remain until `store gc`). Without `--yes` it asks on a terminal and refuses otherwise. |
| `pwc mod add <spec>…` | Add mods (id, `id@req`, source dir, or `.pwcmod`) and re-lock. |
| `pwc mod remove <id>…` | Remove mods and re-lock. |
| `pwc mod update [<id>…]` | Re-resolve preferring the newest compatible versions. |
| `pwc mod list` | The locked packages, marking direct requirements. |
| `pwc lock` | Re-resolve conservatively and write `pwc.lock`. |
| `pwc env` | Print the environment hash of the existing lock (no re-locking; an error if there is none). |
| `pwc build [--profile release\|dev] [--force] [--print-dir]` | Build the instance (re-locking conservatively first); print the executable path, or the build directory with `--print-dir`. |
| `pwc run [--golden] [--profile release\|dev] [-- <args>…]` | Build if needed, then run the game (or the golden harness) behind the build wrapper. |
| `pwc store list` / `store verify` / `store gc [--dry-run]` | Inspect and maintain the store (`gc` refuses to run if any instance's lock is unreadable). |
| `pwc cache clean` | Delete build outputs and the shared target directory. |

The first end-to-end milestone:

```bash
pwc setup
pwc instance create default --use
pwc mod add pwc.essentials
pwc run
```
