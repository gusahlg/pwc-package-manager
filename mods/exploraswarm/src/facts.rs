// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The vocabulary between the machinery and the rules: what happened to one agent, as plain data.
//!
//! The agent loop turns its body's events into `Fact`s and hands each to the rules' `Episode`.
//! Rules never see networks, threads, the archive or the game client, only these facts.
//! Materials are keyed by a hash of their canonical spec, so keys agree across agents and runs.

/// A world cell (storage coordinates).
pub type Cell = (i32, i32, i32);

/// Number of observed features per material.
pub const MATERIAL_FEATURES: usize = 7;

/// A material as rules see it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    /// Stable key: a hash of the canonical spec (`"air"` is [`AIR`]).
    pub key: u64,
    /// solid, hardness, friction, transparency, emission, cohesion, element count; each in 0..=1.
    pub features: [f32; MATERIAL_FEATURES],
}

/// The key of air.
pub const AIR: u64 = 0;

impl Material {
    pub const AIR: Material = Material {
        key: AIR,
        features: [0.0; MATERIAL_FEATURES],
    };

    pub fn is_air(&self) -> bool {
        self.key == AIR
    }
}

/// Where the body is, sampled once per decision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Eye position.
    pub eye: [f64; 3],
    pub velocity: [f32; 3],
    /// Local up (against gravity); `None` where gravity is too weak to define it.
    pub up: Option<[f64; 3]>,
    pub grounded: bool,
    /// Inside an atmosphere.
    pub in_air: bool,
}

/// The coarse kind of an action, so rules can reason about actions without the action table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionKind {
    Idle,
    Move,
    Jump,
    Look,
    Break,
    Place,
    UseTool,
    Swing,
}

impl ActionKind {
    /// Whether the action tries to change the world.
    pub fn acts_on_world(self) -> bool {
        matches!(
            self,
            ActionKind::Break | ActionKind::Place | ActionKind::UseTool
        )
    }
}

/// One decision: what the agent chose and where it was.
#[derive(Clone, Debug)]
pub struct Decision {
    /// Decision index within the episode, from 0.
    pub step: u32,
    pub pose: Pose,
    /// Index into the action table.
    pub action: u16,
    pub kind: ActionKind,
    /// How many actions were legal.
    pub legal: u16,
    /// Peers within the sensing radius and the distance to the nearest one.
    pub peers_near: u16,
    pub nearest_peer: Option<f32>,
}

/// What became of an edit the agent issued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Applied locally, waiting for the server.
    Predicted,
    Confirmed,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    Broke,
    Placed,
}

/// Why an action was refused before it reached the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NoTarget,
    Cooldown,
    NotHeld,
    Occupied,
}

/// Why an episode ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    /// The decision budget ran out (a truncation: the world goes on).
    TimeLimit,
    /// No ground and no defined up for too long; the body will be reset.
    Lost,
    Disconnected,
    /// The swarm is stopping.
    Stopped,
}

/// One thing that happened to the agent. `step` names the decision that caused it, so delayed
/// answers (server confirmations, reactions) can be credited to the right decision.
#[derive(Clone, Debug)]
pub enum Fact {
    Start {
        pose: Pose,
    },
    Decision(Decision),
    Edit {
        step: u32,
        cell: Cell,
        kind: EditKind,
        before: Material,
        after: Material,
        outcome: Outcome,
    },
    /// The law's answer to a tool use: (cell, tool) became (cell_after, tool_after).
    Tool {
        step: u32,
        cell: Cell,
        cell_before: Material,
        tool_before: Material,
        cell_after: Material,
        tool_after: Material,
        reacted: bool,
    },
    /// A cell changed that the agent did not edit (another player, or a reaction commit), within the
    /// sensing radius. `since_own_edit` is the time since the agent's last confirmed edit within a
    /// few metres of it, when there was one: a hint that the agent caused it.
    Mutation {
        cell: Cell,
        before: Material,
        after: Material,
        distance: f32,
        since_own_edit: Option<f32>,
    },
    /// The inventory gained (`delta > 0`) or lost a material.
    Inventory {
        material: Material,
        delta: i32,
    },
    Refused {
        step: u32,
        reason: Refusal,
    },
    SnappedBack,
    End {
        reason: EndReason,
    },
}
