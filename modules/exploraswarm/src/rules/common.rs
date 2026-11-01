// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Helpers most rule sets share: coarse cells, signature mixing, diminishing-return credit,
//! an action tally and a vertical-tendency tracker.

use std::collections::BTreeSet;

use crate::facts::ActionKind;
use crate::rng;

/// Visited 4 m cells, keyed on the eye position. The first time a cell is entered, `enter` is true.
#[derive(Clone, Debug, Default)]
pub struct CoarseCells {
    seen: BTreeSet<(i32, i32, i32)>,
}

impl CoarseCells {
    pub fn enter(&mut self, eye: [f64; 3]) -> bool {
        self.seen.insert(coarse_key(eye))
    }

    pub fn len(&self) -> usize {
        self.seen.len()
    }
}

fn coarse_key(eye: [f64; 3]) -> (i32, i32, i32) {
    (
        (eye[0] / 4.0).floor() as i32,
        (eye[1] / 4.0).floor() as i32,
        (eye[2] / 4.0).floor() as i32,
    )
}

/// SplitMix-style mix of the parts. The same parts in the same order always give the same `u64`.
pub fn mix(parts: &[u64]) -> u64 {
    rng::mix(parts)
}

/// `√n` for a count. Fitness uses this so the tenth new thing is worth less than the first.
pub fn sqrt_credit(count: u32) -> f32 {
    (count as f32).sqrt()
}

/// `√(n+1) − √n`, the fitness gained by one more item.
pub fn marginal_sqrt(before: u32) -> f32 {
    sqrt_credit(before + 1) - sqrt_credit(before)
}

/// How often each [`ActionKind`] was chosen.
#[derive(Clone, Debug, Default)]
pub struct Tally {
    n: [u32; 8],
}

impl Tally {
    pub fn add(&mut self, kind: ActionKind) {
        self.n[kind_index(kind)] = self.n[kind_index(kind)].saturating_add(1);
    }

    pub fn get(&self, kind: ActionKind) -> u32 {
        self.n[kind_index(kind)]
    }
}

fn kind_index(kind: ActionKind) -> usize {
    match kind {
        ActionKind::Idle => 0,
        ActionKind::Move => 1,
        ActionKind::Jump => 2,
        ActionKind::Look => 3,
        ActionKind::Break => 4,
        ActionKind::Place => 5,
        ActionKind::UseTool => 6,
        ActionKind::Swing => 7,
    }
}

/// Displacement along the local up, summed per decision. Steps with no defined up are ignored
/// and do not bridge the gap to the next defined one.
#[derive(Clone, Debug, Default)]
pub struct Vertical {
    prev: Option<[f64; 3]>,
    pub up_gained: f32,
    pub down_gained: f32,
}

impl Vertical {
    pub fn observe(&mut self, eye: [f64; 3], up: Option<[f64; 3]>) {
        let Some(u) = up else {
            self.prev = None;
            return;
        };
        if let Some(prev) = self.prev {
            let proj = ((eye[0] - prev[0]) * u[0]
                + (eye[1] - prev[1]) * u[1]
                + (eye[2] - prev[2]) * u[2]) as f32;
            if proj >= 0.0 {
                self.up_gained += proj;
            } else {
                self.down_gained -= proj;
            }
        }
        self.prev = Some(eye);
    }

    /// `(up − down) / (up + down + 0.5 m)`. Zero when the body never had a defined up.
    pub fn score(&self) -> f32 {
        (self.up_gained - self.down_gained) / (self.up_gained + self.down_gained + 0.5)
    }
}
