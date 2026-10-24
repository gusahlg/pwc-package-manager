// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! A second rule set: reward confirmed placements and how high they stand.
//!
//! It exists so a swarm can be pointed at building without any change to agents, networks or the
//! archive. Fitness is stationary; which materials have been placed before is only a bonus.

use std::collections::BTreeSet;

use crate::body::CELL_METRES;
use crate::facts::{EditKind, Fact, Outcome};
use crate::rules::common;
use crate::rules::{Axis, Discoveries, Episode, EpisodeSpec, Rules, Verdict};

const AXES: &[Axis] = &[
    Axis {
        name: "placed",
        min: 0.0,
        max: 60.0,
        bins: 8,
    },
    Axis {
        name: "height",
        min: 0.0,
        max: 8.0,
        bins: 8,
    },
    Axis {
        name: "variety",
        min: 0.0,
        max: 6.0,
        bins: 6,
    },
];

pub struct Builder;

impl Rules for Builder {
    fn name(&self) -> &'static str {
        "builder"
    }

    fn about(&self) -> &'static str {
        "Reward confirmed placements and the height of what was built."
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
        Box::new(BuilderEpisode::default())
    }
}

#[derive(Default)]
struct BuilderEpisode {
    start_eye: Option<[f64; 3]>,
    start_up: Option<[f64; 3]>,
    placed: u32,
    height: f32,
    materials: BTreeSet<u64>,
    rejected: u32,
    decisions: u32,
    rewards: Vec<f32>,
}

impl BuilderEpisode {
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
}

fn above_start(cell: (i32, i32, i32), eye: [f64; 3], up: [f64; 3]) -> f32 {
    let m = f64::from(CELL_METRES);
    let d = [
        (cell.0 as f64 + 0.5) * m - eye[0],
        (cell.1 as f64 + 0.5) * m - eye[1],
        (cell.2 as f64 + 0.5) * m - eye[2],
    ];
    (d[0] * up[0] + d[1] * up[1] + d[2] * up[2]) as f32
}

impl Episode for BuilderEpisode {
    fn observe(&mut self, fact: &Fact, _swarm: &dyn Discoveries) {
        match fact {
            Fact::Start { pose } => {
                self.start_eye = Some(pose.eye);
                self.start_up = pose.up;
            }
            Fact::Decision(_) => {
                self.decisions = self.decisions.saturating_add(1);
            }
            Fact::Edit {
                step,
                cell,
                kind,
                after,
                outcome,
                ..
            } => match outcome {
                Outcome::Confirmed if *kind == EditKind::Placed => {
                    let before = self.placed;
                    self.placed = self.placed.saturating_add(1);
                    self.credit(*step, common::marginal_sqrt(before));
                    if !after.is_air() {
                        self.materials.insert(after.key);
                    }
                    if let (Some(eye), Some(up)) = (self.start_eye, self.start_up) {
                        let h = above_start(*cell, eye, up);
                        if h > self.height {
                            self.credit(*step, 1.5 * (h - self.height));
                            self.height = h;
                        }
                    }
                }
                Outcome::Rejected => {
                    self.rejected = self.rejected.saturating_add(1);
                    self.credit(*step, -0.2);
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn finish(mut self: Box<Self>, swarm: &dyn Discoveries) -> Verdict {
        let fitness =
            common::sqrt_credit(self.placed) + 1.5 * self.height - 0.2 * self.rejected as f32;
        let mut bonus = 0.0f32;
        let mut signatures = Vec::with_capacity(self.materials.len());
        for key in &self.materials {
            let seen = swarm.count(*key);
            bonus += 1.0 / (1.0 + seen as f32);
            let k = *key;
            swarm.note(*key, &|| format!("placed c:{k:x}"));
            signatures.push(*key);
        }
        if (self.decisions as usize) > self.rewards.len() {
            self.rewards.resize(self.decisions as usize, 0.0);
        }
        Verdict {
            fitness,
            descriptor: vec![self.placed as f32, self.height, self.materials.len() as f32],
            bonus,
            signatures,
            rewards: self.rewards,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::{ActionKind, Decision, Material, Pose};
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct Mem {
        map: Mutex<HashMap<u64, u64>>,
    }

    impl Discoveries for Mem {
        fn note(&self, signature: u64, _label: &dyn Fn() -> String) -> bool {
            let mut m = self.map.lock().unwrap();
            let e = m.entry(signature).or_insert(0);
            *e += 1;
            *e == 1
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

    fn mat(key: u64) -> Material {
        Material {
            key,
            features: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        }
    }

    fn stream() -> Vec<Fact> {
        let pose = Pose {
            eye: [0.0, 0.0, 0.0],
            velocity: [0.0; 3],
            up: Some([0.0, 1.0, 0.0]),
            grounded: true,
            in_air: true,
        };
        vec![
            Fact::Start { pose },
            Fact::Decision(Decision {
                step: 0,
                pose,
                action: 13,
                kind: ActionKind::Place,
                legal: 4,
                peers_near: 0,
                nearest_peer: None,
            }),
            Fact::Edit {
                step: 0,
                cell: (0, 2, 0),
                kind: EditKind::Placed,
                before: Material::AIR,
                after: mat(7),
                outcome: Outcome::Predicted,
            },
            Fact::Edit {
                step: 0,
                cell: (0, 2, 0),
                kind: EditKind::Placed,
                before: Material::AIR,
                after: mat(7),
                outcome: Outcome::Confirmed,
            },
            Fact::End {
                reason: crate::facts::EndReason::TimeLimit,
            },
        ]
    }

    fn run(mem: &Mem) -> Verdict {
        let mut ep = Builder.begin();
        for fact in stream() {
            ep.observe(&fact, mem);
        }
        ep.finish(mem)
    }

    #[test]
    fn confirmed_placements_score_and_a_prediction_does_not() {
        let mem = Mem {
            map: Mutex::new(HashMap::new()),
        };
        let v = run(&mem);
        let height = (2.5 * f64::from(CELL_METRES)) as f32;
        let expect = 1.0 + 1.5 * height;
        assert!(
            (v.fitness - expect).abs() < 1e-4,
            "{} vs {expect}",
            v.fitness
        );
        assert!((v.descriptor[0] - 1.0).abs() < 1e-5);
        assert!((v.descriptor[1] - height).abs() < 1e-4);
        assert!((v.descriptor[2] - 1.0).abs() < 1e-5);
        for (i, d) in v.descriptor.iter().enumerate() {
            assert!(
                *d >= AXES[i].min - 1e-4 && *d <= AXES[i].max + 1e-4,
                "{} = {d}",
                AXES[i].name
            );
        }
        assert!(
            (v.rewards[0] - expect).abs() < 1e-4,
            "the confirmation credits step 0"
        );
        assert!((v.bonus - 1.0).abs() < 1e-5);
        let again = run(&mem);
        assert!((again.fitness - v.fitness).abs() < 1e-5);
        assert!(again.bonus < v.bonus);
    }
}
