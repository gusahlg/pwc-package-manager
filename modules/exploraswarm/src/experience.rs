// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Per-episode trajectories kept in memory for a future gradient emitter.
//!
//! The swarm holds the last `N` (`--experience`, default 64). They are not written into the v1
//! checkpoint. Rewards already have delayed credit applied: index `t` is the reward of decision `t`.
//! Memory inputs are inside `obs` (see `senses`), so a critic can train without a second buffer.

use std::collections::VecDeque;

use crate::archive::Candidate;
use crate::facts::EndReason;
use crate::rules::Verdict;

/// One finished episode, stored densely.
#[derive(Clone, Debug)]
pub struct Trajectory {
    // Read by the planned gradient learner (see README).
    #[allow(dead_code)]
    pub seed: u64,
    #[allow(dead_code)]
    pub obs_len: usize,
    /// `T * obs_len` floats, decision by decision.
    pub obs: Vec<f32>,
    pub masks: Vec<u64>,
    pub actions: Vec<u16>,
    pub logp: Vec<f32>,
    pub rewards: Vec<f32>,
    /// The episode ended because the decision budget ran out (or the swarm stopped).
    pub truncated: bool,
    /// The body was lost or the server dropped it.
    pub terminal: bool,
    pub genome_id: u64,
    pub descriptor: Vec<f32>,
}

impl Trajectory {
    // Read by the planned gradient learner (see README).
    #[allow(dead_code)]
    pub fn steps(&self) -> usize {
        self.actions.len()
    }

    pub fn empty(seed: u64, obs_len: usize, genome_id: u64) -> Self {
        Self {
            seed,
            obs_len,
            obs: Vec::new(),
            masks: Vec::new(),
            actions: Vec::new(),
            logp: Vec::new(),
            rewards: Vec::new(),
            truncated: false,
            terminal: false,
            genome_id,
            descriptor: Vec::new(),
        }
    }
}

/// One evaluation, as the agent hands it back.
#[derive(Clone, Debug)]
pub struct Evaluation {
    pub candidate: Candidate,
    pub verdict: Verdict,
    pub trajectory: Trajectory,
    pub end_reason: EndReason,
    #[allow(dead_code)]
    pub decisions: u32,
    /// Id assigned when the episode started. The log and the archive share it.
    pub episode: u64,
    /// Agent name, for the episode log.
    pub agent: String,
}

/// Ring of recent trajectories. Pushing past `cap` drops the oldest.
#[derive(Clone, Debug)]
pub struct Experience {
    cap: usize,
    buf: VecDeque<Trajectory>,
}

impl Experience {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            buf: VecDeque::new(),
        }
    }

    pub fn push(&mut self, trajectory: Trajectory) {
        if self.cap == 0 {
            return;
        }
        if self.buf.len() == self.cap {
            self.buf.pop_front();
        }
        self.buf.push_back(trajectory);
    }

    // Read by the planned gradient learner (see README).
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    #[allow(dead_code)]
    pub fn iter(&self) -> impl Iterator<Item = &Trajectory> {
        self.buf.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_drops_the_oldest() {
        let mut exp = Experience::new(2);
        for i in 0..3 {
            let mut t = Trajectory::empty(i, 1, i);
            t.actions.push(i as u16);
            exp.push(t);
        }
        assert_eq!(exp.len(), 2);
        let ids: Vec<u64> = exp.iter().map(|t| t.genome_id).collect();
        assert_eq!(ids, vec![1, 2]);
    }
}
