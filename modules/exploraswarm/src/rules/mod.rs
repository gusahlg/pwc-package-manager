// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Rules: what the swarm finds interesting. Everything that decides rewards, quality and
//! behavioural difference lives here; the machinery (agents, networks, archive, learning) only
//! calls this interface.
//!
//! A rule set names a behaviour space (`axes`) and starts one `Episode` per evaluation. The episode
//! owns its own state, watches the agent's `Fact`s and finally returns a `Verdict`: the quality that
//! decides who keeps a niche, the descriptor that decides which niche, per-decision rewards for
//! gradient learners, and the behaviour signatures used for novelty.
//!
//! Two kinds of score are kept apart on purpose:
//! - `fitness` must be *stationary*: the same behaviour in the same place scores the same whenever it
//!   happens, so elites can be compared fairly and a critic can learn from it;
//! - swarm-wide novelty (doing what nobody did before) is *not* stationary, so it goes to `bonus` and
//!   `signatures`, which steer parent selection and the discovery log, never niche replacement.
//!
//! To add a rule set: write a file next to `explorer.rs`, implement `Rules` and `Episode`, and add it
//! to `all()`. `common.rs` has helpers most rule sets share.

pub mod builder;
pub mod common;
pub mod explorer;

use crate::facts::Fact;

/// One dimension of the behaviour space. Values outside `min..=max` fall in the edge bins.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axis {
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub bins: u16,
}

/// How long an episode is and how often the agent decides.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EpisodeSpec {
    /// Decisions per episode.
    pub decisions: u32,
    /// Decisions per second.
    pub decide_hz: f32,
}

/// The result of one evaluation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Verdict {
    /// Stationary quality: decides which genome keeps a niche.
    pub fitness: f32,
    /// One value per axis, in axis order.
    pub descriptor: Vec<f32>,
    /// Non-stationary extra credit (e.g. swarm-wide firsts). Reported and used for parent selection,
    /// never for niche replacement.
    pub bonus: f32,
    /// Behaviour signatures of this episode (rule-defined hashes, e.g. reaction kinds). Parents whose
    /// signatures are rare in the archive are preferred.
    pub signatures: Vec<u64>,
    /// Reward per decision, indexed by `Decision::step`, with delayed answers credited to the decision
    /// that caused them. Its sum need not equal `fitness`.
    pub rewards: Vec<f32>,
}

/// The swarm's shared memory of what has been seen, across all agents and episodes. Rules decide what
/// a signature means; the machinery stores hashes, counts and the first finder for the discovery log.
pub trait Discoveries {
    /// Record one occurrence of `signature`. Returns true when nobody in the swarm produced it before;
    /// then `label` (a short human description) goes to the discovery log.
    fn note(&self, signature: u64, label: &dyn Fn() -> String) -> bool;
    /// How often `signature` was noted so far.
    fn count(&self, signature: u64) -> u64;
}

/// One evaluation in progress. Owns whatever state the rule set needs.
pub trait Episode: Send {
    /// Called for every fact, in order.
    fn observe(&mut self, fact: &Fact, swarm: &dyn Discoveries);
    /// Called once after the `Fact::End`.
    fn finish(self: Box<Self>, swarm: &dyn Discoveries) -> Verdict;
}

/// A rule set.
pub trait Rules: Send + Sync {
    /// Short, stable name (`--rules <name>`, run directories, archive headers).
    fn name(&self) -> &'static str;
    /// One line for `swarm rules`.
    fn about(&self) -> &'static str;
    fn episode(&self) -> EpisodeSpec;
    /// The behaviour space. Changing it makes old archives of this rule set incompatible.
    fn axes(&self) -> &[Axis];
    fn begin(&self) -> Box<dyn Episode>;
}

/// Every rule set, default first.
pub fn all() -> Vec<Box<dyn Rules>> {
    vec![Box::new(explorer::Explorer), Box::new(builder::Builder)]
}

/// The rule set called `name`.
pub fn by_name(name: &str) -> Option<Box<dyn Rules>> {
    all().into_iter().find(|r| r.name() == name)
}
