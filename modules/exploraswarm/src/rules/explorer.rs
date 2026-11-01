// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The default rule set: reward reactions, tool trials, materials gained and ground covered,
//! and keep those policies apart in a four-axis behaviour space.
//!
//! Fitness is stationary. How novel a reaction is across the swarm goes to `bonus` and
//! `signatures`, which steer parent selection, and never into who keeps a cell.
//!
//! A reaction signature is `mix(cell, tool, cell_after, tool_after)` for a tool that reacted, plus
//! `mix(before, after)` for a mutation within 2 s and 6 m of the agent's own edit that turns one
//! material into another. A mutation to or from air is left out: it is usually a nearby player
//! breaking or placing, which the client reports the same way as a reaction. Snap-backs are
//! counted and not penalised.

use std::collections::{BTreeMap, BTreeSet};

use crate::facts::{ActionKind, Fact, Outcome};
use crate::rules::common::{self, CoarseCells, Tally, Vertical};
use crate::rules::{Axis, Discoveries, Episode, EpisodeSpec, Rules, Verdict};

const AXES: &[Axis] = &[
    Axis {
        name: "coverage",
        min: 0.0,
        max: 5.0,
        bins: 8,
    },
    Axis {
        name: "vertical",
        min: -1.0,
        max: 1.0,
        bins: 8,
    },
    Axis {
        name: "act_rate",
        min: 0.0,
        max: 0.5,
        bins: 8,
    },
    Axis {
        name: "place_share",
        min: 0.0,
        max: 1.0,
        bins: 8,
    },
];

pub struct Explorer;

impl Rules for Explorer {
    fn name(&self) -> &'static str {
        "explorer"
    }

    fn about(&self) -> &'static str {
        "Seek reactions, new materials and fresh ground; travel style is a niche, not a score."
    }

    fn episode(&self) -> EpisodeSpec {
        EpisodeSpec {
            decisions: 600,
            decide_hz: 10.0,
        }
    }

    fn axes(&self) -> &[Axis] {
        AXES
    }

    fn begin(&self) -> Box<dyn Episode> {
        Box::new(ExplorerEpisode::default())
    }
}

#[derive(Default)]
struct ExplorerEpisode {
    decisions: u32,
    cells: CoarseCells,
    vertical: Vertical,
    tally: Tally,
    reactions: BTreeMap<u64, String>,
    pairs: BTreeSet<(u64, u64)>,
    obtained: BTreeSet<u64>,
    rejected: u32,
    /// Counted, not scored. See the module note.
    snaps: u32,
    rewards: Vec<f32>,
    last_step: u32,
}

impl ExplorerEpisode {
    fn credit(&mut self, step: u32, amount: f32) {
        if amount == 0.0 || !amount.is_finite() {
            return;
        }
        let i = step as usize;
        if self.rewards.len() <= i {
            self.rewards.resize(i + 1, 0.0);
        }
        self.rewards[i] += amount;
    }

    fn credit_cell(&mut self, step: u32, eye: [f64; 3]) {
        if self.cells.enter(eye) {
            let before = self.cells.len() as u32 - 1;
            self.credit(step, 0.5 * common::marginal_sqrt(before));
        }
    }

    fn credit_reaction(&mut self, step: u32, sig: u64, label: String) {
        if self.reactions.contains_key(&sig) {
            return;
        }
        let before = self.reactions.len() as u32;
        self.reactions.insert(sig, label);
        self.credit(step, 3.0 * common::marginal_sqrt(before));
    }
}

impl Episode for ExplorerEpisode {
    fn observe(&mut self, fact: &Fact, _swarm: &dyn Discoveries) {
        match fact {
            Fact::Start { pose } => {
                self.vertical.observe(pose.eye, pose.up);
                self.credit_cell(0, pose.eye);
            }
            Fact::Decision(d) => {
                self.decisions = self.decisions.saturating_add(1);
                self.last_step = d.step;
                self.tally.add(d.kind);
                self.vertical.observe(d.pose.eye, d.pose.up);
                self.credit_cell(d.step, d.pose.eye);
            }
            Fact::Tool {
                step,
                cell_before,
                tool_before,
                cell_after,
                tool_after,
                reacted,
                ..
            } => {
                let pair = (tool_before.key, cell_before.key);
                if self.pairs.insert(pair) {
                    let before = self.pairs.len() as u32 - 1;
                    self.credit(*step, common::marginal_sqrt(before));
                }
                if *reacted {
                    let sig = common::mix(&[
                        cell_before.key,
                        tool_before.key,
                        cell_after.key,
                        tool_after.key,
                    ]);
                    let label = format!(
                        "reaction c:{:x} + tool c:{:x} -> c:{:x} + c:{:x}",
                        cell_before.key, tool_before.key, cell_after.key, tool_after.key
                    );
                    self.credit_reaction(*step, sig, label);
                }
            }
            Fact::Mutation {
                before,
                after,
                distance,
                since_own_edit,
                ..
            } => {
                let close = since_own_edit.is_some_and(|dt| dt < 2.0) && *distance < 6.0;
                if close && !before.is_air() && !after.is_air() {
                    let sig = common::mix(&[before.key, after.key]);
                    let label = format!("mutation c:{:x} -> c:{:x}", before.key, after.key);
                    self.credit_reaction(self.last_step, sig, label);
                }
            }
            Fact::Inventory { material, delta } => {
                if *delta > 0 && !material.is_air() && self.obtained.insert(material.key) {
                    let before = self.obtained.len() as u32 - 1;
                    self.credit(self.last_step, common::marginal_sqrt(before));
                }
            }
            Fact::Edit {
                step,
                outcome: Outcome::Rejected,
                ..
            } => {
                self.rejected = self.rejected.saturating_add(1);
                self.credit(*step, -0.2);
            }
            Fact::SnappedBack => self.snaps = self.snaps.saturating_add(1),
            Fact::Edit { .. } | Fact::Refused { .. } | Fact::End { .. } => {}
        }
    }

    fn finish(mut self: Box<Self>, swarm: &dyn Discoveries) -> Verdict {
        let breaks = self.tally.get(ActionKind::Break);
        let places = self.tally.get(ActionKind::Place);
        let tools = self.tally.get(ActionKind::UseTool);
        let world_acts = breaks + places + tools;
        let decisions = self.decisions;
        let coverage = if decisions == 0 {
            0.0
        } else {
            let per100 = self.cells.len() as f32 * 100.0 / decisions as f32;
            (1.0 + per100).log2()
        };
        let vertical = self.vertical.score();
        let act_rate = if decisions == 0 {
            0.0
        } else {
            world_acts as f32 / decisions as f32
        };
        let place_share = if world_acts == 0 {
            0.0
        } else {
            places as f32 / world_acts as f32
        };
        let fitness = 3.0 * common::sqrt_credit(self.reactions.len() as u32)
            + common::sqrt_credit(self.pairs.len() as u32)
            + common::sqrt_credit(self.obtained.len() as u32)
            + 0.5 * common::sqrt_credit(self.cells.len() as u32)
            - 0.2 * self.rejected as f32;

        let mut bonus = 0.0f32;
        let mut signatures = Vec::with_capacity(self.reactions.len() + self.obtained.len());
        for (sig, label) in &self.reactions {
            let seen = swarm.count(*sig);
            bonus += 1.0 / (1.0 + seen as f32);
            swarm.note(*sig, &|| label.clone());
            signatures.push(*sig);
        }
        for key in &self.obtained {
            let seen = swarm.count(*key);
            bonus += 1.0 / (1.0 + seen as f32);
            let k = *key;
            swarm.note(*key, &|| format!("material c:{k:x}"));
            signatures.push(*key);
        }

        if (decisions as usize) > self.rewards.len() {
            self.rewards.resize(decisions as usize, 0.0);
        }
        let _counted = self.snaps;
        Verdict {
            fitness,
            descriptor: vec![coverage, vertical, act_rate, place_share],
            bonus,
            signatures,
            rewards: self.rewards,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::{Decision, EditKind, EndReason, Material, Pose};
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct Mem {
        map: Mutex<HashMap<u64, u64>>,
    }

    impl Discoveries for Mem {
        fn note(&self, signature: u64, label: &dyn Fn() -> String) -> bool {
            let mut m = self.map.lock().unwrap();
            let e = m.entry(signature).or_insert(0);
            *e += 1;
            let first = *e == 1;
            if first {
                let _ = label();
            }
            first
        }

        fn count(&self, signature: u64) -> u64 {
            self.map
                .lock()
                .unwrap()
                .get(&signature)
                .copied()
                .unwrap_or(0)
        }
    }

    fn pose(x: f64, y: f64) -> Pose {
        Pose {
            eye: [x, y, 0.0],
            velocity: [0.0; 3],
            up: Some([0.0, 1.0, 0.0]),
            grounded: true,
            in_air: true,
        }
    }

    fn mat(key: u64) -> Material {
        Material {
            key,
            features: [1.0, 0.2, 0.2, 0.0, 0.0, 0.5, 0.1],
        }
    }

    fn decision(step: u32, kind: ActionKind) -> Fact {
        Fact::Decision(Decision {
            step,
            pose: pose(0.5, 1.0),
            action: 0,
            kind,
            legal: 8,
            peers_near: 0,
            nearest_peer: None,
        })
    }

    fn stream() -> Vec<Fact> {
        vec![
            Fact::Start {
                pose: pose(0.5, 1.0),
            },
            decision(0, ActionKind::Move),
            decision(1, ActionKind::Move),
            decision(2, ActionKind::Break),
            decision(3, ActionKind::Place),
            Fact::Tool {
                step: 0,
                cell: (1, 1, 1),
                cell_before: mat(1),
                tool_before: mat(2),
                cell_after: mat(3),
                tool_after: mat(4),
                reacted: true,
            },
            Fact::Edit {
                step: 2,
                cell: (1, 1, 1),
                kind: EditKind::Broke,
                before: mat(1),
                after: Material::AIR,
                outcome: Outcome::Rejected,
            },
            Fact::Inventory {
                material: mat(9),
                delta: 1,
            },
            Fact::SnappedBack,
            Fact::End {
                reason: EndReason::TimeLimit,
            },
        ]
    }

    fn run(mem: &Mem) -> Verdict {
        let mut ep = Explorer.begin();
        for fact in stream() {
            ep.observe(&fact, mem);
        }
        ep.finish(mem)
    }

    #[test]
    fn verdict_is_in_range_and_delayed_credit_lands_on_the_issuing_step() {
        let mem = Mem {
            map: Mutex::new(HashMap::new()),
        };
        let v = run(&mem);
        assert!((v.fitness - 5.3).abs() < 1e-4, "fitness {}", v.fitness);
        assert_eq!(v.descriptor.len(), 4);
        for (i, d) in v.descriptor.iter().enumerate() {
            assert!(d.is_finite(), "{i}");
            assert!(
                *d >= AXES[i].min - 1e-4 && *d <= AXES[i].max + 1e-4,
                "axis {} = {d}",
                AXES[i].name
            );
        }
        assert!(
            (v.rewards[0] - 4.5).abs() < 1e-4,
            "step 0 got the cell, the pair and the reaction: {:?}",
            v.rewards
        );
        assert_eq!(
            v.rewards[1], 0.0,
            "the tool was issued at step 0, not step 1"
        );
        assert!((v.rewards[2] + 0.2).abs() < 1e-4);
        assert!((v.rewards[3] - 1.0).abs() < 1e-4);
        assert!((v.bonus - 2.0).abs() < 1e-4);
    }

    #[test]
    fn fitness_is_stationary_and_novelty_is_only_in_the_bonus() {
        let mem = Mem {
            map: Mutex::new(HashMap::new()),
        };
        let first = run(&mem);
        let second = run(&mem);
        assert!((first.fitness - second.fitness).abs() < 1e-5);
        assert!(first.bonus > second.bonus);
        assert!(
            (second.bonus - 1.0).abs() < 1e-4,
            "each signature was already seen once"
        );
        assert_eq!(first.descriptor, second.descriptor);
    }
}
