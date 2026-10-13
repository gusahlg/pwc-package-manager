# Law Lab (`pwc.law-lab`)

A window onto other universes. The lab runs the **candidate fundamental laws**
(`pwc.law-demon`, `pwc.law-belousov`, `pwc.law-toda`, `pwc.law-margolus`, `pwc.law-zeldovich` and
`pwc.law-polyakov`) on the real terrain around you. You can see how heat, excitation, vibration,
particles or fields would behave in the matter of your world under each law, before any law
replaces the game's own.

`/law <id>` copies a box of the world centred on your feet into the law's own state. That is the
same configurations the world holds, nothing renamed. The lab then steps the law and draws the
slice you stand on as a heat map. The lab **never writes a cell**: the world, its own reaction
law and every other mod carry on untouched. Recapture with `/law <id>` after you change the
world.

Mod id: `law_lab`. It depends on `pwc.commands` for its commands. Add it with
`pwc mod add pwc.law-lab`; the law libraries come with it.

## Controls and commands

| Input | Does |
|---|---|
| `/law` | List the candidate laws with a one-line summary each, and the running one's status. Also `/lab`. |
| `/law <id> [side]` | Capture a `side`³ box (default 32, at most 64) around you and run the law `<id>` on it. |
| `/poke [strength]` or `P` | Act on the cell you look at (within 12 blocks; else the box centre) in the law's own terms: add heat, excite, kick, emit a particle or deposit charge. The strength persists. |
| `/law rewind [n]` | Run a reversible law backwards `n` ticks (default 20), **exactly**, then pause. Laws that forget refuse. |
| `/law pause`, `/law speed <n>` | Pause/resume; run `n` ticks per mod tick (0..64; the lab never spends more than 4 ms a tick). |
| `/law depth <n>` | Move the shown slice along your up axis (0 = the ground under your feet). |
| `/law ledger` | Print the law's books: its conserved totals and counters. |
| `/law off` | Stop and forget the run. |

The HUD shows matter at rest in grey and activity from dark red to white. Your column is
outlined, and the law's books are listed underneath.

## What it persists

Nothing. A run lives only for the session and ends when you leave the world.

## The laws

The design brief, the shared interface and every candidate's README (inspiration, rules, measured
phenomena, failures, cost and verdict) are in the `pwc.law-*` packages and in the repository's
`physics-lab/` workbench.
