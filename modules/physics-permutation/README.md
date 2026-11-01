<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Matter Permutations

This experiment changes the question from "which constituents fit better together?" to
"can a very small conservative permutation law create useful matter motion?" Whole configurations
move; their constituents are never rewritten and no named material behavior is added.

Install `pwc module add pwc.physics-permutation`. Enable `pwc.physics-permutation.enabled` in
Settings, disable competing model providers, and use a separate instance. It defaults off.
Ctrl+F5 activates a closed 7×7×7 domain around the player and moves the **existing** matter there.
It creates no matter. The HUD reports swaps, geometric phase and refused operations. Player
collision/gravity continue to use the common world physics; swapped matter changes ordinary cells.

## State and law

State is only domain center, turn, swap count and refusal count; actual material is stored by the
core world. Six deterministic geometric phases choose disjoint face-neighbor matchings:
+x even/odd relative coordinate, then +y even/odd, then +z even/odd. Outer unmatched cells stay put.
For configurations A,B let `m(A)` be the sum of their element coordinate bytes. Swap A,B when

```text
(popcount(m(A) XOR m(B)) + |A| |B|) module 2 = 1.
```

The predicate is symmetric, unchanged by swapping its inputs, and nonlinear in composition.
Therefore each admitted matching is its own inverse. Reverse the six phase order to reverse an
ideal fixed-domain run. The theorem is about whole stored configurations under full admission;
a storage refusal can prevent the intended phase inverse, so live-host reversibility is conditional.
Each pair commit is atomic, and a refused pair leaves both cells unchanged. Other admitted pairs
in that phase may proceed; the phase is not a whole-world transaction.

## Emergence and limitations

Collisions can switch whether configurations exchange places without naming a particle, wall,
wire or machine. Composition changes the predicate continuously only in the loose sense of
algebraic dependence: the bit-parity law is intentionally discontinuous and may be hard for players
to infer. The six phase clock is a fundamental update schedule, not a discovered oscillator.
Repeated local bijections can support persistent trajectories, but no logic gates, universal
computation, fluids, realistic momentum or thermodynamic energy are established here.

The one demonstration runs six phases with one occurrence, reverses their order and recovers
exactly the same sparse map. It also checks complete model-state save/load. The interesting next
experiment needs multiple compositions and interactive collisions, not a claim that one moving
cell validates the fictional universe.

The finite domain is a workbench boundary; no whole-world scan or unbounded activation occurs.
There are 147 pairs per phase at this domain size, so the 384 unit default quota covers a whole phase.
Cost includes 343 reads plus repeated moment sums; future implementations could cache immutable
moments per configuration. Raw chart aliases are refused. Material edits replicate through core,
but the input/domain instrumentation is singleplayer and no experimental network controls exist.
Pausing retains the domain/phase. Saving its model payload alone does not save matter; the core
world overlay must accompany it. Inputs perform activation, not external work on an energy ledger.

## Inspiration and development

[Margolus, Crystalline Computation](https://arxiv.org/abs/comp-gas/9811002) motivates partitioned
reversible dynamics, not this particular swap predicate or its usefulness. The nonlinear predicate
and matter-valued pairing are our proposed toy rules. No paper code is copied.

`module.toml` is canonical; use a mutable path entry in a disposable instance and
`pwc build --profile dev --print-dir`. In the printed generated workspace run
`cargo test -p pwc_physics_permutation --lib -- --nocapture`. No Cargo manifest ships in this package.

## Licence

Apache License 2.0 or MIT, at your option. Both complete texts are in `LICENSES/`.
