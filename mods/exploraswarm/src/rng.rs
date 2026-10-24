// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Seeded randomness: SplitMix64 expands a run seed, xoshiro256** draws the values.
//!
//! Every random choice in the swarm comes from one of these. A seed splits into an agent seed and
//! then an episode seed, so a run can be repeated. The saved state is the four xoshiro words.

/// xoshiro256** with a SplitMix64 seeder.
#[derive(Clone, Debug)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    /// Four state words from `seed` (any value, including 0).
    pub fn from_seed(seed: u64) -> Self {
        let mut sm = seed;
        let mut s = [0u64; 4];
        for word in &mut s {
            *word = splitmix(&mut sm);
        }
        if s == [0; 4] {
            s[0] = 0x9E37_79B9_7F4A_7C15;
        }
        Self { s }
    }

    /// Restore a checkpoint. An all-zero state (a corrupt file) is replaced so the generator moves.
    pub fn from_state(mut s: [u64; 4]) -> Self {
        if s == [0; 4] {
            s[0] = 0x9E37_79B9_7F4A_7C15;
        }
        Self { s }
    }

    pub fn state(&self) -> [u64; 4] {
        self.s
    }

    pub fn u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform in `[0, 1)`.
    pub fn f64(&mut self) -> f64 {
        let x = self.u64() >> 11;
        (x as f64) * (1.0 / ((1u64 << 53) as f64))
    }

    /// Standard normal, via Box-Muller. One sample per call (the paired sample is discarded) so
    /// the checkpoint stays the four state words.
    pub fn normal(&mut self) -> f64 {
        let u1 = self.f64().max(1e-12);
        let u2 = self.f64();
        let mag = (-2.0 * u1.ln()).sqrt();
        mag * (std::f64::consts::TAU * u2).cos()
    }

    pub fn index(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.u64() as usize) % n }
    }

    #[cfg(test)]
    pub fn choose<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            Some(&items[self.index(items.len())])
        }
    }

    /// Bernoulli trial.
    pub fn chance(&mut self, p: f64) -> bool {
        self.f64() < p
    }
}

fn splitmix(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// SplitMix-style mix of several `u64`s. Rules use this for behaviour signatures.
pub fn mix(parts: &[u64]) -> u64 {
    let mut h = 0x9E37_79B9_7F4A_7C15u64;
    for &p in parts {
        h ^= p.wrapping_add(0x9E37_79B9_7F4A_7C15);
        h = splitmix(&mut h);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::from_seed(42);
        let mut b = Rng::from_seed(42);
        let mut seq_a = [0u64; 8];
        let mut seq_b = [0u64; 8];
        for i in 0..8 {
            seq_a[i] = a.u64();
            seq_b[i] = b.u64();
        }
        assert_eq!(seq_a, seq_b);
        assert_ne!(Rng::from_seed(1).u64(), Rng::from_seed(2).u64());
    }

    #[test]
    fn zero_seed_still_moves() {
        let mut rng = Rng::from_seed(0);
        let a = rng.u64();
        let b = rng.u64();
        assert_ne!(a, b);
    }

    #[test]
    fn state_round_trips() {
        let mut rng = Rng::from_seed(99);
        let _ = rng.u64();
        let saved = rng.state();
        let mut again = Rng::from_state(saved);
        assert_eq!(rng.u64(), again.u64());
        assert_eq!(rng.normal().to_bits(), again.normal().to_bits());
    }

    #[test]
    fn normal_is_finite_and_choose_stays_inside() {
        let mut rng = Rng::from_seed(7);
        for _ in 0..32 {
            assert!(rng.normal().is_finite());
        }
        let items = [10, 20, 30];
        for _ in 0..20 {
            let v = *rng.choose(&items).unwrap();
            assert!(items.contains(&v));
        }
        assert!(rng.choose::<u8>(&[]).is_none());
    }

    #[test]
    fn mix_is_order_sensitive_and_stable() {
        assert_eq!(mix(&[1, 2, 3]), mix(&[1, 2, 3]));
        assert_ne!(mix(&[1, 2, 3]), mix(&[3, 2, 1]));
        assert_ne!(mix(&[1, 2]), mix(&[1, 2, 0]));
    }
}
