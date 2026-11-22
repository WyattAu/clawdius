//! Deterministic PRNG for reproducible evolution runs (SplitMix64).
//!
//! Zero dependencies: quality-diversity selection needs only uniform
//! sampling and shuffling, both of which a seeded SplitMix64 provides.
//! A fixed seed makes an entire evolution run replayable — the
//! reproducibility property AlphaEvolve advertises, enforced by type.

/// SplitMix64 — small, fast, well-distributed. Seedable from any u64.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Create a generator from a seed.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Next 64 random bits.
    #[must_use]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform float in `[0, 1)`.
    #[must_use]
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform usize in `[0, n)`. Returns 0 when `n == 0`.
    #[must_use]
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    /// Pick a random element, or `None` when empty.
    #[must_use]
    pub fn choose<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            items.get(self.below(items.len()))
        }
    }

    /// Fisher-Yates shuffle, in place.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
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
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let hits = (0..16)
            .filter(|_| {
                let (x, y) = (a.next_u64(), b.next_u64());
                x == y
            })
            .count();
        assert!(hits < 4, "sequences should diverge (collided {hits}/16)");
    }

    #[test]
    fn f64_in_unit_range() {
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            let v = r.f64();
            assert!((0.0..1.0).contains(&v));
        }
    }

    #[test]
    fn below_respects_bound() {
        let mut r = Rng::new(9);
        for _ in 0..500 {
            assert!(r.below(3) < 3);
        }
        assert_eq!(r.below(0), 0);
    }

    #[test]
    fn shuffle_is_permutation() {
        let mut r = Rng::new(11);
        let mut items: Vec<usize> = (0..50).collect();
        r.shuffle(&mut items);
        items.sort_unstable();
        assert_eq!(items, (0..50).collect::<Vec<_>>());
    }
}
