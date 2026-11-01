// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The agent's body: the seam between the machinery and the game.
//!
//! The machinery drives a `Body` and never names a game type; `client.rs` implements it over the
//! game's `HeadlessClient`, and tests implement it with a scripted fake. Materials are plain
//! `facts::Material`s, and an action names a held material by its key.

use crate::facts::{Cell, EditKind, Material, Outcome, Pose, Refusal};

/// Which frame a sensing ray is aimed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RayBasis {
    /// `yaw` and `pitch` are offsets from the look direction, in radians.
    View,
    /// Straight along the body's up. `yaw` and `pitch` are ignored.
    Up,
    /// Straight along the body's down. `yaw` and `pitch` are ignored.
    Down,
}

/// The direction of one sensing ray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayDir {
    pub yaw: f32,
    pub pitch: f32,
    pub basis: RayBasis,
}

impl RayDir {
    pub const fn view(yaw: f32, pitch: f32) -> Self {
        Self {
            yaw,
            pitch,
            basis: RayBasis::View,
        }
    }

    pub const fn up() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            basis: RayBasis::Up,
        }
    }

    pub const fn down() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            basis: RayBasis::Down,
        }
    }
}

/// One cell edge in the units `Pose`, ray distances and peer distances use.
///
/// The game's block is 0.85 m. `client.rs` scales block distances into these units, and the
/// machinery treats one cell index step as this many metres when it compares cells.
pub const CELL_METRES: f32 = 0.85;

/// What one ray hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    /// Distance to the first solid cell in metres; `range` when nothing was hit.
    pub distance: f32,
    /// The material hit, `Material::AIR` when nothing was.
    pub material: Material,
}

/// The block the agent looks at, within reach.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub cell: Cell,
    pub distance: f32,
    pub material: Material,
    /// A block placed now would fit (the cell in front is free and outside the body).
    pub placeable: bool,
}

/// Another player near the agent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PeerView {
    /// Offset from the agent's eye in its body frame: right, up, forward (metres).
    pub offset: [f32; 3],
    pub distance: f32,
}

/// What a player logically knows at one moment. Reused between decisions (no allocation per tick).
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub pose: Option<Pose>,
    pub yaw: f32,
    pub pitch: f32,
    /// Velocity in the body frame: right, up, forward (m/s).
    pub body_velocity: [f32; 3],
    /// Gravity magnitude (m/s²).
    pub gravity: f32,
    /// One entry per requested ray, in request order.
    pub rays: Vec<RayHit>,
    pub target: Option<Target>,
    /// Held materials and counts, in the client's inventory order.
    pub inventory: Vec<(Material, u32)>,
    pub inventory_total: u32,
    pub inventory_capacity: u32,
    /// Peers within the sensing radius, nearest first.
    pub peers: Vec<PeerView>,
    /// 0..1 through the day.
    pub time_of_day: f32,
}

/// One world action.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    Break,
    /// Place one unit of the held material with this key.
    Place(u64),
    /// Use the held material with this key as a tool on the target.
    UseTool(u64),
    Swing,
}

/// The controls for one physics frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls {
    /// -1..=1.
    pub forward: f32,
    pub right: f32,
    pub jump: bool,
    pub sprint: bool,
    pub sneak: bool,
    /// Yaw and pitch change this frame (radians).
    pub turn: f32,
    pub tilt: f32,
    pub act: Option<Act>,
}

/// What happened during one physics frame. The agent loop adds decision indices and turns these into
/// `facts::Fact`s.
#[derive(Clone, Debug, PartialEq)]
pub enum BodyEvent {
    Edit {
        cell: Cell,
        kind: EditKind,
        before: Material,
        after: Material,
        outcome: Outcome,
    },
    Tool {
        cell: Cell,
        cell_before: Material,
        tool_before: Material,
        cell_after: Material,
        tool_after: Material,
        reacted: bool,
    },
    /// Someone else's edit or a reaction commit within `radius` of the agent.
    Mutation {
        cell: Cell,
        before: Material,
        after: Material,
        distance: f32,
    },
    Inventory {
        material: Material,
        delta: i32,
    },
    Refused(Refusal),
    SnappedBack,
    Disconnected {
        reason: String,
    },
}

/// One player in the world.
pub trait Body: Send {
    /// Fill `out` with what the player knows now; `rays` are sampled up to `range` metres.
    fn sense(&mut self, rays: &[RayDir], range: f32, radius: f32, out: &mut Snapshot);
    /// Advance one physics frame of `dt` seconds with `controls`, appending what happened to `events`.
    /// Mutations are reported only within `radius` metres.
    fn step(&mut self, controls: &Controls, dt: f32, radius: f32, events: &mut Vec<BodyEvent>);
    fn alive(&self) -> bool;
}

/// Makes bodies: joins the server as a new player. Dropping a body leaves; joining again is the reset
/// (the server scatters each join around spawn and the client-side inventory starts empty).
pub trait Connect: Send + Sync {
    fn connect(&self, name: &str) -> Result<Box<dyn Body>, String>;
}
