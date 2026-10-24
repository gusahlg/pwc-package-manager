// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! One agent's episode loop, generic over a [`Body`].
//!
//! Physics frames run at `physics_hz`. Decisions run at the rule set's rate. The body is asked
//! what it knows, the network picks a legal action, and every body event becomes a fact for the
//! rules. Reconnects back off. The episode seed is mixed from the run seed, the agent index and
//! the episode id.

use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};

use crate::actions::{self, Action, Cooldowns, HAND_SLOTS, HandTable};
use crate::archive::Candidate;
use crate::body::{Body, BodyEvent, CELL_METRES, Connect, Controls, RayDir, Snapshot};
use crate::discover::Discoveries;
use crate::experience::{Evaluation, Trajectory};
use crate::facts::{Cell, Decision, EditKind, EndReason, Fact, Material, Outcome, Pose};
use crate::nn::{self, NetSpec, Scratch};
use crate::rng::{self, Rng};
use crate::rules::Episode;
use crate::senses;

/// What the swarm offers one agent thread. Methods take `&self` and lock internally.
pub trait Drive: Send + Sync {
    fn stopping(&self) -> bool;
    fn paused(&self) -> bool;
    fn retired(&self, index: u32) -> bool;
    fn begin_episode(&self, agent: &str) -> u64;
    fn open_episode(&self) -> Box<dyn Episode>;
    fn candidate(&self) -> Option<Candidate>;
    fn submit(&self, eval: Evaluation);
    fn discoveries(&self) -> &Discoveries;
}

/// Fixed settings for one named agent.
pub struct Launch {
    pub index: u32,
    pub name: String,
    pub seed: u64,
    pub physics_hz: f32,
    pub decide_hz: f32,
    pub decisions: u32,
    pub temperature: f32,
    pub reset_every: u32,
    pub paced: bool,
    pub greedy: bool,
    pub spec: NetSpec,
}

/// Seconds to wait before the next join attempt: 1, 2, 4, 8, 16, then 30.
pub fn backoff_secs(attempt: u32) -> f64 {
    if attempt >= 5 {
        30.0
    } else {
        (1u64 << attempt) as f64
    }
}

/// Join, play episodes, and reconnect after a loss or every `reset_every` episodes.
pub fn serve(connect: &dyn Connect, launch: &Launch, drive: &dyn Drive) {
    let mut since_reset = 0u32;
    let mut failures = 0u32;
    let mut body: Option<Box<dyn Body>> = None;
    loop {
        if drive.stopping() || drive.retired(launch.index) {
            break;
        }
        if body.is_none() {
            match connect.connect(&launch.name) {
                Ok(joined) => {
                    body = Some(joined);
                    failures = 0;
                }
                Err(err) => {
                    let wait = backoff_secs(failures);
                    eprintln!(
                        "{}: join failed ({err}), retrying in {wait:.0}s",
                        launch.name
                    );
                    failures = failures.saturating_add(1);
                    if !sleep_secs(wait, drive, launch.index) {
                        break;
                    }
                    continue;
                }
            }
        }
        let Some(candidate) = drive.candidate() else {
            if !sleep_secs(0.05, drive, launch.index) {
                break;
            }
            continue;
        };
        let episode_id = drive.begin_episode(&launch.name);
        let reason = run_episode(
            body.as_deref_mut().unwrap(),
            launch,
            drive,
            candidate,
            episode_id,
        );
        since_reset += 1;
        let reset = matches!(reason, EndReason::Lost | EndReason::Disconnected)
            || since_reset >= launch.reset_every.max(1);
        if reset {
            body = None;
            since_reset = 0;
        }
        if reason == EndReason::Stopped {
            break;
        }
    }
}

fn sleep_secs(secs: f64, drive: &dyn Drive, index: u32) -> bool {
    let slice = Duration::from_millis(20);
    let mut left = Duration::from_secs_f64(secs.max(0.0));
    while !left.is_zero() {
        if drive.stopping() || drive.retired(index) {
            return false;
        }
        let step = slice.min(left);
        thread::sleep(step);
        left = left.saturating_sub(step);
    }
    !(drive.stopping() || drive.retired(index))
}

struct Loop {
    snap: Snapshot,
    events: Vec<BodyEvent>,
    obs: Vec<f32>,
    legal: Vec<bool>,
    memory: Vec<f32>,
    scratch: Scratch,
    rays: [RayDir; senses::RAY_COUNT],
    hands: HandTable,
    known: Vec<Material>,
    cd: Cooldowns,
    held: Controls,
    prev_action: u16,
    rng: Rng,
    traj: Trajectory,
    pending_edit: HashMap<Cell, u32>,
    pending_tool: HashMap<Cell, u32>,
    pending_place: Option<u32>,
    confirmed: Vec<(Cell, f32)>,
    link_lost: bool,
    dt: f32,
    period: f32,
    sim: f32,
    t0: f32,
    hold_until: f32,
    budget: u32,
    decisions: u32,
    lost_for: f32,
    wall: Instant,
    frame: Duration,
}

fn run_episode(
    body: &mut dyn Body,
    launch: &Launch,
    drive: &dyn Drive,
    candidate: Candidate,
    episode_id: u64,
) -> EndReason {
    let episode_seed = rng::mix(&[launch.seed, launch.index as u64, episode_id]);
    let obs_len = senses::obs_len();
    let budget = launch.decisions.max(1);
    let mut traj = Trajectory::empty(episode_seed, obs_len, candidate.genome.id);
    traj.obs.reserve(budget as usize * obs_len);
    traj.masks.reserve(budget as usize);
    traj.actions.reserve(budget as usize);
    traj.logp.reserve(budget as usize);
    let dt = 1.0 / launch.physics_hz.max(1.0);
    let mut st = Loop {
        snap: Snapshot::default(),
        events: Vec::new(),
        obs: vec![0.0; obs_len],
        legal: vec![false; actions::ACTION_COUNT],
        memory: vec![0.0; launch.spec.memory as usize],
        scratch: Scratch::new(&launch.spec),
        rays: senses::ray_table(),
        hands: HandTable::default(),
        known: Vec::new(),
        cd: Cooldowns::default(),
        held: Controls::default(),
        prev_action: u16::MAX,
        rng: Rng::from_seed(episode_seed),
        traj,
        pending_edit: HashMap::new(),
        pending_tool: HashMap::new(),
        pending_place: None,
        confirmed: Vec::new(),
        link_lost: false,
        dt,
        period: 1.0 / launch.decide_hz.max(1.0),
        sim: 0.0,
        t0: 0.0,
        hold_until: f32::MAX,
        budget,
        decisions: 0,
        lost_for: 0.0,
        wall: Instant::now(),
        frame: Duration::from_secs_f64(dt as f64),
    };
    drive.discoveries().set_context(&launch.name, episode_id);
    let mut rules = drive.open_episode();
    let reason = match warmup(body, launch, drive, &mut st) {
        Some(reason) => reason,
        None => {
            begin_rules(body, &mut st, &mut *rules, drive);
            play(body, &mut st, &mut *rules, launch, drive, &candidate)
        }
    };
    close(drive, launch, candidate, episode_id, rules, st, reason);
    reason
}

fn begin_rules(body: &mut dyn Body, st: &mut Loop, rules: &mut dyn Episode, drive: &dyn Drive) {
    body.sense(
        &st.rays,
        senses::RAY_RANGE,
        senses::SENSE_RADIUS,
        &mut st.snap,
    );
    remember_inv(&mut st.known, &st.snap);
    st.hands.begin(&st.snap.inventory);
    rules.observe(
        &Fact::Start {
            pose: pose_of(&st.snap),
        },
        drive.discoveries(),
    );
}

fn play(
    body: &mut dyn Body,
    st: &mut Loop,
    rules: &mut dyn Episode,
    launch: &Launch,
    drive: &dyn Drive,
    candidate: &Candidate,
) -> EndReason {
    st.t0 = st.sim;
    loop {
        if let Some(reason) = halt(drive, launch.index, body) {
            return reason;
        }
        if drive.paused() {
            let idle = Controls::default();
            step_body(body, &idle, st);
            apply_events(st, rules, drive, st.decisions.saturating_sub(1));
            if st.link_lost {
                return EndReason::Disconnected;
            }
            pace(launch.paced, st.frame, &mut st.wall);
            continue;
        }
        let due =
            st.decisions < st.budget && st.sim + 1e-4 >= st.t0 + st.decisions as f32 * st.period;
        if due {
            decide(body, st, rules, launch, drive, candidate);
            st.decisions += 1;
            if st.decisions == st.budget {
                st.hold_until = st.sim + st.period;
            }
        } else {
            hold_frame(body, st, rules, drive);
        }
        if st.link_lost {
            return EndReason::Disconnected;
        }
        st.sim += st.dt;
        if lost_now(st) {
            return EndReason::Lost;
        }
        if st.decisions >= st.budget && st.sim + 1e-4 >= st.hold_until {
            return EndReason::TimeLimit;
        }
        pace(launch.paced, st.frame, &mut st.wall);
    }
}

fn decide(
    body: &mut dyn Body,
    st: &mut Loop,
    rules: &mut dyn Episode,
    launch: &Launch,
    drive: &dyn Drive,
    candidate: &Candidate,
) {
    let step = st.decisions;
    body.sense(
        &st.rays,
        senses::RAY_RANGE,
        senses::SENSE_RADIUS,
        &mut st.snap,
    );
    remember_inv(&mut st.known, &st.snap);
    st.hands.sync(&st.snap.inventory);
    let slots = hand_view(&st.hands, &st.known);
    senses::fill(&st.snap, &slots, st.prev_action, &st.memory, &mut st.obs);
    st.cd.now = st.sim;
    let target = st.snap.target.is_some();
    let placeable = st.snap.target.is_some_and(|t| t.placeable);
    actions::mask(&mut st.legal, target, placeable, &st.hands, &st.cd);
    let (action, logp) = choose(st, launch, candidate);
    let kind = actions::kind_of(action);
    note_issue(st, action, step);
    if is_world(action) {
        st.cd.on_world_act(kind);
    }
    let n = senses::obs_len();
    st.traj.obs.extend_from_slice(&st.obs[..n]);
    st.traj.masks.push(actions::mask_bits(&st.legal));
    st.traj.actions.push(action);
    st.traj.logp.push(logp);
    rules.observe(
        &Fact::Decision(Decision {
            step,
            pose: pose_of(&st.snap),
            action,
            kind,
            legal: actions::legal_count(&st.legal),
            peers_near: st.snap.peers.len().min(u16::MAX as usize) as u16,
            nearest_peer: st.snap.peers.first().map(|p| p.distance),
        }),
        drive.discoveries(),
    );
    st.prev_action = action;
    st.held = actions::controls(action, st.dt, &st.hands, true);
    let held = st.held;
    step_body(body, &held, st);
    st.held.act = None;
    apply_events(st, rules, drive, step);
}

fn choose(st: &mut Loop, launch: &Launch, candidate: &Candidate) -> (u16, f32) {
    // Copy off the scratch borrow before sampling, which needs `&mut` of the episode RNG.
    let n_act = (launch.spec.actions as usize).min(actions::ACTION_COUNT);
    let n_mem = (launch.spec.memory as usize).min(senses::MEMORY);
    let mut logits = [0.0f32; actions::ACTION_COUNT];
    let mut memory = [0.0f32; senses::MEMORY];
    {
        let (lg, mem) = nn::forward(
            &candidate.genome.params,
            &launch.spec,
            &st.obs,
            &mut st.scratch,
        );
        logits[..n_act].copy_from_slice(&lg[..n_act]);
        memory[..n_mem].copy_from_slice(&mem[..n_mem]);
    }
    for (slot, value) in st.memory.iter_mut().zip(memory[..n_mem].iter()) {
        *slot = *value;
    }
    if launch.greedy {
        (nn::greedy(&logits[..n_act], &st.legal), 0.0)
    } else {
        nn::sample(&logits[..n_act], &st.legal, launch.temperature, &mut st.rng)
    }
}

fn hold_frame(body: &mut dyn Body, st: &mut Loop, rules: &mut dyn Episode, drive: &dyn Drive) {
    body.sense(&[], 0.0, senses::SENSE_RADIUS, &mut st.snap);
    let step = st.decisions.saturating_sub(1);
    let held = st.held;
    step_body(body, &held, st);
    apply_events(st, rules, drive, step);
}

fn step_body(body: &mut dyn Body, controls: &Controls, st: &mut Loop) {
    st.events.clear();
    st.link_lost = false;
    body.step(controls, st.dt, senses::SENSE_RADIUS, &mut st.events);
    if !body.alive() {
        st.link_lost = true;
    }
}

fn apply_events(st: &mut Loop, rules: &mut dyn Episode, drive: &dyn Drive, step: u32) {
    let events = std::mem::take(&mut st.events);
    for ev in events {
        let fact = match ev {
            BodyEvent::Edit {
                cell,
                kind,
                before,
                after,
                outcome,
            } => Some(edit_fact(st, cell, kind, before, after, outcome, step)),
            BodyEvent::Tool {
                cell,
                cell_before,
                tool_before,
                cell_after,
                tool_after,
                reacted,
            } => {
                let issued = st.pending_tool.remove(&cell).unwrap_or(step);
                Some(Fact::Tool {
                    step: issued,
                    cell,
                    cell_before,
                    tool_before,
                    cell_after,
                    tool_after,
                    reacted,
                })
            }
            BodyEvent::Mutation {
                cell,
                before,
                after,
                distance,
            } => Some(Fact::Mutation {
                cell,
                before,
                after,
                distance,
                since_own_edit: since_own(&st.confirmed, cell, st.sim),
            }),
            BodyEvent::Inventory { material, delta } => {
                st.hands.note_gained(&material, delta);
                remember(&mut st.known, material);
                Some(Fact::Inventory { material, delta })
            }
            BodyEvent::Refused(reason) => Some(Fact::Refused { step, reason }),
            BodyEvent::SnappedBack => Some(Fact::SnappedBack),
            BodyEvent::Disconnected { .. } => {
                st.link_lost = true;
                None
            }
        };
        if let Some(fact) = fact {
            rules.observe(&fact, drive.discoveries());
        }
    }
}

fn edit_fact(
    st: &mut Loop,
    cell: Cell,
    kind: EditKind,
    before: Material,
    after: Material,
    outcome: Outcome,
    step: u32,
) -> Fact {
    let issued = st
        .pending_edit
        .get(&cell)
        .copied()
        .or(st.pending_place)
        .unwrap_or(step);
    if outcome == Outcome::Predicted {
        st.pending_edit.insert(cell, issued);
        if kind == EditKind::Placed {
            st.pending_place = None;
        }
    }
    if matches!(outcome, Outcome::Confirmed | Outcome::Rejected) {
        st.pending_edit.remove(&cell);
    }
    if outcome == Outcome::Confirmed {
        st.confirmed.push((cell, st.sim));
    }
    Fact::Edit {
        step: issued,
        cell,
        kind,
        before,
        after,
        outcome,
    }
}

fn note_issue(st: &mut Loop, action: u16, step: u32) {
    match actions::decode(action) {
        Action::Break => {
            if let Some(t) = st.snap.target {
                st.pending_edit.insert(t.cell, step);
            }
        }
        Action::Place(_) => st.pending_place = Some(step),
        Action::UseTool(_) => {
            if let Some(t) = st.snap.target {
                st.pending_tool.insert(t.cell, step);
            }
        }
        _ => {}
    }
}

fn is_world(action: u16) -> bool {
    matches!(
        actions::decode(action),
        Action::Break | Action::Place(_) | Action::UseTool(_) | Action::Swing
    )
}

fn since_own(confirmed: &[(Cell, f32)], cell: Cell, now: f32) -> Option<f32> {
    let mut best: Option<f32> = None;
    for &(other, at) in confirmed {
        if cell_metres(other, cell) < 6.0 {
            let age = (now - at).max(0.0);
            best = Some(best.map_or(age, |b| b.min(age)));
        }
    }
    best
}

fn cell_metres(a: Cell, b: Cell) -> f32 {
    let dx = (a.0 - b.0) as f32;
    let dy = (a.1 - b.1) as f32;
    let dz = (a.2 - b.2) as f32;
    (dx * dx + dy * dy + dz * dz).sqrt() * CELL_METRES
}

fn lost_now(st: &mut Loop) -> bool {
    let falling = match st.snap.pose {
        Some(p) => !p.grounded && p.up.is_none(),
        None => true,
    };
    if falling {
        st.lost_for += st.dt;
    } else {
        st.lost_for = 0.0;
    }
    st.lost_for >= 10.0
}

fn pose_of(snap: &Snapshot) -> Pose {
    snap.pose.unwrap_or(Pose {
        eye: [0.0; 3],
        velocity: [0.0; 3],
        up: None,
        grounded: false,
        in_air: false,
    })
}

fn remember(known: &mut Vec<Material>, mat: Material) {
    if mat.is_air() {
        return;
    }
    if let Some(slot) = known.iter_mut().find(|m| m.key == mat.key) {
        *slot = mat;
    } else {
        known.push(mat);
    }
}

fn remember_inv(known: &mut Vec<Material>, snap: &Snapshot) {
    for (mat, count) in &snap.inventory {
        if *count > 0 {
            remember(known, *mat);
        }
    }
}

fn hand_view(hands: &HandTable, known: &[Material]) -> [(Material, u32); HAND_SLOTS] {
    let mut out = [(Material::AIR, 0); HAND_SLOTS];
    for (slot, (&key, count)) in out.iter_mut().zip(hands.keys.iter().zip(hands.counts)) {
        let mat = known
            .iter()
            .copied()
            .find(|m| m.key == key)
            .unwrap_or(Material::AIR);
        *slot = (mat, count);
    }
    out
}

fn warmup(
    body: &mut dyn Body,
    launch: &Launch,
    drive: &dyn Drive,
    st: &mut Loop,
) -> Option<EndReason> {
    let idle = Controls::default();
    while st.sim < 3.0 {
        if let Some(reason) = halt(drive, launch.index, body) {
            return Some(reason);
        }
        if drive.paused() {
            step_body(body, &idle, st);
            pace(launch.paced, st.frame, &mut st.wall);
            continue;
        }
        body.sense(&[], 0.0, senses::SENSE_RADIUS, &mut st.snap);
        if st.snap.pose.is_some_and(|p| p.grounded) {
            break;
        }
        step_body(body, &idle, st);
        st.sim += st.dt;
        pace(launch.paced, st.frame, &mut st.wall);
        if st.link_lost {
            return Some(EndReason::Disconnected);
        }
    }
    None
}

fn halt(drive: &dyn Drive, index: u32, body: &dyn Body) -> Option<EndReason> {
    if drive.stopping() || drive.retired(index) {
        Some(EndReason::Stopped)
    } else if !body.alive() {
        Some(EndReason::Disconnected)
    } else {
        None
    }
}

fn pace(paced: bool, frame: Duration, next: &mut Instant) {
    if !paced {
        return;
    }
    let now = Instant::now();
    if *next + frame * 3 < now {
        *next = now;
    } else if *next > now {
        thread::sleep(*next - now);
    }
    *next += frame;
}

fn close(
    drive: &dyn Drive,
    launch: &Launch,
    candidate: Candidate,
    episode_id: u64,
    mut rules: Box<dyn Episode>,
    st: Loop,
    reason: EndReason,
) {
    drive.discoveries().set_context(&launch.name, episode_id);
    rules.observe(&Fact::End { reason }, drive.discoveries());
    let verdict = rules.finish(drive.discoveries());
    let mut traj = st.traj;
    let n = traj.actions.len();
    traj.rewards = verdict.rewards.clone();
    if traj.rewards.len() < n {
        traj.rewards.resize(n, 0.0);
    } else {
        traj.rewards.truncate(n);
    }
    traj.descriptor.clone_from(&verdict.descriptor);
    traj.genome_id = candidate.genome.id;
    traj.truncated = matches!(reason, EndReason::TimeLimit | EndReason::Stopped);
    traj.terminal = matches!(reason, EndReason::Lost | EndReason::Disconnected);
    drive.submit(Evaluation {
        candidate,
        verdict,
        trajectory: traj,
        end_reason: reason,
        decisions: st.decisions,
        episode: episode_id,
        agent: launch.name.clone(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_then_sits_at_thirty_seconds() {
        assert!((backoff_secs(0) - 1.0).abs() < 1e-9);
        assert!((backoff_secs(1) - 2.0).abs() < 1e-9);
        assert!((backoff_secs(2) - 4.0).abs() < 1e-9);
        assert!((backoff_secs(3) - 8.0).abs() < 1e-9);
        assert!((backoff_secs(4) - 16.0).abs() < 1e-9);
        assert!((backoff_secs(5) - 30.0).abs() < 1e-9);
        assert!((backoff_secs(9) - 30.0).abs() < 1e-9);
    }

    #[test]
    fn a_confirmed_edit_within_six_metres_sets_since_own_edit() {
        let confirmed = vec![((0, 0, 0), 1.0)];
        let near = since_own(&confirmed, (1, 0, 0), 1.4).unwrap();
        assert!(near < 2.0);
        assert!(since_own(&confirmed, (20, 0, 0), 1.4).is_none());
    }
}
