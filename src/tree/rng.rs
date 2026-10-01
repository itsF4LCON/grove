//! Small deterministic PRNG (SplitMix64) so every repo always grows the same tree.

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: u32) -> u32 {
        assert!(n > 0, "below(0)");
        (self.next_u64() % n as u64) as u32
    }

    /// Inclusive on both ends.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        assert!(lo <= hi);
        lo + self.below((hi - lo + 1) as u32) as i32
    }

    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u32) as usize]
    }
}

/// FNV-1a, 64-bit.
pub fn hash_str(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let a: Vec<u64> = { let mut r = Rng::new(42); (0..5).map(|_| r.next_u64()).collect() };
        let b: Vec<u64> = { let mut r = Rng::new(42); (0..5).map(|_| r.next_u64()).collect() };
        assert_eq!(a, b);
        assert_ne!(Rng::new(1).next_u64(), Rng::new(2).next_u64());
    }

    #[test]
    fn ranges_stay_in_bounds() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let v = r.range(-2, 2);
            assert!((-2..=2).contains(&v));
            assert!(r.below(3) < 3);
            let u = r.unit();
            assert!((0.0..1.0).contains(&u));
        }
        assert!(!Rng::new(1).chance(0.0));
        assert!(Rng::new(1).chance(1.0));
    }

    #[test]
    fn hash_is_stable() {
        assert_eq!(hash_str("itsF4LCON/grove"), hash_str("itsF4LCON/grove"));
        assert_ne!(hash_str("a/b"), hash_str("a/c"));
        assert_eq!(hash_str(""), 0xcbf2_9ce4_8422_2325);
    }
}
