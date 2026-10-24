// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The `swarm` command line. Parsing and the read-only commands are pure; `run` and `replay`
//! are the only paths that ask the injected connector for a body.
//!
//! Default run directory: `<data_dir>/exploraswarm/<rules>/`. Default port: 5555.

use std::fs;
use std::path::{Path, PathBuf};

use crate::archive::Archive;
use crate::body::Connect;
use crate::nn::NetSpec;
use crate::rules;
use crate::store::{self, Status};
use crate::swarm::{self, Config, Replay, ServerArgs};

/// What the game (or a test) hands the command. `args` is everything after the name `swarm`.
pub struct Invocation {
    pub args: Vec<String>,
    pub data_dir: PathBuf,
    pub mods: Vec<(String, String)>,
}

/// Parse `inv` and run it. `connect` is called only for `run` and `replay`, and only after the
/// archive check, so a bad invocation never opens a socket.
pub fn run(inv: Invocation, connect: &dyn Fn(&ServerArgs) -> Box<dyn Connect>) -> i32 {
    let action = match parse(&inv) {
        Ok(action) => action,
        Err(err) => {
            eprintln!("swarm: {err}");
            return 2;
        }
    };
    match action {
        Action::Help(text) => {
            println!("{text}");
            0
        }
        Action::Rules => {
            print_rules();
            0
        }
        Action::Status(dir) => show_status(&dir),
        Action::Elites { dir, top } => show_elites(&dir, top),
        Action::Discoveries { dir, last } => show_discoveries(&dir, last),
        Action::Ctl { dir, command } => write_control(&dir, &command),
        Action::Run(cfg) => launch(cfg, connect, false),
        Action::Replay(cfg) => launch(cfg, connect, true),
    }
}

enum Action {
    Help(String),
    Rules,
    Status(PathBuf),
    Elites { dir: PathBuf, top: usize },
    Discoveries { dir: PathBuf, last: usize },
    Ctl { dir: PathBuf, command: String },
    Run(Config),
    Replay(Config),
}

fn launch(cfg: Config, connect: &dyn Fn(&ServerArgs) -> Box<dyn Connect>, replay: bool) -> i32 {
    if let Err(err) =
        rules::by_name(&cfg.rules).ok_or_else(|| format!("unknown rules '{}'", cfg.rules))
    {
        eprintln!("swarm: {err}");
        return 1;
    }
    if (replay || !cfg.fresh)
        && let Err(err) = preflight(&cfg, replay)
    {
        eprintln!("swarm: {err}");
        return 1;
    }
    let conn = connect(&cfg.server);
    swarm::launch(cfg, conn)
}

/// Refuse a mismatched or missing archive before anything joins the server.
fn preflight(cfg: &Config, replay: bool) -> Result<(), String> {
    let path = cfg.run_dir.join(store::ARCHIVE_FILE);
    if !path.exists() {
        if replay {
            return Err(format!("no archive at {}", path.display()));
        }
        return Ok(());
    }
    let rules =
        rules::by_name(&cfg.rules).ok_or_else(|| format!("unknown rules '{}'", cfg.rules))?;
    let (archive, _) = store::load_archive(&path)?;
    if let Some(err) =
        swarm::archive_conflict(&archive, rules.name(), rules.axes(), &NetSpec::standard())
    {
        return Err(err);
    }
    if replay {
        let cell = match cfg.replay.as_ref().and_then(|r| r.cell) {
            Some(cell) => cell,
            None => archive
                .occupied()
                .into_iter()
                .max_by(|&a, &b| {
                    let fa = archive
                        .elite(a)
                        .map(|e| e.fitness_mean)
                        .unwrap_or(f32::NEG_INFINITY);
                    let fb = archive
                        .elite(b)
                        .map(|e| e.fitness_mean)
                        .unwrap_or(f32::NEG_INFINITY);
                    fa.total_cmp(&fb).then(b.cmp(&a))
                })
                .ok_or_else(|| "the archive has no elites to replay".to_string())?,
        };
        if archive.elite(cell).is_none() {
            return Err(format!("cell {cell} is empty"));
        }
    }
    Ok(())
}

fn parse(inv: &Invocation) -> Result<Action, String> {
    if inv.args.is_empty() || is_help(&inv.args[0]) {
        return Ok(Action::Help(usage()));
    }
    let sub = inv.args[0].as_str();
    let rest = &inv.args[1..];
    if rest.iter().any(|a| is_help(a)) {
        return Ok(Action::Help(subcommand_help(sub)?));
    }
    match sub {
        "run" => Ok(Action::Run(parse_run(inv, rest, false)?)),
        "replay" => Ok(Action::Replay(parse_run(inv, rest, true)?)),
        "status" => {
            let opt = parse_dir(inv, rest, &[])?;
            Ok(Action::Status(opt.dir))
        }
        "elites" => {
            let opt = parse_dir(inv, rest, &["top"])?;
            Ok(Action::Elites {
                dir: opt.dir,
                top: opt.top,
            })
        }
        "discoveries" => {
            let opt = parse_dir(inv, rest, &["last"])?;
            Ok(Action::Discoveries {
                dir: opt.dir,
                last: opt.last,
            })
        }
        "ctl" => {
            let (opt, pos) = parse_flags(inv, rest, &[])?;
            let command = ctl_command(&pos)?;
            Ok(Action::Ctl {
                dir: opt.dir,
                command,
            })
        }
        "rules" => {
            let (_opt, pos) = parse_flags(inv, rest, &[])?;
            if !pos.is_empty() {
                return Err(format!("unexpected argument '{}'", pos[0]));
            }
            Ok(Action::Rules)
        }
        other => Err(format!("unknown command '{other}'. {SHORT}")),
    }
}

struct Opt {
    dir: PathBuf,
    rules: String,
    top: usize,
    last: usize,
    server: Option<ServerArgs>,
    agents: u32,
    physics_hz: f32,
    temperature: f32,
    sigma: f32,
    reeval: f32,
    reset_every: u32,
    experience: usize,
    seed: u64,
    name: String,
    minutes: Option<f32>,
    fresh: bool,
    console: bool,
    episodes: u32,
    cell: Option<usize>,
    password: String,
}

fn defaults() -> Opt {
    Opt {
        dir: PathBuf::new(),
        rules: "explorer".into(),
        top: 10,
        last: 20,
        server: None,
        agents: 8,
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
        episodes: 1,
        cell: None,
        password: String::new(),
    }
}

fn parse_dir(inv: &Invocation, args: &[String], extra: &[&str]) -> Result<Opt, String> {
    let (opt, pos) = parse_flags(inv, args, extra)?;
    if !pos.is_empty() {
        return Err(format!("unexpected argument '{}'", pos[0]));
    }
    Ok(opt)
}

fn parse_run(inv: &Invocation, args: &[String], replay: bool) -> Result<Config, String> {
    let (mut opt, pos) = parse_flags(inv, args, if replay { &["episodes"] } else { &[] })?;
    if replay {
        if pos.len() != 1 {
            return Err("replay needs one target: best, or a cell index".into());
        }
        if pos[0] == "best" {
            opt.cell = None;
        } else {
            opt.cell = Some(parse_usize(&pos[0], "cell")?);
        }
    } else if !pos.is_empty() {
        return Err(format!("unexpected argument '{}'", pos[0]));
    }
    let server = opt
        .server
        .clone()
        .ok_or_else(|| "missing --server HOST[:PORT]".to_string())?;
    let mut cfg = Config::new(server, &opt.rules, opt.dir.clone());
    cfg.agents = opt.agents;
    cfg.physics_hz = opt.physics_hz;
    cfg.temperature = opt.temperature;
    cfg.sigma = opt.sigma;
    cfg.reeval = opt.reeval;
    cfg.reset_every = opt.reset_every;
    cfg.experience = opt.experience;
    cfg.seed = opt.seed;
    cfg.name = opt.name;
    cfg.minutes = opt.minutes;
    cfg.fresh = opt.fresh;
    cfg.console = opt.console;
    if replay {
        cfg.replay = Some(Replay {
            cell: opt.cell,
            episodes: opt.episodes,
        });
        cfg.agents = 1;
        cfg.fresh = false;
        cfg.console = false;
    }
    Ok(cfg)
}

fn take_value(
    args: &[String],
    i: &mut usize,
    name: &str,
    inline: Option<String>,
) -> Result<String, String> {
    if let Some(value) = inline {
        return Ok(value);
    }
    *i += 1;
    match args.get(*i) {
        Some(value) if !value.starts_with('-') => Ok(value.clone()),
        _ => Err(format!("--{name} needs a value")),
    }
}

fn parse_flags(
    inv: &Invocation,
    args: &[String],
    extra: &[&str],
) -> Result<(Opt, Vec<String>), String> {
    let mut opt = defaults();
    let mut dir_set = false;
    let mut i = 0;
    let mut pos = Vec::new();
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "--" {
            pos.extend(args[i + 1..].iter().cloned());
            break;
        }
        if let Some(flag) = arg.strip_prefix("--") {
            let (name, inline) = match flag.split_once('=') {
                Some((name, value)) => (name, Some(value.to_string())),
                None => (flag, None),
            };
            match name {
                "fresh" | "no-console" => {
                    if inline.is_some() {
                        return Err(format!("--{name} takes no value"));
                    }
                    if name == "fresh" {
                        opt.fresh = true;
                    } else {
                        opt.console = false;
                    }
                }
                "server" => {
                    opt.server = Some(split_server(&take_value(args, &mut i, name, inline)?)?)
                }
                "password" => opt.password = take_value(args, &mut i, name, inline)?,
                "agents" => {
                    opt.agents =
                        parse_u32(&take_value(args, &mut i, name, inline)?, "agents", 1, 128)?
                }
                "rules" => {
                    opt.rules = take_value(args, &mut i, name, inline)?;
                    if opt.rules.is_empty() {
                        return Err("--rules needs a name".into());
                    }
                }
                "run-dir" => {
                    opt.dir = PathBuf::from(take_value(args, &mut i, name, inline)?);
                    dir_set = true;
                }
                "physics-hz" => {
                    opt.physics_hz = parse_f32(
                        &take_value(args, &mut i, name, inline)?,
                        "physics-hz",
                        0.0,
                        false,
                    )?;
                }
                "temperature" => {
                    opt.temperature = parse_f32(
                        &take_value(args, &mut i, name, inline)?,
                        "temperature",
                        0.0,
                        true,
                    )?;
                }
                "sigma" => {
                    opt.sigma =
                        parse_f32(&take_value(args, &mut i, name, inline)?, "sigma", 0.0, true)?
                }
                "reeval" => {
                    let v = parse_f32(
                        &take_value(args, &mut i, name, inline)?,
                        "reeval",
                        0.0,
                        true,
                    )?;
                    if v > 1.0 {
                        return Err("--reeval must be from 0 to 1".into());
                    }
                    opt.reeval = v;
                }
                "reset-every" => {
                    opt.reset_every = parse_u32(
                        &take_value(args, &mut i, name, inline)?,
                        "reset-every",
                        1,
                        u32::MAX,
                    )?;
                }
                "experience" => {
                    opt.experience =
                        parse_usize(&take_value(args, &mut i, name, inline)?, "experience")?
                }
                "seed" => {
                    opt.seed = take_value(args, &mut i, name, inline)?
                        .parse::<u64>()
                        .map_err(|_| "--seed needs an integer".to_string())?;
                }
                "name" => {
                    opt.name = take_value(args, &mut i, name, inline)?;
                    if opt.name.is_empty() || opt.name.chars().any(|c| c.is_whitespace()) {
                        return Err("--name needs a single word".into());
                    }
                }
                "minutes" => {
                    opt.minutes = Some(parse_f32(
                        &take_value(args, &mut i, name, inline)?,
                        "minutes",
                        0.0,
                        false,
                    )?);
                }
                "top" if extra.contains(&"top") => {
                    opt.top = parse_usize(&take_value(args, &mut i, name, inline)?, "top")?.max(1);
                }
                "last" if extra.contains(&"last") => {
                    opt.last = parse_usize(&take_value(args, &mut i, name, inline)?, "last")?;
                }
                "episodes" if extra.contains(&"episodes") => {
                    opt.episodes = parse_u32(
                        &take_value(args, &mut i, name, inline)?,
                        "episodes",
                        1,
                        u32::MAX,
                    )?;
                }
                other => return Err(format!("unknown flag --{other}")),
            }
        } else {
            pos.push(arg.to_string());
        }
        i += 1;
    }
    if let Some(server) = opt.server.as_mut() {
        server.password.clone_from(&opt.password);
    }
    if !dir_set {
        opt.dir = inv.data_dir.join("exploraswarm").join(&opt.rules);
    }
    Ok((opt, pos))
}

fn ctl_command(pos: &[String]) -> Result<String, String> {
    match pos {
        [cmd]
            if matches!(
                cmd.as_str(),
                "pause" | "resume" | "save" | "status" | "quit"
            ) =>
        {
            Ok(cmd.clone())
        }
        [cmd, n] if cmd == "agents" => {
            let n = parse_u32(n, "agents", 0, 128)?;
            Ok(format!("agents {n}"))
        }
        _ => Err("ctl needs pause, resume, save, status, quit, or agents N".into()),
    }
}

fn split_server(text: &str) -> Result<ServerArgs, String> {
    if text.is_empty() {
        return Err("--server needs HOST[:PORT]".into());
    }
    let (host, port) = if let Some(rest) = text.strip_prefix('[') {
        if let Some((host, port)) = rest.rsplit_once("]:") {
            (host.to_string(), Some(port.to_string()))
        } else if let Some(host) = rest.strip_suffix(']') {
            (host.to_string(), None)
        } else {
            return Err(format!("bad address '{text}'"));
        }
    } else if text.matches(':').count() == 1 {
        let (host, port) = text.split_once(':').unwrap();
        (host.to_string(), Some(port.to_string()))
    } else {
        (text.to_string(), None)
    };
    if host.is_empty() {
        return Err("--server needs a host".into());
    }
    let port = match port {
        None => 5555,
        Some(text) => {
            if !text.chars().all(|c| c.is_ascii_digit()) {
                return Err(format!("bad port '{text}'"));
            }
            let port: u16 = text.parse().map_err(|_| format!("bad port '{text}'"))?;
            if port == 0 {
                return Err("port 0 is not valid".into());
            }
            port
        }
    };
    Ok(ServerArgs {
        host,
        port,
        password: String::new(),
    })
}

fn parse_u32(text: &str, name: &str, lo: u32, hi: u32) -> Result<u32, String> {
    let n: u32 = text
        .parse()
        .map_err(|_| format!("--{name} needs an integer"))?;
    if n < lo || n > hi {
        return Err(format!("--{name} must be from {lo} to {hi}"));
    }
    Ok(n)
}

fn parse_usize(text: &str, name: &str) -> Result<usize, String> {
    text.parse::<usize>()
        .map_err(|_| format!("--{name} needs an integer"))
}

fn parse_f32(text: &str, name: &str, lo: f32, lo_inclusive_zero: bool) -> Result<f32, String> {
    let v: f32 = text
        .parse()
        .map_err(|_| format!("--{name} needs a number"))?;
    if !v.is_finite() || (lo_inclusive_zero && v < lo) || (!lo_inclusive_zero && v <= lo) {
        return Err(format!("--{name} is out of range"));
    }
    Ok(v)
}

fn is_help(arg: &str) -> bool {
    arg == "--help" || arg == "-h"
}

const SHORT: &str = "Try `swarm --help`.";

fn usage() -> String {
    "\
exploraswarm — learning players on a server you name

usage:
  swarm run         --server HOST[:PORT] [options]
  swarm status      [--rules R | --run-dir DIR]
  swarm elites      [--rules R | --run-dir DIR] [--top N]
  swarm discoveries [--rules R | --run-dir DIR] [--last N]
  swarm ctl         [--rules R | --run-dir DIR] pause|resume|save|status|agents N|quit
  swarm rules
  swarm replay      [--rules R | --run-dir DIR] best|CELL --server HOST[:PORT]

`swarm <command> --help` prints that command. The archive and logs go under
<data_dir>/exploraswarm/<rules>/ unless --run-dir is set."
        .to_string()
}

fn subcommand_help(sub: &str) -> Result<String, String> {
    let text = match sub {
        "run" => "\
usage: swarm run --server HOST[:PORT] [options]

  --server HOST[:PORT]   server to join (default port 5555)
  --password P           server password
  --agents N             players to run (default 8, 1..=128)
  --rules NAME           rule set (default explorer)
  --run-dir DIR          archive directory
  --physics-hz N         physics frames per second (default 30)
  --temperature T        action softmax temperature (default 0.4)
  --sigma S              gaussian mutation scale (default 0.03)
  --reeval F             fraction of candidates that re-evaluate an elite (default 0.1)
  --reset-every N        reconnect every N episodes (default 5)
  --experience N         trajectories kept in memory (default 64)
  --seed N               run seed for a fresh archive (default 1)
  --name STEM            agent names are <stem>01, <stem>02, ... (default swarm)
  --minutes M            stop after M minutes
  --fresh                replace an existing archive
  --no-console           ignore stdin; control the run through the control file",
        "status" => "usage: swarm status [--rules R | --run-dir DIR]\n\nPrint coverage, the emitter counts and a heat map of the first two axes.",
        "elites" => "usage: swarm elites [--rules R | --run-dir DIR] [--top N]\n\nList the strongest elites (default 10).",
        "discoveries" => "usage: swarm discoveries [--rules R | --run-dir DIR] [--last N]\n\nPrint the latest discovery-log lines (default 20).",
        "ctl" => "usage: swarm ctl [--rules R | --run-dir DIR] pause|resume|save|status|agents N|quit\n\nWrite one command into the run's control file.",
        "rules" => "usage: swarm rules\n\nList every rule set, its episode length and its behaviour axes.",
        "replay" => "\
usage: swarm replay [--rules R | --run-dir DIR] best|CELL --server HOST[:PORT] [--password P] [--episodes N]

Play one stored elite with the greedy policy. Nothing is written back.",
        other => return Err(format!("unknown command '{other}'. {SHORT}")),
    };
    Ok(text.to_string())
}

fn print_rules() {
    for rules in rules::all() {
        let spec = rules.episode();
        println!("{} — {}", rules.name(), rules.about());
        println!(
            "  episode: {} decisions at {} Hz",
            spec.decisions, spec.decide_hz
        );
        for axis in rules.axes() {
            println!(
                "  axis {:<12} {:>6} .. {:>5}   {} bins",
                axis.name, axis.min, axis.max, axis.bins
            );
        }
    }
}

fn show_status(dir: &Path) -> i32 {
    let archive = match load_or_tell(dir) {
        Ok(archive) => archive,
        Err(code) => return code,
    };
    let live = store::load_status(&dir.join(store::STATUS_FILE)).ok();
    print_summary(&archive, live.as_ref(), discovery_count(dir, live.as_ref()));
    println!("{}", heat_map(&archive));
    0
}

fn discovery_count(dir: &Path, live: Option<&Status>) -> u64 {
    if let Some(live) = live {
        return live.discoveries;
    }
    store::load_discoveries(&dir.join(store::DISCOVERIES_FILE))
        .map(|rows| rows.len() as u64)
        .unwrap_or(0)
}

fn show_elites(dir: &Path, top: usize) -> i32 {
    let archive = match load_or_tell(dir) {
        Ok(archive) => archive,
        Err(code) => return code,
    };
    let mut rows: Vec<(usize, f32)> = archive
        .occupied()
        .into_iter()
        .filter_map(|cell| archive.elite(cell).map(|e| (cell, e.fitness_mean)))
        .collect();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    if rows.is_empty() {
        println!("no elites");
        return 0;
    }
    println!("cell\tfitness\tevals\temitter\tgenome\tdescriptor");
    for (cell, _) in rows.into_iter().take(top) {
        let Some(elite) = archive.elite(cell) else {
            continue;
        };
        println!(
            "{cell}\t{:.4}\t{}\t{}\t{}\t{}",
            elite.fitness_mean,
            elite.evals,
            crate::emitter::name_of(elite.genome.emitter),
            elite.genome.id,
            desc(&elite.descriptor)
        );
    }
    0
}

fn show_discoveries(dir: &Path, last: usize) -> i32 {
    let path = dir.join(store::DISCOVERY_LOG);
    let Ok(text) = fs::read_to_string(&path) else {
        println!("no discoveries yet");
        return 0;
    };
    let lines: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
    if lines.is_empty() {
        println!("no discoveries yet");
        return 0;
    }
    let start = lines.len().saturating_sub(last);
    for line in &lines[start..] {
        println!("{line}");
    }
    0
}

fn write_control(dir: &Path, command: &str) -> i32 {
    if let Err(err) = fs::create_dir_all(dir) {
        eprintln!("swarm: {err}");
        return 1;
    }
    let path = dir.join(store::CONTROL_FILE);
    if let Err(err) = fs::write(&path, format!("{command}\n")) {
        eprintln!("swarm: {err}");
        return 1;
    }
    println!("wrote {command} to {}", path.display());
    0
}

fn load_or_tell(dir: &Path) -> Result<Archive, i32> {
    let path = dir.join(store::ARCHIVE_FILE);
    match store::load_archive(&path) {
        Ok((archive, _)) => Ok(archive),
        Err(err) => {
            eprintln!("swarm: {err}");
            Err(1)
        }
    }
}

fn print_summary(archive: &Archive, live: Option<&Status>, discoveries: u64) {
    let agents = live
        .map(|s| s.agents.to_string())
        .unwrap_or_else(|| "-".into());
    let paused = live
        .map(|s| if s.paused { "1" } else { "0" })
        .unwrap_or("-");
    let eps = live
        .map(|s| format!("{:.3}", s.eps_per_min))
        .unwrap_or_else(|| "-".into());
    let best = archive
        .best()
        .map(|v| format!("{v:.4}"))
        .unwrap_or_default();
    println!("rules={}", archive.rules);
    println!("agents={agents}");
    println!("paused={paused}");
    println!("episodes={}", archive.episodes);
    println!("coverage={:.6}", archive.coverage());
    println!("qd_score={:.4}", archive.qd_score());
    println!("best={best}");
    println!("discoveries={discoveries}");
    let names = ["random", "gaussian", "isoline"];
    for (i, name) in names.iter().enumerate() {
        let stats = archive.stats.get(i).copied().unwrap_or_default();
        println!("emitter.{name}.insertions={}", stats.insertions);
        println!("emitter.{name}.challenges={}", stats.challenges);
        println!("emitter.{name}.replacements={}", stats.replacements);
        println!("emitter.{name}.rejected={}", stats.rejected);
        println!("emitter.{name}.reevals={}", stats.reevals);
    }
    println!("eps_per_min={eps}");
    if archive.axes.len() >= 2 {
        println!(
            "map: {} →   {} ↑",
            archive.axes[0].name, archive.axes[1].name
        );
    }
}

/// Project the archive onto its first two axes. Empty cells are a space. Filled cells use
/// `.:-=+*#%@` from the lowest fitness on the map to the highest. A single filled value is `@`.
pub(crate) fn heat_map(archive: &Archive) -> String {
    let grid = archive.projection();
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    let mut any = false;
    for row in &grid {
        for cell in row {
            if let Some(v) = cell.filter(|v| v.is_finite()) {
                any = true;
                min = min.min(v);
                max = max.max(v);
            }
        }
    }
    let ramp = b".:-=+*#%@";
    let mut out = String::new();
    for row in grid.iter().rev() {
        for cell in row {
            let glyph = match cell {
                Some(v) if v.is_finite() && any => {
                    if (max - min).abs() <= f32::EPSILON {
                        '@'
                    } else {
                        let t = ((*v - min) / (max - min)).clamp(0.0, 1.0);
                        let idx = (t * (ramp.len() - 1) as f32).round() as usize;
                        ramp[idx] as char
                    }
                }
                _ => ' ',
            };
            out.push(glyph);
        }
        out.push('\n');
    }
    out
}

fn desc(values: &[f32]) -> String {
    values
        .iter()
        .map(|v| format!("{v:.3}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{Archive, Genome};
    use crate::rules::Axis;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn inv(args: &[&str]) -> Invocation {
        Invocation {
            args: args.iter().map(|s| (*s).to_string()).collect(),
            data_dir: PathBuf::from("/tmp/xswarm-data"),
            mods: Vec::new(),
        }
    }

    fn scratch() -> PathBuf {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("xswarm-cli-{n}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn defaults_and_addresses() {
        let cfg = match parse(&inv(&["run", "--server", "127.0.0.1"])).unwrap() {
            Action::Run(cfg) => cfg,
            _ => panic!("run"),
        };
        assert_eq!(cfg.server.host, "127.0.0.1");
        assert_eq!(cfg.server.port, 5555);
        assert_eq!(cfg.agents, 8);
        assert!((cfg.temperature - 0.4).abs() < 1e-6);
        assert!((cfg.sigma - 0.03).abs() < 1e-6);
        assert!((cfg.reeval - 0.1).abs() < 1e-6);
        assert_eq!(cfg.reset_every, 5);
        assert_eq!(cfg.experience, 64);
        assert_eq!(cfg.seed, 1);
        assert_eq!(cfg.name, "swarm");
        assert!(cfg.console);
        assert!(!cfg.fresh);
        assert_eq!(cfg.rules, "explorer");
        assert_eq!(
            cfg.run_dir,
            PathBuf::from("/tmp/xswarm-data/exploraswarm/explorer")
        );

        let cfg = match parse(&inv(&[
            "run",
            "--server=[::1]:9",
            "--agents=3",
            "--agents",
            "4",
            "--no-console",
            "--fresh",
        ]))
        .unwrap()
        {
            Action::Run(cfg) => cfg,
            _ => panic!("run"),
        };
        assert_eq!(cfg.server.host, "::1");
        assert_eq!(cfg.server.port, 9);
        assert_eq!(cfg.agents, 4);
        assert!(!cfg.console);
        assert!(cfg.fresh);

        let cfg = match parse(&inv(&["run", "--server", "::1", "--rules", "builder"])).unwrap() {
            Action::Run(cfg) => cfg,
            _ => panic!("ipv6"),
        };
        assert_eq!(cfg.server.host, "::1");
        assert_eq!(cfg.server.port, 5555);
        assert_eq!(
            cfg.run_dir,
            PathBuf::from("/tmp/xswarm-data/exploraswarm/builder")
        );
    }

    #[test]
    fn errors_name_the_problem() {
        let message = |args: &[&str]| match parse(&inv(args)) {
            Err(text) => text,
            Ok(_) => panic!("expected a parse error"),
        };
        assert!(message(&["run"]).contains("--server"));
        assert!(message(&["run", "--server", "h:0"]).contains("port"));
        assert!(message(&["run", "--server", "h", "--fresh=1"]).contains("takes no value"));
        assert!(message(&["nope"]).contains("unknown command"));
        assert!(message(&["run", "--server", "h", "--agents", "0"]).contains("agents"));
        assert!(message(&["replay", "best"]).contains("--server"));
        assert!(message(&["replay", "--server", "h"]).contains("replay needs"));
    }

    #[test]
    fn help_rules_status_ctl_and_replay_exit_codes() {
        let connect = |_: &ServerArgs| -> Box<dyn Connect> { panic!("connect should not run") };
        assert_eq!(run(inv(&[]), &connect), 0);
        assert_eq!(run(inv(&["--help"]), &connect), 0);
        assert_eq!(run(inv(&["run", "--help"]), &connect), 0);
        assert_eq!(run(inv(&["rules"]), &connect), 0);
        assert_eq!(run(inv(&["run"]), &connect), 2);
        assert_eq!(run(inv(&["nope"]), &connect), 2);
        assert_eq!(run(inv(&["replay", "best"]), &connect), 2);

        let dir = scratch();
        let data = dir.join("data");
        fs::create_dir_all(&data).unwrap();
        let mut status_inv = inv(&["status", "--rules", "explorer"]);
        status_inv.data_dir = data.clone();
        assert_eq!(run(status_inv, &connect), 1);

        let mut disc = inv(&["discoveries"]);
        disc.data_dir = data.clone();
        assert_eq!(run(disc, &connect), 0);

        let mut ctl = inv(&["ctl", "--run-dir", dir.to_str().unwrap(), "pause"]);
        ctl.data_dir = data;
        assert_eq!(run(ctl, &connect), 0);
        let written = fs::read_to_string(dir.join(store::CONTROL_FILE)).unwrap();
        assert_eq!(written, "pause\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn heat_map_ramps_the_first_two_axes() {
        let axes = [
            Axis {
                name: "x",
                min: 0.0,
                max: 1.0,
                bins: 2,
            },
            Axis {
                name: "y",
                min: 0.0,
                max: 1.0,
                bins: 2,
            },
        ];
        let mut archive = Archive::new("explorer", &axes, NetSpec::standard(), 1);
        let genome = Genome {
            id: 1,
            parent: None,
            emitter: 0,
            params: vec![0.1],
        };
        archive.offer_new(genome, 3.0, vec![0.1, 0.9], vec![], 0.0, 1);
        let map = heat_map(&archive);
        let lines: Vec<&str> = map.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].chars().next().unwrap(), '@');
        assert_eq!(lines[0].chars().nth(1).unwrap(), ' ');
        assert!(lines[1].chars().all(|c| c == ' '));
    }
}
