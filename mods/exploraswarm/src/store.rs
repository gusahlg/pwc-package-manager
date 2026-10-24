// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Checkpoints, the status file and the logs.
//!
//! `archive.bin` and `discoveries.bin` are little-endian. A truncated, mismatched or trailing-junk
//! file is an error string, never a panic. Writes land in a temporary file, which is synced and
//! then renamed over the destination.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::archive::{Archive, Challenger, Elite, EmitterStats, Genome, Partition};
use crate::discover::Record;
use crate::nn::NetSpec;
use crate::rules::Axis;

pub const ARCHIVE_FILE: &str = "archive.bin";
pub const DISCOVERIES_FILE: &str = "discoveries.bin";
pub const STATUS_FILE: &str = "status.txt";
pub const EPISODES_LOG: &str = "episodes.log";
pub const DISCOVERY_LOG: &str = "discoveries.log";
pub const CONTROL_FILE: &str = "control";

const ARCHIVE_MAGIC: &[u8] = b"XSWARM\0\x01";
const DISC_MAGIC: &[u8] = b"XSDISC\0\x01";
const FORMAT: u32 = 1;
const MAX_STR: usize = 4_096;
const MAX_COUNT: usize = 2_000_000;

/// Human-readable snapshot. `episodes` is the archive total; `eps_per_min` is this process.
#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    pub agents: u32,
    pub paused: bool,
    pub episodes: u64,
    pub coverage: f32,
    pub qd_score: f32,
    pub best: Option<f32>,
    pub discoveries: u64,
    pub emitters: Vec<(String, EmitterStats)>,
    pub eps_per_min: f64,
    pub updated_unix: u64,
}

pub fn save_archive(path: &Path, archive: &Archive, rng: [u64; 4]) -> Result<(), String> {
    atomic_write(path, &encode_archive(archive, rng))
}

pub fn load_archive(path: &Path) -> Result<(Archive, [u64; 4]), String> {
    let bytes = fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    decode_archive(&bytes)
}

pub fn save_discoveries(path: &Path, records: &[Record]) -> Result<(), String> {
    atomic_write(path, &encode_discoveries(records))
}

pub fn load_discoveries(path: &Path) -> Result<Vec<Record>, String> {
    let bytes = fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    decode_discoveries(&bytes)
}

pub fn save_status(path: &Path, status: &Status) -> Result<(), String> {
    atomic_write(path, status_text(status).as_bytes())
}

pub fn load_status(path: &Path) -> Result<Status, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    parse_status(&text)
}

pub fn append_lines(path: &Path, lines: &[String]) -> Result<(), String> {
    if lines.is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    for line in lines {
        writeln!(file, "{line}").map_err(|e| e.to_string())?;
    }
    file.flush().map_err(|e| e.to_string())
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("creating {}: {e}", parent.display()))?;
    }
    let tmp = tmp_path(path);
    {
        let mut file =
            File::create(&tmp).map_err(|e| format!("creating {}: {e}", tmp.display()))?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, path)
        .map_err(|e| format!("renaming {} onto {}: {e}", tmp.display(), path.display()))
}

fn tmp_path(path: &Path) -> PathBuf {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    path.with_file_name(format!("{name}.tmp"))
}

fn encode_archive(archive: &Archive, rng: [u64; 4]) -> Vec<u8> {
    let mut w = Vec::new();
    w.extend_from_slice(ARCHIVE_MAGIC);
    put_u32(&mut w, FORMAT);
    put_str(&mut w, &archive.rules);
    put_u32(&mut w, archive.axes.len() as u32);
    for ax in &archive.axes {
        put_str(&mut w, ax.name);
        put_f32(&mut w, ax.min);
        put_f32(&mut w, ax.max);
        put_u16(&mut w, ax.bins);
    }
    put_spec(&mut w, &archive.spec);
    put_u64(&mut w, archive.episodes);
    put_u64(&mut w, archive.next_genome_id);
    match archive.min_seen {
        Some(v) => {
            w.push(1);
            put_f32(&mut w, v);
        }
        None => w.push(0),
    }
    for word in rng {
        put_u64(&mut w, word);
    }
    put_u64(&mut w, archive.run_seed);
    put_u32(&mut w, archive.stats.len() as u32);
    for stat in &archive.stats {
        put_stats(&mut w, stat);
    }
    put_u32(&mut w, archive.cells.len() as u32);
    for slot in &archive.cells {
        match slot {
            None => w.push(0),
            Some(elite) => {
                w.push(1);
                put_elite(&mut w, elite);
            }
        }
    }
    put_u32(&mut w, archive.challengers.len() as u32);
    for ch in &archive.challengers {
        put_genome(&mut w, &ch.genome);
        put_u32(&mut w, ch.cell as u32);
        put_f32(&mut w, ch.first_fitness);
        put_f32s(&mut w, &ch.descriptor);
        put_u64s(&mut w, &ch.signatures);
        put_f32(&mut w, ch.bonus);
    }
    w
}

fn decode_archive(bytes: &[u8]) -> Result<(Archive, [u64; 4]), String> {
    let mut r = Reader::new(bytes);
    let magic = r.bytes(ARCHIVE_MAGIC.len())?;
    if magic != ARCHIVE_MAGIC {
        return Err("bad archive magic".into());
    }
    let version = r.u32()?;
    if version != FORMAT {
        return Err(format!("unsupported archive version {version}"));
    }
    let rules = r.str()?;
    let axis_n = r.count()?;
    let mut axes = Vec::with_capacity(axis_n);
    for _ in 0..axis_n {
        let name = r.str()?;
        // `Axis::name` is `'static`. A loaded archive lives for the process, so the name stays leaked.
        let name: &'static str = Box::leak(name.into_boxed_str());
        axes.push(Axis {
            name,
            min: r.f32()?,
            max: r.f32()?,
            bins: r.u16()?,
        });
    }
    let spec = r.spec()?;
    let mut archive = Archive::new(&rules, &axes, spec, 0);
    archive.episodes = r.u64()?;
    archive.next_genome_id = r.u64()?;
    archive.min_seen = match r.u8()? {
        0 => None,
        1 => Some(r.f32()?),
        other => return Err(format!("bad min_seen tag {other}")),
    };
    if let Some(v) = archive.min_seen
        && !v.is_finite()
    {
        return Err("min_seen is not finite".into());
    }
    let rng = [r.u64()?, r.u64()?, r.u64()?, r.u64()?];
    archive.run_seed = r.u64()?;
    let stat_n = r.count()?;
    archive.stats = Vec::with_capacity(stat_n);
    for _ in 0..stat_n {
        archive.stats.push(r.stats()?);
    }
    let cell_n = r.count()?;
    if cell_n != archive.grid.cells() {
        return Err(format!(
            "archive has {cell_n} cells, the axes describe {}",
            archive.grid.cells()
        ));
    }
    archive.cells = Vec::with_capacity(cell_n);
    for _ in 0..cell_n {
        match r.u8()? {
            0 => archive.cells.push(None),
            1 => archive.cells.push(Some(r.elite()?)),
            other => return Err(format!("bad cell tag {other}")),
        }
    }
    let ch_n = r.count()?;
    for _ in 0..ch_n {
        archive.challengers.push(Challenger {
            genome: r.genome()?,
            cell: r.u32()? as usize,
            first_fitness: r.f32()?,
            descriptor: r.f32s()?,
            signatures: r.u64s()?,
            bonus: r.f32()?,
            in_flight: false,
        });
    }
    r.finish("archive")?;
    archive.rebuild_signatures();
    Ok((archive, rng))
}

fn encode_discoveries(records: &[Record]) -> Vec<u8> {
    let mut sorted: Vec<&Record> = records.iter().collect();
    sorted.sort_by_key(|r| r.key);
    let mut w = Vec::new();
    w.extend_from_slice(DISC_MAGIC);
    put_u32(&mut w, FORMAT);
    put_u32(&mut w, sorted.len() as u32);
    for rec in sorted {
        put_u64(&mut w, rec.key);
        put_u64(&mut w, rec.count);
        put_u64(&mut w, rec.episode);
        put_str(&mut w, &rec.agent);
    }
    w
}

fn decode_discoveries(bytes: &[u8]) -> Result<Vec<Record>, String> {
    let mut r = Reader::new(bytes);
    let magic = match r.bytes(DISC_MAGIC.len()) {
        Ok(magic) => magic,
        Err(_) => return Err("bad discoveries magic".into()),
    };
    if magic != DISC_MAGIC {
        return Err("bad discoveries magic".into());
    }
    let version = r.u32()?;
    if version != FORMAT {
        return Err(format!("unsupported discoveries version {version}"));
    }
    let n = r.count()?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(Record {
            key: r.u64()?,
            count: r.u64()?,
            episode: r.u64()?,
            agent: r.str()?,
        });
    }
    r.finish("discoveries")?;
    Ok(out)
}

fn status_text(status: &Status) -> String {
    let mut lines = Vec::new();
    lines.push(format!("agents={}", status.agents));
    lines.push(format!("paused={}", u8::from(status.paused)));
    lines.push(format!("episodes={}", status.episodes));
    lines.push(format!("coverage={:.6}", status.coverage));
    lines.push(format!("qd_score={:.6}", status.qd_score));
    match status.best {
        Some(v) => lines.push(format!("best={:.6}", v)),
        None => lines.push("best=".into()),
    }
    lines.push(format!("discoveries={}", status.discoveries));
    for (name, stat) in &status.emitters {
        lines.push(format!("emitter.{name}.insertions={}", stat.insertions));
        lines.push(format!("emitter.{name}.challenges={}", stat.challenges));
        lines.push(format!("emitter.{name}.replacements={}", stat.replacements));
        lines.push(format!("emitter.{name}.rejected={}", stat.rejected));
        lines.push(format!("emitter.{name}.reevals={}", stat.reevals));
    }
    lines.push(format!("eps_per_min={:.4}", status.eps_per_min));
    lines.push(format!("updated_unix={}", status.updated_unix));
    lines.push(String::new());
    lines.join("\n")
}

fn parse_status(text: &str) -> Result<Status, String> {
    let mut map = std::collections::BTreeMap::new();
    for (n, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            return Err(format!("status line {}: missing '='", n + 1));
        };
        map.insert(k.to_string(), v.to_string());
    }
    let need = |key: &str| -> Result<String, String> {
        map.get(key)
            .cloned()
            .ok_or_else(|| format!("status missing {key}"))
    };
    let num = |key: &str| -> Result<f64, String> {
        need(key)?
            .parse()
            .map_err(|_| format!("status {key} is not a number"))
    };
    let mut emitters = Vec::new();
    let mut names = Vec::new();
    for key in map.keys() {
        if let Some(rest) = key.strip_prefix("emitter.")
            && let Some((name, _)) = rest.split_once('.')
            && !names.iter().any(|n| n == name)
        {
            names.push(name.to_string());
        }
    }
    for name in names {
        let stat = EmitterStats {
            insertions: num(&format!("emitter.{name}.insertions"))? as u64,
            challenges: num(&format!("emitter.{name}.challenges"))? as u64,
            replacements: num(&format!("emitter.{name}.replacements"))? as u64,
            rejected: num(&format!("emitter.{name}.rejected"))? as u64,
            reevals: num(&format!("emitter.{name}.reevals"))? as u64,
        };
        emitters.push((name, stat));
    }
    let best = match need("best")?.as_str() {
        "" => None,
        text => Some(
            text.parse::<f32>()
                .map_err(|_| "status best is not a number".to_string())?,
        ),
    };
    Ok(Status {
        agents: num("agents")? as u32,
        paused: num("paused")? != 0.0,
        episodes: num("episodes")? as u64,
        coverage: num("coverage")? as f32,
        qd_score: num("qd_score")? as f32,
        best,
        discoveries: num("discoveries")? as u64,
        emitters,
        eps_per_min: num("eps_per_min")?,
        updated_unix: num("updated_unix")? as u64,
    })
}

fn put_spec(w: &mut Vec<u8>, spec: &NetSpec) {
    put_u32(w, spec.version);
    put_u16(w, spec.obs_len);
    put_u16(w, spec.actions);
    put_u16(w, spec.memory);
    put_u32(w, spec.hidden.len() as u32);
    for h in &spec.hidden {
        put_u16(w, *h);
    }
    put_u32(w, spec.norm_version);
}

fn put_stats(w: &mut Vec<u8>, stat: &EmitterStats) {
    put_u64(w, stat.insertions);
    put_u64(w, stat.challenges);
    put_u64(w, stat.replacements);
    put_u64(w, stat.rejected);
    put_u64(w, stat.reevals);
}

fn put_elite(w: &mut Vec<u8>, elite: &Elite) {
    put_genome(w, &elite.genome);
    put_f32(w, elite.fitness_mean);
    put_f32(w, elite.fitness_m2);
    put_u32(w, elite.evals);
    put_f32s(w, &elite.descriptor);
    put_u64s(w, &elite.signatures);
    put_f32(w, elite.bonus);
    put_u64(w, elite.born);
    put_u64(w, elite.last_eval);
    match elite.moved_votes {
        None => w.push(0),
        Some((cell, votes)) => {
            w.push(1);
            put_u32(w, cell as u32);
            w.push(votes);
        }
    }
}

fn put_genome(w: &mut Vec<u8>, genome: &Genome) {
    put_u64(w, genome.id);
    match genome.parent {
        Some(id) => {
            w.push(1);
            put_u64(w, id);
        }
        None => {
            w.push(0);
            put_u64(w, 0);
        }
    }
    w.push(genome.emitter);
    put_f32s(w, &genome.params);
}

fn put_str(w: &mut Vec<u8>, text: &str) {
    put_u32(w, text.len() as u32);
    w.extend_from_slice(text.as_bytes());
}

fn put_f32s(w: &mut Vec<u8>, vals: &[f32]) {
    put_u32(w, vals.len() as u32);
    for v in vals {
        put_f32(w, *v);
    }
}

fn put_u64s(w: &mut Vec<u8>, vals: &[u64]) {
    put_u32(w, vals.len() as u32);
    for v in vals {
        put_u64(w, *v);
    }
}

fn put_u16(w: &mut Vec<u8>, v: u16) {
    w.extend_from_slice(&v.to_le_bytes());
}
fn put_u32(w: &mut Vec<u8>, v: u32) {
    w.extend_from_slice(&v.to_le_bytes());
}
fn put_u64(w: &mut Vec<u8>, v: u64) {
    w.extend_from_slice(&v.to_le_bytes());
}
fn put_f32(w: &mut Vec<u8>, v: f32) {
    w.extend_from_slice(&v.to_le_bytes());
}

struct Reader<'a> {
    buf: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, i: 0 }
    }

    fn bytes(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .i
            .checked_add(n)
            .ok_or_else(|| "archive offset overflow".to_string())?;
        if end > self.buf.len() {
            return Err("truncated file".into());
        }
        let slice = &self.buf[self.i..end];
        self.i = end;
        Ok(slice)
    }

    fn finish(&self, what: &str) -> Result<(), String> {
        if self.i != self.buf.len() {
            Err(format!("trailing bytes in {what}"))
        } else {
            Ok(())
        }
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.bytes(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, String> {
        let mut a = [0u8; 2];
        a.copy_from_slice(self.bytes(2)?);
        Ok(u16::from_le_bytes(a))
    }

    fn u32(&mut self) -> Result<u32, String> {
        let mut a = [0u8; 4];
        a.copy_from_slice(self.bytes(4)?);
        Ok(u32::from_le_bytes(a))
    }

    fn u64(&mut self) -> Result<u64, String> {
        let mut a = [0u8; 8];
        a.copy_from_slice(self.bytes(8)?);
        Ok(u64::from_le_bytes(a))
    }

    fn f32(&mut self) -> Result<f32, String> {
        let mut a = [0u8; 4];
        a.copy_from_slice(self.bytes(4)?);
        Ok(f32::from_le_bytes(a))
    }

    fn count(&mut self) -> Result<usize, String> {
        let n = self.u32()? as usize;
        if n > MAX_COUNT {
            return Err(format!("length {n} is too large"));
        }
        Ok(n)
    }

    fn str(&mut self) -> Result<String, String> {
        let n = self.u32()? as usize;
        if n > MAX_STR {
            return Err(format!("string length {n} is too large"));
        }
        let bytes = self.bytes(n)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| "string is not utf-8".to_string())
    }

    fn f32s(&mut self) -> Result<Vec<f32>, String> {
        let n = self.count()?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(self.f32()?);
        }
        Ok(out)
    }

    fn u64s(&mut self) -> Result<Vec<u64>, String> {
        let n = self.count()?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(self.u64()?);
        }
        Ok(out)
    }

    fn spec(&mut self) -> Result<NetSpec, String> {
        let version = self.u32()?;
        let obs_len = self.u16()?;
        let actions = self.u16()?;
        let memory = self.u16()?;
        let n = self.count()?;
        if n > 64 {
            return Err("too many hidden layers".into());
        }
        let mut hidden = Vec::with_capacity(n);
        for _ in 0..n {
            hidden.push(self.u16()?);
        }
        Ok(NetSpec {
            version,
            obs_len,
            actions,
            memory,
            hidden,
            norm_version: self.u32()?,
        })
    }

    fn stats(&mut self) -> Result<EmitterStats, String> {
        Ok(EmitterStats {
            insertions: self.u64()?,
            challenges: self.u64()?,
            replacements: self.u64()?,
            rejected: self.u64()?,
            reevals: self.u64()?,
        })
    }

    fn genome(&mut self) -> Result<Genome, String> {
        let id = self.u64()?;
        let has_parent = self.u8()?;
        let parent_id = self.u64()?;
        let parent = match has_parent {
            0 => None,
            1 => Some(parent_id),
            other => return Err(format!("bad parent tag {other}")),
        };
        let emitter = self.u8()?;
        Ok(Genome {
            id,
            parent,
            emitter,
            params: self.f32s()?,
        })
    }

    fn elite(&mut self) -> Result<Elite, String> {
        let genome = self.genome()?;
        let fitness_mean = self.f32()?;
        let fitness_m2 = self.f32()?;
        if !fitness_mean.is_finite() || !fitness_m2.is_finite() {
            return Err("elite fitness is not finite".into());
        }
        let evals = self.u32()?;
        let descriptor = self.f32s()?;
        let signatures = self.u64s()?;
        let bonus = self.f32()?;
        let born = self.u64()?;
        let last_eval = self.u64()?;
        let moved_votes = match self.u8()? {
            0 => None,
            1 => Some((self.u32()? as usize, self.u8()?)),
            other => return Err(format!("bad moved_votes tag {other}")),
        };
        Ok(Elite {
            genome,
            fitness_mean,
            fitness_m2,
            evals,
            descriptor,
            signatures,
            bonus,
            born,
            last_eval,
            moved_votes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{Genome as G, Settle};
    use crate::nn::NetSpec;
    use crate::rules::Axis;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn dir(label: &str) -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "xswarm-store-{label}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn spec() -> NetSpec {
        NetSpec {
            version: 1,
            obs_len: 2,
            actions: 2,
            memory: 1,
            hidden: vec![2],
            norm_version: 1,
        }
    }

    fn sample() -> Archive {
        let axes = [Axis {
            name: "x",
            min: 0.0,
            max: 4.0,
            bins: 4,
        }];
        let mut archive = Archive::new("explorer", &axes, spec(), 99);
        let g = G {
            id: 3,
            parent: Some(1),
            emitter: 1,
            params: vec![0.25, -0.5],
        };
        assert_eq!(
            archive.offer_new(g, 1.5, vec![0.5], vec![7, 7], 0.25, 4),
            Settle::Inserted
        );
        let challenger = G {
            id: 8,
            parent: None,
            emitter: 2,
            params: vec![1.0, 2.0],
        };
        assert_eq!(
            archive.offer_new(challenger, 9.0, vec![0.5], vec![9], 1.0, 5),
            Settle::Challenged
        );
        archive.episodes = 6;
        archive.stats[1].insertions = 1;
        archive
    }

    #[test]
    fn archive_and_discoveries_round_trip_and_match_bytes() {
        let archive = sample();
        let rng = [1u64, 2, 3, 4];
        let bytes = encode_archive(&archive, rng);
        assert_eq!(encode_archive(&archive, rng), bytes);
        let (loaded, loaded_rng) = decode_archive(&bytes).unwrap();
        assert_eq!(loaded_rng, rng);
        assert_eq!(loaded.rules, "explorer");
        assert!(loaded.axes_compatible(&archive.axes));
        assert!(loaded.spec.compatible(&archive.spec));
        assert_eq!(loaded.episodes, 6);
        assert_eq!(loaded.run_seed, 99);
        assert_eq!(loaded.elite_count(), 1);
        let elite = loaded.elite(0).unwrap();
        assert_eq!(elite.genome.id, 3);
        assert_eq!(elite.genome.parent, Some(1));
        assert_eq!(elite.genome.params, vec![0.25, -0.5]);
        assert!((elite.fitness_mean - 1.5).abs() < 1e-6);
        assert_eq!(elite.signatures, vec![7]);
        assert_eq!(loaded.signature_count(7), 1);
        assert_eq!(loaded.challengers.len(), 1);
        assert!(!loaded.challengers[0].in_flight);
        assert_eq!(loaded.challengers[0].genome.id, 8);
        assert_eq!(loaded.stats[1].insertions, 1);

        let records = vec![
            Record {
                key: 9,
                count: 2,
                episode: 1,
                agent: "swarm02".into(),
            },
            Record {
                key: 3,
                count: 1,
                episode: 4,
                agent: "swarm01".into(),
            },
        ];
        let disc = encode_discoveries(&records);
        let back = decode_discoveries(&disc).unwrap();
        assert_eq!(back[0].key, 3);
        assert_eq!(back[1].key, 9);
        assert_eq!(back[1].agent, "swarm02");
    }

    #[test]
    fn files_round_trip_and_malformed_input_is_an_error() {
        let root = dir("round");
        let archive = sample();
        let rng = [9, 8, 7, 6];
        save_archive(&root.join(ARCHIVE_FILE), &archive, rng).unwrap();
        let (loaded, got) = load_archive(&root.join(ARCHIVE_FILE)).unwrap();
        assert_eq!(got, rng);
        assert_eq!(loaded.elite(0).unwrap().genome.id, 3);

        let records = vec![Record {
            key: 4,
            count: 1,
            episode: 2,
            agent: "a".into(),
        }];
        save_discoveries(&root.join(DISCOVERIES_FILE), &records).unwrap();
        assert_eq!(
            load_discoveries(&root.join(DISCOVERIES_FILE)).unwrap()[0].key,
            4
        );

        let status = Status {
            agents: 3,
            paused: true,
            episodes: 12,
            coverage: 0.25,
            qd_score: 1.5,
            best: Some(4.0),
            discoveries: 2,
            emitters: vec![
                (
                    "random".into(),
                    EmitterStats {
                        insertions: 4,
                        ..EmitterStats::default()
                    },
                ),
                ("gaussian".into(), EmitterStats::default()),
                (
                    "isoline".into(),
                    EmitterStats {
                        reevals: 3,
                        ..EmitterStats::default()
                    },
                ),
            ],
            eps_per_min: 8.5,
            updated_unix: 1_700_000_000,
        };
        save_status(&root.join(STATUS_FILE), &status).unwrap();
        let back = load_status(&root.join(STATUS_FILE)).unwrap();
        assert_eq!(back.agents, 3);
        assert!(back.paused);
        assert_eq!(back.episodes, 12);
        assert!((back.coverage - 0.25).abs() < 1e-5);
        assert_eq!(back.best.unwrap() as i32, 4);
        let random = back.emitters.iter().find(|e| e.0 == "random").unwrap();
        assert_eq!(random.1.insertions, 4);
        let iso = back.emitters.iter().find(|e| e.0 == "isoline").unwrap();
        assert_eq!(iso.1.reevals, 3);
        assert_eq!(back.updated_unix, 1_700_000_000);

        assert!(decode_archive(b"nope").is_err());
        assert!(
            decode_archive(b"XSWARM\0\x01").is_err(),
            "truncated after magic"
        );
        let mut junk = encode_archive(&archive, rng);
        junk.push(1);
        assert!(decode_archive(&junk).unwrap_err().contains("trailing"));
        assert!(decode_discoveries(b"XSDISC\0\x02").is_err());
        assert!(
            decode_discoveries(&[0, 1, 2, 3])
                .unwrap_err()
                .contains("magic")
        );
        let mut short = DISC_MAGIC.to_vec();
        short.extend_from_slice(&1u32.to_le_bytes());
        assert!(decode_discoveries(&short).is_err());
        let _ = SystemTime::now().duration_since(UNIX_EPOCH);
        let _ = fs::remove_dir_all(&root);
    }
}
