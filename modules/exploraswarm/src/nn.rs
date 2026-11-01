// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! A small multilayer perceptron stored as one flat parameter vector.
//!
//! Layout, layer by layer, input first: for a layer with `fan_in` inputs and `fan_out` outputs,
//! `fan_out * fan_in` weights in row-major order (output row, then input), then `fan_out` biases.
//! Hidden layers use tanh. The output layer is `actions` logits (linear) followed by `memory`
//! values (tanh). A later gradient step can address any weight by this same order.
//!
//! Weights are drawn from a normal scaled by `1/√fan_in`; biases start at 0.

use crate::rng::Rng;
use crate::senses;

/// Shape of one network. Stored in the archive header; a mismatch refuses the file.
#[derive(Clone, Debug, PartialEq)]
pub struct NetSpec {
    pub version: u32,
    pub obs_len: u16,
    pub actions: u16,
    pub memory: u16,
    pub hidden: Vec<u16>,
    pub norm_version: u32,
}

pub const NET_VERSION: u32 = 1;

impl NetSpec {
    /// The shape this module trains: observation length from [`senses`], two hidden layers of 32.
    pub fn standard() -> Self {
        Self {
            version: NET_VERSION,
            obs_len: senses::obs_len() as u16,
            actions: crate::actions::ACTION_COUNT as u16,
            memory: senses::MEMORY as u16,
            hidden: vec![32, 32],
            norm_version: senses::NORM_VERSION,
        }
    }

    pub fn compatible(&self, other: &NetSpec) -> bool {
        self.version == other.version
            && self.obs_len == other.obs_len
            && self.actions == other.actions
            && self.memory == other.memory
            && self.hidden == other.hidden
            && self.norm_version == other.norm_version
    }

    pub fn output_len(&self) -> usize {
        self.actions as usize + self.memory as usize
    }
}

/// How many floats [`forward`] reads.
pub fn param_count(spec: &NetSpec) -> usize {
    let mut n = 0usize;
    let mut fan_in = spec.obs_len as usize;
    for &h in &spec.hidden {
        let fan_out = h as usize;
        n += fan_out * fan_in + fan_out;
        fan_in = fan_out;
    }
    let fan_out = spec.output_len();
    n + fan_out * fan_in + fan_out
}

/// Draw a fresh network.
pub fn init(spec: &NetSpec, rng: &mut Rng) -> Vec<f32> {
    let mut params = vec![0.0; param_count(spec)];
    let mut cursor = 0usize;
    let mut fan_in = spec.obs_len as usize;
    let mut widths: Vec<usize> = spec.hidden.iter().map(|h| *h as usize).collect();
    widths.push(spec.output_len());
    for fan_out in widths {
        let scale = 1.0 / (fan_in as f64).sqrt();
        let weights = fan_out * fan_in;
        for _ in 0..weights {
            params[cursor] = (rng.normal() * scale) as f32;
            cursor += 1;
        }
        cursor += fan_out;
        fan_in = fan_out;
    }
    params
}

/// Reused activation buffers. The first `forward` sizes them; later calls do not allocate.
#[derive(Clone, Debug, Default)]
pub struct Scratch {
    a: Vec<f32>,
    b: Vec<f32>,
    out: Vec<f32>,
}

impl Scratch {
    pub fn new(spec: &NetSpec) -> Self {
        let mut s = Self::default();
        s.ensure(spec);
        s
    }

    fn ensure(&mut self, spec: &NetSpec) {
        let hidden = spec.hidden.iter().map(|h| *h as usize).max().unwrap_or(0);
        let out = spec.output_len();
        if self.a.len() < hidden {
            self.a.resize(hidden, 0.0);
        }
        if self.b.len() < hidden {
            self.b.resize(hidden, 0.0);
        }
        if self.out.len() < out {
            self.out.resize(out, 0.0);
        }
    }
}

/// Cursor and source selection for one forward pass. Kept together so the layer helpers stay small.
struct Walk<'a> {
    params: &'a [f32],
    cursor: usize,
    obs: &'a [f32],
    src_is_obs: bool,
    src_is_a: bool,
}

/// Run the network. `logits` has `actions` values; `memory` has `memory` values, already through tanh.
pub fn forward<'a>(
    params: &[f32],
    spec: &NetSpec,
    obs: &[f32],
    scratch: &'a mut Scratch,
) -> (&'a [f32], &'a [f32]) {
    scratch.ensure(spec);
    let actions = spec.actions as usize;
    let memory = spec.memory as usize;
    let mut fan_in = spec.obs_len as usize;
    let mut walk = Walk {
        params,
        cursor: 0,
        obs,
        src_is_obs: true,
        src_is_a: true,
    };

    for (layer, &width) in spec.hidden.iter().enumerate() {
        let fan_out = width as usize;
        let dest_is_a = layer % 2 == 0;
        affine_tanh(&mut walk, fan_in, fan_out, scratch, dest_is_a);
        walk.src_is_obs = false;
        walk.src_is_a = dest_is_a;
        fan_in = fan_out;
    }

    write_output(&mut walk, fan_in, actions, memory, scratch);
    debug_assert_eq!(walk.cursor, params.len());
    scratch.out.split_at(actions)
}

fn affine_tanh(
    walk: &mut Walk,
    fan_in: usize,
    fan_out: usize,
    scratch: &mut Scratch,
    dest_is_a: bool,
) {
    for o in 0..fan_out {
        let bias = walk.params[walk.cursor + fan_out * fan_in + o];
        let row = walk.cursor + o * fan_in;
        let mut sum = bias;
        for i in 0..fan_in {
            sum += walk.params[row + i] * read_src(walk, scratch, i);
        }
        let y = sum.tanh();
        if dest_is_a {
            scratch.a[o] = y;
        } else {
            scratch.b[o] = y;
        }
    }
    walk.cursor += fan_out * fan_in + fan_out;
}

fn write_output(
    walk: &mut Walk,
    fan_in: usize,
    actions: usize,
    memory: usize,
    scratch: &mut Scratch,
) {
    let fan_out = actions + memory;
    for o in 0..fan_out {
        let bias = walk.params[walk.cursor + fan_out * fan_in + o];
        let row = walk.cursor + o * fan_in;
        let mut sum = bias;
        for i in 0..fan_in {
            sum += walk.params[row + i] * read_src(walk, scratch, i);
        }
        scratch.out[o] = if o < actions { sum } else { sum.tanh() };
    }
    walk.cursor += fan_out * fan_in + fan_out;
}

fn read_src(walk: &Walk, scratch: &Scratch, i: usize) -> f32 {
    if walk.src_is_obs {
        walk.obs.get(i).copied().unwrap_or(0.0)
    } else if walk.src_is_a {
        scratch.a.get(i).copied().unwrap_or(0.0)
    } else {
        scratch.b.get(i).copied().unwrap_or(0.0)
    }
}

/// Argmax over legal actions. Ties keep the lowest index. An empty mask returns 0 (idle).
pub fn greedy(logits: &[f32], legal: &[bool]) -> u16 {
    let mut best_i = 0u16;
    let mut best = f32::NEG_INFINITY;
    let mut found = false;
    for (i, &ok) in legal.iter().enumerate() {
        if ok && i < logits.len() && (!found || logits[i] > best) {
            best = logits[i];
            best_i = i as u16;
            found = true;
        }
    }
    best_i
}

/// Masked softmax sample. Returns `(action, ln p)`. Temperature `<= 1e-4` is greedy with `ln p = 0`.
pub fn sample(logits: &[f32], legal: &[bool], temperature: f32, rng: &mut Rng) -> (u16, f32) {
    if temperature <= 1e-4 {
        return (greedy(logits, legal), 0.0);
    }
    let n = legal.len().min(logits.len()).min(64);
    let mut max = f32::NEG_INFINITY;
    let mut any = false;
    for i in 0..n {
        if legal[i] && logits[i] > max {
            max = logits[i];
            any = true;
        }
    }
    if !any {
        return (0, 0.0);
    }
    let inv_t = 1.0 / temperature;
    let mut exps = [0.0f32; 64];
    let mut sum = 0.0f32;
    for i in 0..n {
        if legal[i] {
            let e = ((logits[i] - max) * inv_t).exp();
            exps[i] = e;
            sum += e;
        }
    }
    if sum <= 0.0 || !sum.is_finite() {
        let a = greedy(logits, legal);
        return (a, 0.0);
    }
    let draw = (rng.f64() as f32) * sum;
    let mut acc = 0.0f32;
    let mut chosen = 0u16;
    for i in 0..n {
        if legal[i] {
            acc += exps[i];
            chosen = i as u16;
            if draw <= acc {
                break;
            }
        }
    }
    let logp = (logits[chosen as usize] - max) * inv_t - sum.ln();
    (chosen, logp)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> NetSpec {
        NetSpec {
            version: 1,
            obs_len: 3,
            actions: 4,
            memory: 2,
            hidden: vec![5, 5],
            norm_version: 1,
        }
    }

    #[test]
    fn param_count_matches_the_documented_layout() {
        let spec = tiny();
        // 5*3+5 + 5*5+5 + 6*5+6 = 20 + 30 + 36
        assert_eq!(param_count(&spec), 86);
        let std_spec = NetSpec::standard();
        let mut fan = std_spec.obs_len as usize;
        let mut expect = 0usize;
        for &h in &std_spec.hidden {
            expect += h as usize * fan + h as usize;
            fan = h as usize;
        }
        let out = std_spec.output_len();
        expect += out * fan + out;
        assert_eq!(param_count(&std_spec), expect);
    }

    #[test]
    fn forward_is_deterministic_and_memory_is_squashed() {
        let spec = tiny();
        let mut rng = Rng::from_seed(3);
        let params = init(&spec, &mut rng);
        assert_eq!(params.len(), param_count(&spec));
        let obs = [0.2, -0.5, 1.0];
        let mut scratch = Scratch::new(&spec);
        let (l1, m1) = forward(&params, &spec, &obs, &mut scratch);
        let logits = l1.to_vec();
        let memory = m1.to_vec();
        assert_eq!(logits.len(), 4);
        assert_eq!(memory.len(), 2);
        for &m in &memory {
            assert!(m.abs() <= 1.0 + 1e-5, "{m}");
        }
        let (l2, m2) = forward(&params, &spec, &obs, &mut scratch);
        assert_eq!(l2, logits.as_slice());
        assert_eq!(m2, memory.as_slice());
    }

    #[test]
    fn sample_never_picks_an_illegal_action_and_greedy_picks_the_max_legal() {
        let logits = [10.0, 0.1, 3.0, -1.0, 8.0];
        let legal = [false, true, false, true, false];
        // Index 0 (10) and index 4 (8) are illegal. Of the legal pair, index 1 wins.
        assert_eq!(greedy(&logits, &legal), 1);
        let mut rng = Rng::from_seed(11);
        for _ in 0..40 {
            let (a, logp) = sample(&logits, &legal, 0.8, &mut rng);
            assert!(legal[a as usize], "picked {a}");
            assert!(logp.is_finite());
        }
        let mut only = [false; 5];
        only[3] = true;
        for _ in 0..15 {
            let (a, _) = sample(&logits, &only, 1.0, &mut rng);
            assert_eq!(a, 3);
        }
    }
}
