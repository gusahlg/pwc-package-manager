<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Exploraswarm (`pwc.exploraswarm`)

Exploraswarm connects, over the network, to the multiplayer server you name. It joins that server as N ordinary players and writes its archive and logs under the instance's data directory. That is the point of the module: the agents are real headless clients, and everything they learn is files in the instance data directory (`<data_dir>/exploraswarm/<rules>/` unless you pass `--run-dir`).

Each player is a small neural network. It sees what a player can see, picks one of the actions a player has, and the swarm keeps the networks that found different ways to play. The aim is to walk the game quickly and surface interactions nobody has tried yet. What counts as interesting is a rule set. The machinery that trains and stores the networks does not know.

## What it does

A run is MAP-Elites. The behaviour space is a grid. Each cell holds at most one network, the elite, scored by a stationary fitness. New networks are variations of elites. A variation replaces the elite only when it is clearly better, and a noisy score has to win twice before it takes the cell. Rules decide the axes, the fitness and a separate novelty bonus. Swapping the rule set changes what the swarm searches for, and changes nothing else.

## Quick start

Add the package to an instance, start a dedicated server, then point the module at it:

```sh
pwc module add ./modules/exploraswarm
watt_server --port 5555
pwc run -- swarm run --server 127.0.0.1:5555 --agents 8
```

`watt_server` is the game's headless dedicated server (`watt_server --port 5555 --password …` when the server is not open). It accepts at most 256 players. Every agent is one of those players, so `--agents` plus anyone already on the server has to fit under that cap. Names must be unique. Agents are named `<name>01`, `<name>02`, … (`swarm01` by default) and the full name is kept to 16 characters.

The command above is a client that has this module compiled in. It does not open a window for the agents. Watch them by joining the same server yourself.

While a run is going, type `pause`, `resume`, `status`, `save`, `agents N` or `quit` on stdin, or write the same line to `<run-dir>/control`. There is no signal handler. Ctrl-C kills the process and loses at most one checkpoint interval, which is one minute. `quit`, or closing stdin, stops cleanly.

## Command reference

`swarm` or `swarm help` prints usage. `swarm help <command>`, or `--help` on a subcommand, prints that command. Default port is 5555. Default rules are `explorer`. Default run directory is `<data_dir>/exploraswarm/<rules>/`.

```text
swarm run     --server HOST[:PORT] [--password P] [--agents 8] [--rules explorer]
              [--run-dir DIR] [--physics-hz 30] [--temperature 0.4] [--sigma 0.03]
              [--reeval 0.1] [--reset-every 5] [--experience 64] [--seed N]
              [--name swarm] [--minutes M] [--fresh] [--no-console]
swarm status  [--rules R | --run-dir DIR]
swarm elites  [--rules R | --run-dir DIR] [--top 10]
swarm discoveries [--rules R | --run-dir DIR] [--last 20]
swarm ctl     [--rules R | --run-dir DIR] pause|resume|save|status|agents N|quit
swarm rules
swarm replay  [--rules R | --run-dir DIR] best|CELL --server HOST[:PORT]
              [--password P] [--episodes 1]
```

`run` resumes an archive when the rule-set name, the axes and the network shape match. Otherwise it stops and tells you to pass `--fresh` or another `--run-dir`. `--seed` applies to a new archive; a resumed run continues the saved random stream. `--reset-every` drops the body and joins again under the same name, which is how an agent gets a fresh spawn. A lost connection does the same, with a backoff of 1, 2, 4, 8, 16, then 30 seconds.

`status` prints the totals and a heat map of the first two axes. Empty cells are a space. Filled cells use `.:-=+*#%@` from the lowest fitness on that map to the highest. `elites` lists the strongest cells. `discoveries` prints the latest lines of the discovery log. `ctl` appends nothing in memory: it writes one command into the control file for a run that is already going. `replay` plays one stored elite with the greedy action and does not write the archive.

## How learning works

The archive is a grid over the rule set's axes. A descriptor outside an axis clamps into the edge bin. Each occupied cell stores one genome, a running mean and variance of its fitness (Welford), the descriptor from its first evaluation, and the behaviour signatures used for parent choice.

Insertion of a new genome:

- an empty cell takes it;
- an occupied cell queues a challenger when the new fitness beats `mean + margin`, with `margin = max(0.05·|mean|, 0.5·std)`;
- the challenger is evaluated once more, and it replaces the elite only if the mean of those two scores still beats the margin.

Re-evaluation (`--reeval`, default one candidate in ten) updates an elite's mean. Its cell stays put unless two re-evaluations in a row land in the same other cell, and then it has to win that cell under the normal rule. The score reported as QD-score is the sum of `(mean − lowest fitness seen)`, so it stays non-negative. Coverage is filled cells over the grid.

Emitters propose genomes:

- **Random** fills the archive until it holds 16 elites.
- **Gaussian** adds `N(0, σ²)` to every weight (`--sigma`, default 0.03).
- **IsoLine** takes two parents `x` and `y` and emits `x + σ_iso·N + σ_line·(y − x)·N(0,1)` with `σ_iso = 0.01` and `σ_line = 0.2`.

After the warm-up, a small bandit picks Gaussian or IsoLine from their recent insertion rate (the last 100 outcomes, with an ε floor of 0.1). Challengers and re-evaluations are handed out before a new child is asked for. The bandit windows live in the process; a resumed run starts them again. The random stream, the archive and the discovery map are what the checkpoint keeps.

Parent selection is shared. Draw four elites. With probability 0.75 take the one with the highest `rarity + bonus_norm`, otherwise take one of the four at random. Rarity is the mean, over that elite's signatures, of `1 / (1 + how many elites carry the signature)`. The bonus is the rule set's novelty term, squashed with `bonus / (1 + |bonus|)`. It never decides who keeps a cell. Fitness does, and fitness is stationary: the same behaviour scores the same after the swarm has seen it before.

Every random choice comes from a seeded generator. The run seed splits into an agent seed and then an episode seed.

## How to write or change rules

Rules are the extension point. A rule set names axes, says how long an episode is, and turns the agent's facts into a verdict. Fitness must be stationary. Anything that depends on what the rest of the swarm has already seen goes in `bonus` and `signatures`, which steer parent choice and the discovery log.

Add a file next to `src/rules/explorer.rs`, implement `Rules` and `Episode`, and register it in `all()` in `src/rules/mod.rs`. `src/rules/common.rs` has the helpers the two built-in sets share: coarse cells, a signature mix, diminishing-return square roots, an action tally, and a vertical-travel tracker. Material keys are canonical-spec hashes (`facts::Material.key`). `"air"` is `0`. Do not put game-local block ids in a signature.

A minimal set that only rewards ground covered:

```rust
use crate::facts::Fact;
use crate::rules::common::{self, CoarseCells};
use crate::rules::{Axis, Discoveries, Episode, EpisodeSpec, Rules, Verdict};

const AXES: &[Axis] = &[Axis { name: "cells", min: 0.0, max: 30.0, bins: 8 }];

pub struct Wander;

impl Rules for Wander {
    fn name(&self) -> &'static str { "wander" }
    fn about(&self) -> &'static str { "Reward newly entered 4 m cells." }
    fn episode(&self) -> EpisodeSpec { EpisodeSpec { decisions: 600, decide_hz: 10.0 } }
    fn axes(&self) -> &[Axis] { AXES }
    fn begin(&self) -> Box<dyn Episode> { Box::new(WanderEpisode::default()) }
}

#[derive(Default)]
struct WanderEpisode { cells: CoarseCells, rewards: Vec<f32> }

impl Episode for WanderEpisode {
    fn observe(&mut self, fact: &Fact, _swarm: &dyn Discoveries) {
        let Fact::Decision(decision) = fact else { return };
        if self.cells.enter(decision.pose.eye) {
            let before = self.cells.len() as u32 - 1;
            let credit = 0.5 * common::marginal_sqrt(before);
            if self.rewards.len() <= decision.step as usize {
                self.rewards.resize(decision.step as usize + 1, 0.0);
            }
            self.rewards[decision.step as usize] += credit;
        }
    }

    fn finish(self: Box<Self>, _swarm: &dyn Discoveries) -> Verdict {
        let fitness = 0.5 * common::sqrt_credit(self.cells.len() as u32);
        Verdict {
            fitness,
            descriptor: vec![self.cells.len() as f32],
            rewards: self.rewards,
            ..Verdict::default()
        }
    }
}
```

Then `swarm run --rules wander`. An archive written by `explorer` will not load under `wander`.

`explorer` (the default, 600 decisions at 10 Hz) rewards reactions, tool trials, materials gained and new coarse cells, and separates policies by coverage, vertical tendency, how often they act on the world, and the share of those acts that place a block. `builder` rewards confirmed placements and how high they stand. Snap-backs are counted and not penalised.

## Observation and actions

`NORM_VERSION` is 1. Values are scaled by fixed constants into roughly `[-1, 1]`. Nothing tracks a running mean. The vector is 174 floats:

| block | count | contents |
| --- | --- | --- |
| body | 10 | body-frame velocity / 10 (right, up, forward), grounded, in air, speed / 10, pitch / (π/2), sin yaw, cos yaw, gravity / 30 |
| rays | 85 | 17 rays × (distance / 8 m, solid, hardness, transparency, emission) |
| target | 10 | present, distance / 6 m, 7 material features, placeable |
| hand | 33 | 4 slots × (log2(1+count) / 7, 7 features), then inventory fill |
| peers | 4 | count / 8, nearest peer's body-frame right and up, distance / radius in place of forward |
| time | 2 | sin and cos of `2π · time_of_day` |
| previous action | 22 | one-hot, all zero on the first decision |
| memory | 4 | the network's previous memory outputs, zero at episode start |
| descriptor | 4 | reserved, always zero |

The 17 rays are 5 yaw offsets (±40°) by 3 pitch offsets (±25°), then straight down and straight up, out to 8 m. Peer distance replaces the forward component so a peer directly ahead still reports how far they are. The radius is 32 m.

The network is a perceptron: observation, two hidden layers of 32 with tanh, then 22 action logits and 4 memory outputs (the memory outputs pass through tanh). Weights are stored layer by layer, row-major, then that layer's biases, so a later gradient step can address a single weight. Actions are sampled from a masked softmax. `--temperature` defaults to 0.4. Replay takes the legal action with the largest logit.

The 22 actions are Idle, Forward, Back, Left, Right, ForwardJump, Sprint, Sneak, TurnLeft, TurnRight, LookUp, LookDown, Break, Place(0..3), UseTool(0..3), Swing. Movement and looking are held until the next decision. Break, place, tool use and swing fire once, on the decision's first physics frame. Physics defaults to 30 Hz and decisions to 10 Hz.

Four hand slots are filled from the inventory at the start of the episode and then stay put. A new material takes the first empty slot. A slot whose count drops to zero is masked, and it keeps its material so `Place(2)` means the same thing all episode. Break and tool use need a target. Place needs a free cell in front and a non-empty slot. World acts are masked to at most 8 per second, and each kind also waits out its cooldown (break 0.25 s, place 0.20 s, tool 0.25 s). Idle is always legal.

## Files written

All of these live in the run directory:

| file | role |
| --- | --- |
| `archive.bin` | elites, challengers, counters, network shape, random state |
| `discoveries.bin` | signatures the swarm has already seen, so a resume does not announce them again |
| `discoveries.log` | `episode`, agent, label, for the first sight of a signature |
| `episodes.log` | one TSV row per evaluation, ending with its three most chosen actions |
| `status.txt` | `key=value` snapshot, rewritten every few seconds |
| `control` | commands from `swarm ctl` or anything else; the run reads it, then truncates it |

Checkpoints are atomic: a temporary file is synced, then renamed. Trajectories (`--experience`, default 64) stay in memory and are not in the checkpoint.

## A future actor-critic emitter

The mutation emitters are the whole learner in this version. An `ActorCritic` emitter (PGA-ME or DCG-ME) plugs in beside them. `Emitter::ask` may return `None` while it trains, and it may own a background thread. The swarm already keeps the last N trajectories: observations (memory included), legality masks, actions, log-probabilities, and rewards with delayed credit already applied. The four descriptor slots in the observation are reserved so a critic can be conditioned on the behaviour descriptor. The emitter would fit that critic on the buffer, take policy-gradient steps on a selected elite's flat parameter vector, and `ask` would return the updated vector as a candidate. `tell` already receives every evaluation and the archive's insertion outcome, which is the signal PGA-ME uses to decide whether the gradient step improved the cell.

## Compatibility

Requires `pwc-api` ^3.4. Archive format version is 1. `NORM_VERSION` is 1 and the network version is 1. A resumed archive must match the rule-set name, each axis name and its endpoints and bin count, and the network shape (`NetSpec`: version, observation length, action count, memory length, hidden widths, norm version). A mismatch is an error unless `--fresh` replaces the directory's archive. The emitter bandit's recent-success window is not stored, so a resumed run starts that window empty.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
