// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The swarm: one thread per agent, one shared archive, and the control loop.
//!
//! Challengers are handed out before re-evaluations, and both before a new child. A checkpoint
//! is an atomic rename. Ctrl-C is not caught (there is no signal crate); `quit` or EOF on stdin
//! stops the run, and a hard kill loses at most one checkpoint interval.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::actions;
use crate::agent::{self, Drive, Launch};
use crate::archive::{Archive, Candidate, CandidateKind, Partition, Settle};
use crate::body::Connect;
use crate::discover::Discoveries;
use crate::emitter::{self, Scheduler};
use crate::experience::{Evaluation, Experience};
use crate::facts::EndReason;
use crate::nn::NetSpec;
use crate::rng::Rng;
use crate::rules::{self, Axis, Episode, Rules};
use crate::store::{self, Status};

/// Where the agents connect. The CLI fills this; tests pass a fake.
#[derive(Clone, Debug)]
pub struct ServerArgs {
    pub host: String,
    pub port: u16,
    pub password: String,
}

/// One greedy playback of a stored elite. Nothing is written back.
#[derive(Clone, Debug)]
pub struct Replay {
    pub cell: Option<usize>,
    pub episodes: u32,
}

/// Everything `swarm run` and `swarm replay` need. Test-only fields are `decisions`, `paced`
/// and the three intervals.
pub struct Config {
    pub server: ServerArgs,
    pub agents: u32,
    pub rules: String,
    pub run_dir: PathBuf,
    pub physics_hz: f32,
    pub temperature: f32,
    pub sigma: f32,
    pub reeval: f32,
    pub reset_every: u32,
    pub experience: usize,
    pub seed: u64,
    pub name: String,
    pub minutes: Option<f32>,
    pub fresh: bool,
    pub console: bool,
    pub decisions: Option<u32>,
    pub paced: bool,
    pub poll_every: Duration,
    pub status_every: Duration,
    pub checkpoint_every: Duration,
    pub replay: Option<Replay>,
}

impl Config {
    pub fn new(server: ServerArgs, rules: &str, run_dir: PathBuf) -> Self {
        Self {
            server,
            agents: 8,
            rules: rules.to_string(),
            run_dir,
            physics_hz: 30.0,
            temperature: 0.4,
            sigma: 0.03,
            reeval: 0.1,
            reset_every: 5,
            experience: 64,
            seed: 1,
            name: "swarm".into(),
            minutes: None,
            fresh: false,
            console: true,
            decisions: None,
            paced: true,
            poll_every: Duration::from_secs(1),
            status_every: Duration::from_secs(5),
            checkpoint_every: Duration::from_secs(60),
            replay: None,
        }
    }
}

const EPISODE_HEADER: &str =
    "episode\tagent\temitter\tkind\tfitness\tbonus\tdescriptor\tcell\toutcome\tend\tactions";
const MAX_AGENTS: u32 = 128;

/// Run until quit, the minute limit, or (for replay) the requested episodes. `0` is a clean stop.
pub fn launch(cfg: Config, connect: Box<dyn Connect>) -> i32 {
    if cfg.replay.is_some() {
        return replay(&cfg, connect);
    }
    match run(cfg, connect) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("swarm: {err}");
            1
        }
    }
}

/// Why a resumed archive cannot be used. `None` means it matches.
pub fn archive_conflict(
    archive: &Archive,
    rules_name: &str,
    axes: &[Axis],
    spec: &NetSpec,
) -> Option<String> {
    if archive.rules != rules_name {
        return Some(format!(
            "archive rules are '{}', this run uses '{rules_name}'. Pass --fresh to replace it, or choose another --run-dir.",
            archive.rules
        ));
    }
    if !archive.axes_compatible(axes) {
        return Some(
            "archive behaviour axes do not match this rule set. Pass --fresh to replace it, or choose another --run-dir.".into(),
        );
    }
    if !archive.spec.compatible(spec) {
        return Some(
            "archive network shape does not match this build. Pass --fresh to replace it, or choose another --run-dir.".into(),
        );
    }
    None
}

/// `<name><NN>`, at most 16 characters. Index 0 is `01`.
pub fn agent_name(base: &str, index: u32) -> String {
    let n = index.saturating_add(1);
    let suffix = if n < 100 {
        format!("{n:02}")
    } else {
        format!("{n}")
    };
    let room = 16usize.saturating_sub(suffix.chars().count());
    let stem: String = base.chars().take(room).collect();
    let stem = if stem.is_empty() {
        "s".to_string()
    } else {
        stem
    };
    let mut name = format!("{stem}{suffix}");
    if name.chars().count() > 16 {
        name = name.chars().take(16).collect();
    }
    name
}

struct Settings {
    spec: NetSpec,
    seed: u64,
    physics_hz: f32,
    decide_hz: f32,
    decisions: u32,
    temperature: f32,
    reset_every: u32,
    paced: bool,
    greedy: bool,
    name: String,
    reeval: f64,
}

struct Shared {
    archive: Mutex<Archive>,
    scheduler: Mutex<Scheduler>,
    rng: Mutex<Rng>,
    discoveries: Discoveries,
    experience: Mutex<Experience>,
    rules: Arc<dyn Rules>,
    settings: Settings,
    paused: AtomicBool,
    stopping: AtomicBool,
    target: AtomicU32,
    running: AtomicU32,
    next_episode: AtomicU64,
    session_episodes: AtomicU64,
    started: Instant,
    dir: PathBuf,
    save_mu: Mutex<()>,
    log_mu: Mutex<()>,
    handles: Mutex<Vec<Option<JoinHandle<()>>>>,
}

fn run(cfg: Config, connect: Box<dyn Connect>) -> Result<(), String> {
    let rules =
        rules::by_name(&cfg.rules).ok_or_else(|| format!("unknown rules '{}'", cfg.rules))?;
    let loaded = prepare(&cfg, rules.as_ref())?;
    let (archive, rng, discoveries) = loaded;
    let spec = archive.spec.clone();
    let seed = archive.run_seed;
    let next = archive.episodes;
    let decide = rules.episode();
    let settings = Settings {
        spec: spec.clone(),
        seed,
        physics_hz: cfg.physics_hz,
        decide_hz: decide.decide_hz,
        decisions: cfg.decisions.unwrap_or(decide.decisions),
        temperature: cfg.temperature,
        reset_every: cfg.reset_every,
        paced: cfg.paced,
        greedy: false,
        name: cfg.name.clone(),
        reeval: cfg.reeval as f64,
    };
    let shared = Arc::new(Shared {
        archive: Mutex::new(archive),
        scheduler: Mutex::new(Scheduler::new(spec, cfg.sigma)),
        rng: Mutex::new(rng),
        discoveries,
        experience: Mutex::new(Experience::new(cfg.experience)),
        rules: Arc::from(rules),
        settings,
        paused: AtomicBool::new(false),
        stopping: AtomicBool::new(false),
        target: AtomicU32::new(cfg.agents.min(MAX_AGENTS)),
        running: AtomicU32::new(0),
        next_episode: AtomicU64::new(next),
        session_episodes: AtomicU64::new(0),
        started: Instant::now(),
        dir: cfg.run_dir.clone(),
        save_mu: Mutex::new(()),
        log_mu: Mutex::new(()),
        handles: Mutex::new(Vec::new()),
    });
    println!(
        "exploraswarm: {} agents, rules {}, directory {}",
        shared.target.load(Ordering::Relaxed),
        shared.rules.name(),
        shared.dir.display()
    );
    let connect: Arc<dyn Connect> = Arc::from(connect);
    let console = if cfg.console {
        Some(spawn_console(Arc::clone(&shared)))
    } else {
        None
    };
    let mut next_status = Instant::now();
    let mut next_ckpt = Instant::now() + cfg.checkpoint_every;
    let deadline = cfg
        .minutes
        .map(|m| Instant::now() + Duration::from_secs_f64(f64::from(m) * 60.0));
    loop {
        if shared.stopping.load(Ordering::Relaxed) {
            break;
        }
        if deadline.is_some_and(|t| Instant::now() >= t) {
            shared.stopping.store(true, Ordering::Relaxed);
            break;
        }
        read_control(&shared);
        if shared.stopping.load(Ordering::Relaxed) {
            break;
        }
        reconcile(&shared, &connect);
        if Instant::now() >= next_status {
            if let Err(err) = write_status(&shared) {
                eprintln!("status: {err}");
            }
            next_status = Instant::now() + cfg.status_every;
        }
        if Instant::now() >= next_ckpt {
            if let Err(err) = checkpoint(&shared) {
                eprintln!("checkpoint: {err}");
            }
            next_ckpt = Instant::now() + cfg.checkpoint_every;
        }
        thread::sleep(cfg.poll_every);
    }
    if let Some(handle) = console {
        let _ = handle.join();
    }
    drain(&shared);
    let saved = checkpoint(&shared);
    let _ = write_status(&shared);
    println!(
        "exploraswarm: stopped after {} episodes this process ({} in the archive)",
        shared.session_episodes.load(Ordering::Relaxed),
        lock(&shared.archive).episodes
    );
    saved
}

fn prepare(cfg: &Config, rules: &dyn Rules) -> Result<(Archive, Rng, Discoveries), String> {
    fs::create_dir_all(&cfg.run_dir)
        .map_err(|e| format!("creating {}: {e}", cfg.run_dir.display()))?;
    if cfg.fresh {
        for name in [
            store::ARCHIVE_FILE,
            store::DISCOVERIES_FILE,
            store::EPISODES_LOG,
            store::DISCOVERY_LOG,
            store::STATUS_FILE,
        ] {
            let path = cfg.run_dir.join(name);
            if path.exists() {
                fs::remove_file(&path).map_err(|e| format!("removing {}: {e}", path.display()))?;
            }
        }
    }
    let spec = NetSpec::standard();
    let archive_path = cfg.run_dir.join(store::ARCHIVE_FILE);
    let (archive, rng) = if archive_path.exists() {
        let (archive, state) = store::load_archive(&archive_path)?;
        if let Some(err) = archive_conflict(&archive, rules.name(), rules.axes(), &spec) {
            return Err(err);
        }
        (archive, Rng::from_state(state))
    } else {
        (
            Archive::new(rules.name(), rules.axes(), spec, cfg.seed),
            Rng::from_seed(cfg.seed),
        )
    };
    let discoveries = Discoveries::new();
    let disc_path = cfg.run_dir.join(store::DISCOVERIES_FILE);
    if disc_path.exists() && !cfg.fresh {
        discoveries.import(store::load_discoveries(&disc_path)?);
    }
    Ok((archive, rng, discoveries))
}

fn replay(cfg: &Config, connect: Box<dyn Connect>) -> i32 {
    let Some(spec) = cfg.replay.clone() else {
        return 1;
    };
    let rules = match rules::by_name(&cfg.rules) {
        Some(rules) => rules,
        None => {
            eprintln!("swarm: unknown rules '{}'", cfg.rules);
            return 1;
        }
    };
    let path = cfg.run_dir.join(store::ARCHIVE_FILE);
    let archive = match store::load_archive(&path) {
        Ok((archive, _)) => archive,
        Err(err) => {
            eprintln!("swarm: {err}");
            return 1;
        }
    };
    if let Some(err) = archive_conflict(&archive, rules.name(), rules.axes(), &NetSpec::standard())
    {
        eprintln!("swarm: {err}");
        return 1;
    }
    let cell = match spec.cell {
        Some(cell) => cell,
        None => match best_cell(&archive) {
            Some(cell) => cell,
            None => {
                eprintln!("swarm: the archive has no elites to replay");
                return 1;
            }
        },
    };
    let Some(elite) = archive.elite(cell) else {
        eprintln!("swarm: cell {cell} is empty");
        return 1;
    };
    println!(
        "replay: cell {cell}, fitness {:.3}, {} episodes, no archive writes",
        elite.fitness_mean, spec.episodes
    );
    let genome = elite.genome.clone();
    let drive = ReplayDrive {
        rules: Arc::from(rules),
        genome,
        discoveries: Discoveries::new(),
        left: AtomicU32::new(spec.episodes.max(1)),
        stopping: AtomicBool::new(false),
        next_id: AtomicU64::new(1),
    };
    let episode = drive.rules.episode();
    let launch = Launch {
        index: 0,
        name: agent_name(&cfg.name, 0),
        seed: archive.run_seed,
        physics_hz: cfg.physics_hz,
        decide_hz: episode.decide_hz,
        decisions: cfg.decisions.unwrap_or(episode.decisions),
        temperature: cfg.temperature,
        reset_every: u32::MAX,
        paced: cfg.paced,
        greedy: true,
        spec: archive.spec.clone(),
    };
    agent::serve(connect.as_ref(), &launch, &drive);
    0
}

fn best_cell(archive: &Archive) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, slot) in archive.cells.iter().enumerate() {
        let Some(elite) = slot else { continue };
        let replace = match best {
            None => true,
            Some((bi, bf)) => {
                elite.fitness_mean.total_cmp(&bf) == std::cmp::Ordering::Greater
                    || (elite.fitness_mean == bf && i < bi)
            }
        };
        if replace {
            best = Some((i, elite.fitness_mean));
        }
    }
    best.map(|b| b.0)
}

struct ReplayDrive {
    rules: Arc<dyn Rules>,
    genome: crate::archive::Genome,
    discoveries: Discoveries,
    left: AtomicU32,
    stopping: AtomicBool,
    next_id: AtomicU64,
}

impl Drive for ReplayDrive {
    fn stopping(&self) -> bool {
        self.stopping.load(Ordering::Relaxed)
    }
    fn paused(&self) -> bool {
        false
    }
    fn retired(&self, _index: u32) -> bool {
        false
    }
    fn begin_episode(&self, agent: &str) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.discoveries.set_context(agent, id);
        id
    }
    fn open_episode(&self) -> Box<dyn Episode> {
        self.rules.begin()
    }
    fn candidate(&self) -> Option<Candidate> {
        if self.left.load(Ordering::Relaxed) == 0 {
            self.stopping.store(true, Ordering::Relaxed);
            return None;
        }
        Some(Candidate {
            emitter: self.genome.emitter,
            genome: self.genome.clone(),
            kind: CandidateKind::New,
        })
    }
    fn submit(&self, eval: Evaluation) {
        println!(
            "replay\t{}\t{:.4}\t{:.4}\t{}\t{}",
            eval.episode,
            eval.verdict.fitness,
            eval.verdict.bonus,
            end_name(eval.end_reason),
            fmt_desc(&eval.verdict.descriptor)
        );
        let left = self.left.fetch_sub(1, Ordering::Relaxed);
        if left <= 1 {
            self.stopping.store(true, Ordering::Relaxed);
        }
    }
    fn discoveries(&self) -> &Discoveries {
        &self.discoveries
    }
}

impl Drive for Shared {
    fn stopping(&self) -> bool {
        self.stopping.load(Ordering::Relaxed)
    }
    fn paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }
    fn retired(&self, index: u32) -> bool {
        index >= self.target.load(Ordering::Relaxed)
    }
    fn begin_episode(&self, agent: &str) -> u64 {
        let id = self.next_episode.fetch_add(1, Ordering::Relaxed);
        self.discoveries.set_context(agent, id);
        id
    }
    fn open_episode(&self) -> Box<dyn Episode> {
        self.rules.begin()
    }
    fn candidate(&self) -> Option<Candidate> {
        {
            let mut archive = lock(&self.archive);
            if let Some((genome, cell)) = archive.take_challenger() {
                let emitter = genome.emitter;
                return Some(Candidate {
                    genome,
                    emitter,
                    kind: CandidateKind::Challenge(cell),
                });
            }
        }
        let reeval = {
            let archive = lock(&self.archive);
            let mut rng = lock(&self.rng);
            archive.elite_count() > 0 && rng.chance(self.settings.reeval)
        };
        if reeval {
            let archive = lock(&self.archive);
            if let Some(cell) = archive.pick_reeval() {
                let elite = archive.elite(cell)?;
                let genome = elite.genome.clone();
                let emitter = genome.emitter;
                return Some(Candidate {
                    genome,
                    emitter,
                    kind: CandidateKind::Reevaluate(cell),
                });
            }
        }
        let mut genome = {
            let archive = lock(&self.archive);
            let mut sched = lock(&self.scheduler);
            let mut rng = lock(&self.rng);
            sched.ask(&archive, &mut rng)?
        };
        let mut archive = lock(&self.archive);
        archive.assign_id(&mut genome);
        let emitter = genome.emitter;
        Some(Candidate {
            genome,
            emitter,
            kind: CandidateKind::New,
        })
    }
    fn submit(&self, eval: Evaluation) {
        let (outcome, cell) = {
            let mut archive = lock(&self.archive);
            let settled = if cut_short(eval.end_reason) {
                // Fitness counts are not per decision, so a shortened episode cannot compete for a
                // niche. Its trajectory still goes to experience, marked truncated or terminal.
                if let CandidateKind::Challenge(_) = eval.candidate.kind {
                    archive.release_challenger(eval.candidate.genome.id);
                }
                (
                    Settle::Discarded,
                    archive.grid.cell(&eval.verdict.descriptor),
                )
            } else {
                settle(&mut archive, &eval)
            };
            archive.episodes = archive.episodes.saturating_add(1);
            settled
        };
        {
            let mut sched = lock(&self.scheduler);
            sched.tell(&eval, outcome);
        }
        self.session_episodes.fetch_add(1, Ordering::Relaxed);
        write_episode_line(self, &eval, outcome, cell);
        let traj = eval.trajectory.clone();
        lock(&self.experience).push(traj);
    }
    fn discoveries(&self) -> &Discoveries {
        &self.discoveries
    }
}

/// Ended by `stop`, `ctl agents` or the network rather than by what the agent did.
fn cut_short(reason: EndReason) -> bool {
    matches!(reason, EndReason::Stopped | EndReason::Disconnected)
}

fn settle(archive: &mut Archive, eval: &Evaluation) -> (Settle, usize) {
    let aimed = match eval.candidate.kind {
        CandidateKind::Reevaluate(cell) | CandidateKind::Challenge(cell) => cell,
        CandidateKind::New => archive.grid.cell(&eval.verdict.descriptor),
    };
    let outcome = match eval.candidate.kind {
        CandidateKind::New => archive.offer_new(
            eval.candidate.genome.clone(),
            eval.verdict.fitness,
            eval.verdict.descriptor.clone(),
            eval.verdict.signatures.clone(),
            eval.verdict.bonus,
            eval.episode,
        ),
        CandidateKind::Challenge(_) => archive.resolve_challenge(
            eval.candidate.genome.id,
            eval.verdict.fitness,
            eval.verdict.signatures.clone(),
            eval.verdict.bonus,
            eval.episode,
        ),
        CandidateKind::Reevaluate(cell) => archive.reevaluate(
            cell,
            eval.verdict.fitness,
            &eval.verdict.descriptor,
            eval.episode,
        ),
    };
    (outcome, aimed)
}

fn write_episode_line(shared: &Shared, eval: &Evaluation, outcome: Settle, cell: usize) {
    let _guard = lock(&shared.log_mu);
    let path = shared.dir.join(store::EPISODES_LOG);
    let fresh = fs::metadata(&path).map(|m| m.len() == 0).unwrap_or(true);
    if fresh {
        let _ = store::append_lines(&path, &[EPISODE_HEADER.to_string()]);
    }
    let line = format!(
        "{}\t{}\t{}\t{}\t{:.4}\t{:.4}\t{}\t{}\t{}\t{}\t{}",
        eval.episode,
        eval.agent,
        emitter::name_of(eval.candidate.emitter),
        eval.candidate.kind_name(),
        eval.verdict.fitness,
        eval.verdict.bonus,
        fmt_desc(&eval.verdict.descriptor),
        cell,
        outcome.as_str(),
        end_name(eval.end_reason),
        top_actions(&eval.trajectory.actions)
    );
    if let Err(err) = store::append_lines(&path, &[line]) {
        eprintln!("episodes.log: {err}");
    }
}

/// The three most chosen actions and their shares, e.g. `forward:31%,break:12%,turn-left:9%`.
fn top_actions(actions: &[u16]) -> String {
    let mut counts = [0u32; actions::ACTION_COUNT];
    for &a in actions {
        if let Some(c) = counts.get_mut(a as usize) {
            *c += 1;
        }
    }
    let mut order: Vec<usize> = (0..actions::ACTION_COUNT)
        .filter(|&i| counts[i] > 0)
        .collect();
    order.sort_by_key(|&i| std::cmp::Reverse(counts[i]));
    let total = actions.len().max(1) as f32;
    order
        .iter()
        .take(3)
        .map(|&i| {
            format!(
                "{}:{:.0}%",
                actions::NAMES[i],
                100.0 * counts[i] as f32 / total
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn fmt_desc(values: &[f32]) -> String {
    values
        .iter()
        .map(|v| format!("{v:.4}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn end_name(reason: EndReason) -> &'static str {
    match reason {
        EndReason::TimeLimit => "time",
        EndReason::Lost => "lost",
        EndReason::Disconnected => "disconnected",
        EndReason::Stopped => "stopped",
    }
}

fn checkpoint(shared: &Shared) -> Result<(), String> {
    let _save = lock(&shared.save_mu);
    let (archive, rng) = {
        let archive = lock(&shared.archive);
        let rng = lock(&shared.rng);
        (archive.clone(), rng.state())
    };
    store::save_archive(&shared.dir.join(store::ARCHIVE_FILE), &archive, rng)?;
    let records = shared.discoveries.export();
    store::save_discoveries(&shared.dir.join(store::DISCOVERIES_FILE), &records)?;
    let lines = shared.discoveries.take_log();
    store::append_lines(&shared.dir.join(store::DISCOVERY_LOG), &lines)
}

fn write_status(shared: &Shared) -> Result<(), String> {
    let (episodes, coverage, qd, best, emitters) = {
        let archive = lock(&shared.archive);
        let emitters = emitter_rows(&archive.stats);
        (
            archive.episodes,
            archive.coverage(),
            archive.qd_score(),
            archive.best(),
            emitters,
        )
    };
    let mins = shared.started.elapsed().as_secs_f64() / 60.0;
    let session = shared.session_episodes.load(Ordering::Relaxed);
    let eps = if mins > 1e-6 {
        session as f64 / mins
    } else {
        0.0
    };
    let status = Status {
        agents: shared.running.load(Ordering::Relaxed),
        paused: shared.paused.load(Ordering::Relaxed),
        episodes,
        coverage,
        qd_score: qd,
        best,
        discoveries: shared.discoveries.len() as u64,
        emitters,
        eps_per_min: eps,
        updated_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };
    store::save_status(&shared.dir.join(store::STATUS_FILE), &status)
}

fn emitter_rows(
    stats: &[crate::archive::EmitterStats],
) -> Vec<(String, crate::archive::EmitterStats)> {
    ["random", "gaussian", "isoline"]
        .into_iter()
        .enumerate()
        .map(|(i, name)| (name.to_string(), stats.get(i).copied().unwrap_or_default()))
        .collect()
}

fn read_control(shared: &Shared) {
    let path = shared.dir.join(store::CONTROL_FILE);
    let Ok(text) = fs::read_to_string(&path) else {
        return;
    };
    if text.trim().is_empty() {
        return;
    }
    for line in text.lines() {
        apply_command(shared, line);
    }
    let _ = File::create(&path).and_then(|mut file| file.write_all(b""));
}

fn apply_command(shared: &Shared, line: &str) {
    let mut parts = line.split_whitespace();
    let Some(cmd) = parts.next() else { return };
    match cmd {
        "pause" => {
            shared.paused.store(true, Ordering::Relaxed);
            let _ = write_status(shared);
        }
        "resume" => {
            shared.paused.store(false, Ordering::Relaxed);
            let _ = write_status(shared);
        }
        "status" => {
            let _ = write_status(shared);
        }
        "save" => {
            if let Err(err) = checkpoint(shared) {
                eprintln!("save: {err}");
            }
            let _ = write_status(shared);
        }
        "quit" => shared.stopping.store(true, Ordering::Relaxed),
        "agents" => match parts.next().and_then(|s| s.parse::<u32>().ok()) {
            Some(n) if n <= MAX_AGENTS => {
                shared.target.store(n, Ordering::Relaxed);
                let _ = write_status(shared);
            }
            _ => eprintln!("agents: expected a number from 0 to {MAX_AGENTS}"),
        },
        other => eprintln!("unknown command {other}"),
    }
}

fn reconcile(shared: &Arc<Shared>, connect: &Arc<dyn Connect>) {
    if shared.stopping.load(Ordering::Relaxed) {
        return;
    }
    let mut handles = lock(&shared.handles);
    reap(&mut handles);
    let target = shared.target.load(Ordering::Relaxed) as usize;
    while handles.len() < target {
        handles.push(None);
    }
    for index in 0..target {
        if handles[index].is_none() {
            handles[index] = Some(spawn_agent(shared, connect, index as u32));
        }
    }
}

fn drain(shared: &Arc<Shared>) {
    let start = Instant::now();
    loop {
        {
            let mut handles = lock(&shared.handles);
            reap(&mut handles);
            if handles.iter().all(|slot| slot.is_none()) {
                break;
            }
        }
        if start.elapsed() > Duration::from_secs(120) {
            eprintln!("swarm: agents did not stop within 120s");
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn reap(handles: &mut [Option<JoinHandle<()>>]) {
    for slot in handles.iter_mut() {
        if slot.as_ref().is_some_and(|h| h.is_finished())
            && let Some(handle) = slot.take()
        {
            let _ = handle.join();
        }
    }
}

struct Running(Arc<Shared>);

impl Drop for Running {
    fn drop(&mut self) {
        self.0.running.fetch_sub(1, Ordering::Relaxed);
    }
}

fn spawn_agent(shared: &Arc<Shared>, connect: &Arc<dyn Connect>, index: u32) -> JoinHandle<()> {
    let shared = Arc::clone(shared);
    let connect = Arc::clone(connect);
    thread::Builder::new()
        .name(format!("swarm-{index}"))
        .spawn(move || {
            shared.running.fetch_add(1, Ordering::Relaxed);
            let _guard = Running(Arc::clone(&shared));
            let launch = Launch {
                index,
                name: agent_name(&shared.settings.name, index),
                seed: shared.settings.seed,
                physics_hz: shared.settings.physics_hz,
                decide_hz: shared.settings.decide_hz,
                decisions: shared.settings.decisions,
                temperature: shared.settings.temperature,
                reset_every: shared.settings.reset_every,
                paced: shared.settings.paced,
                greedy: shared.settings.greedy,
                spec: shared.settings.spec.clone(),
            };
            agent::serve(connect.as_ref(), &launch, shared.as_ref());
        })
        .expect("spawn agent")
}

fn spawn_console(shared: Arc<Shared>) -> JoinHandle<()> {
    thread::Builder::new()
        .name("swarm-console".into())
        .spawn(move || console_loop(shared))
        .expect("spawn console")
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

#[cfg(target_os = "linux")]
fn console_loop(shared: Arc<Shared>) {
    use std::os::unix::io::AsRawFd;
    let fd = std::io::stdin().as_raw_fd();
    let old = unsafe { fcntl(fd, 3, 0) };
    if old >= 0 {
        unsafe { fcntl(fd, 4, old | 0o4000) };
    }
    let mut line = String::new();
    let mut byte = [0u8; 1];
    loop {
        if shared.stopping.load(Ordering::Relaxed) {
            break;
        }
        match std::io::stdin().read(&mut byte) {
            Ok(0) => {
                apply_command(&shared, "quit");
                break;
            }
            Ok(_) if byte[0] == b'\n' => {
                let text = std::mem::take(&mut line);
                apply_command(&shared, &text);
            }
            Ok(_) => {
                if byte[0] != b'\r' && line.len() < 1024 {
                    line.push(byte[0] as char);
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(_) => {
                apply_command(&shared, "quit");
                break;
            }
        }
    }
    if old >= 0 {
        unsafe { fcntl(fd, 4, old) };
    }
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn fcntl(fd: i32, cmd: i32, arg: i32) -> i32;
}

#[cfg(not(target_os = "linux"))]
fn console_loop(shared: Arc<Shared>) {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    for line in stdin.lines() {
        match line {
            Ok(text) => apply_command(&shared, &text),
            Err(_) => {
                apply_command(&shared, "quit");
                break;
            }
        }
        if shared.stopping.load(Ordering::Relaxed) {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Axis;

    #[test]
    fn names_stay_within_sixteen_characters() {
        assert_eq!(agent_name("swarm", 0), "swarm01");
        assert_eq!(agent_name("swarm", 8), "swarm09");
        assert_eq!(agent_name("swarm", 99), "swarm100");
        assert!(agent_name("a-very-long-base-name", 0).chars().count() <= 16);
        assert!(agent_name("", 3).ends_with("04"));
    }

    #[test]
    fn a_mismatched_archive_is_refused() {
        let axes = [Axis {
            name: "x",
            min: 0.0,
            max: 1.0,
            bins: 2,
        }];
        let spec = NetSpec::standard();
        let archive = Archive::new("explorer", &axes, spec.clone(), 1);
        assert!(archive_conflict(&archive, "explorer", &axes, &spec).is_none());
        let err = archive_conflict(&archive, "builder", &axes, &spec).unwrap();
        assert!(err.contains("builder"), "{err}");
        assert!(err.contains("--fresh"), "{err}");
    }
}
