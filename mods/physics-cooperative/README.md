<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Cooperative Chemistry

A mathematically small alternative to selective transfer: neighboring configurations may move
one occurrence **or the smallest nontrivial cluster, two occurrences**, when that move increases
summed internal fit. No element has a name or an authored reaction recipe.

Install `pwc mod add pwc.physics-cooperative` after registering this package repository. In
Settings, enable `pwc.physics-cooperative.enabled` and disable other physics providers. It defaults
off. Use a separate instance for comparisons. Place/move matter normally; Ctrl+F5 wakes the six
contacts around your current cell without supplying matter or energy. The HUD reports queued
contacts, changes and two-occurrence moves. There is no held-tool reaction override.

## State and law

Only a bounded deterministic contact queue and counters are stored. Let `Phi` be the sum of
unordered internal pair fits over all cells. For donor elements i,j and receiver B, compute each
single-transfer gain `g_i = sum_B J(i,b) - sum_{k != i} J(i,k)`. The double-transfer gain is

```text
g_ij = g_i + g_j + 2 J(i,j).
```

Accept the greatest strictly positive gain among both orientations and all singles/pairs,
respecting the shared 32-occurrence capacity; break ties by canonical iteration order. Both
products are atomically admitted before any matter changes. A refused write/intern retries next
turn. A successful operation wakes both neighborhoods on the next turn. A no-op contact sleeps.
The original size-dependent threshold and swaps are absent; this is a distinct law, not a tuning
flag silently changing selective transfer.

Internal/cross supports are computed once per contact. Work is O(C²) fits and candidates for
C<=32; the queue is limited to 16,384 contacts and each tick uses the host contact budget. These are
explicit laboratory bounds. Refused new wakeups count as dropped and may remain asleep until a
later event. Registry churn and intern exhaustion still need measurements.

## What is possible, and what remains impossible

A cohesive pair can move together even when either individual move loses fit: two parallel pairs `{(0,0),(64,0)} | {(0,88),(64,88)}` on two resource axes
are an exact example (other coordinates zero). Our test verifies that every single move in both directions
loses fit, the selected double move gains fit, and every occurrence is retained. This creates
cooperative transformations without a table of specific catalysts or recipes.

**This remains strict bounded compatibility ascent.** A finite closed system still settles and
cannot supply persistent clocks. The changed move neighborhood overcomes some local traps; it
does not overcome the main monotonicity limitation. Bigger clusters and capacity swaps are future
alternatives, not hidden cases in this law. Two is a deliberately minimal cluster cutoff.

Saves contain arrival turns and counters, validated transactionally with a stable payload version.
Dynamic model state uses core simulation records; material configurations stay in the usual world
save. Pausing preserves it. Chart-crossing contacts are refused until a canonical physical port
map exists. Input is singleplayer; no auxiliary field replication is claimed. The law creates no
new block types, accesses no files/network/processes outside the API and changes no player forces.

## Development

`mod.toml` is canonical. Use a mutable path entry in a disposable manager instance, then
`pwc build --profile dev --print-dir`. From that generated workspace run
`cargo test -p pwc_physics_cooperative --lib -- --nocapture`. No Cargo manifest is shipped here.
The gain equation is our derivation from PWC's pair potential; its small demonstration is not
proof of gameplay, stable natural palettes or scalable chemistry.

## Licence

Apache License 2.0 or MIT, at your option. Both complete texts are in `LICENSES/`.
