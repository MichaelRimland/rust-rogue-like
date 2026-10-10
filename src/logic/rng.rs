//! A tiny deterministic pseudo random number generator.
//!
//! Every run is driven by a single seed, so a run can be reproduced exactly by
//! starting it with the same seed. SplitMix64 is small, fast and statistically
//! good enough for gameplay; it is not meant for cryptography.

/// SplitMix64 based pseudo random number generator.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Creates a generator from a seed. Equal seeds produce equal sequences.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Returns the next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform float in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        // 24 random bits fill the f32 mantissa exactly.
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform float in `[min, max)`.
    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next_f32()
    }

    /// Uniform integer in `[0, max)`. Panics if `max == 0`.
    pub fn below(&mut self, max: usize) -> usize {
        assert!(max > 0, "Rng::below called with max == 0");
        (self.next_u64() % max as u64) as usize
    }

    /// Returns `true` with probability `p`.
    pub fn chance(&mut self, p: f32) -> bool {
        self.next_f32() < p
    }

    /// Picks up to `k` distinct indices from `0..n` (partial Fisher-Yates shuffle).
    pub fn choose_distinct(&mut self, n: usize, k: usize) -> Vec<usize> {
        let mut pool: Vec<usize> = (0..n).collect();
        let k = k.min(n);
        for i in 0..k {
            let j = i + self.below(n - i);
            pool.swap(i, j);
        }
        pool.truncate(k);
        pool
    }

    /// Picks an index according to the given non-negative weights.
    /// Returns `None` if all weights are zero.
    pub fn weighted_index(&mut self, weights: &[f32]) -> Option<usize> {
        let total: f32 = weights.iter().copied().filter(|w| *w > 0.0).sum();
        if total <= 0.0 {
            return None;
        }
        let mut roll = self.next_f32() * total;
        for (i, w) in weights.iter().copied().enumerate() {
            if w <= 0.0 {
                continue;
            }
            if roll < w {
                return Some(i);
            }
            roll -= w;
        }
        // Floating point rounding: fall back to the last positive weight.
        weights.iter().rposition(|w| *w > 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn floats_stay_in_range() {
        let mut rng = Rng::new(7);
        for _ in 0..10_000 {
            let f = rng.next_f32();
            assert!((0.0..1.0).contains(&f));
            let r = rng.range_f32(-5.0, 5.0);
            assert!((-5.0..5.0).contains(&r));
        }
    }

    #[test]
    fn choose_distinct_has_no_duplicates() {
        let mut rng = Rng::new(3);
        for _ in 0..200 {
            let mut picked = rng.choose_distinct(10, 3);
            assert_eq!(picked.len(), 3);
            picked.sort_unstable();
            picked.dedup();
            assert_eq!(picked.len(), 3);
            assert!(picked.iter().all(|i| *i < 10));
        }
        assert_eq!(rng.choose_distinct(2, 5).len(), 2);
    }

    #[test]
    fn weighted_index_respects_zero_weights() {
        let mut rng = Rng::new(9);
        for _ in 0..1_000 {
            let i = rng.weighted_index(&[0.0, 1.0, 0.0, 2.0]).unwrap();
            assert!(i == 1 || i == 3);
        }
        assert_eq!(rng.weighted_index(&[0.0, 0.0]), None);
    }
}
