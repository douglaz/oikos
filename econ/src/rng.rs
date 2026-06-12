//! Deterministic seeded RNG for reproducible M0 scenarios.

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let state = seed ^ 0x9E37_79B9_7F4A_7C15;
        // xorshift64* has a fixed point at 0 (it would then emit 0 forever);
        // the seed 0x9E37_79B9_7F4A_7C15 maps to that state, so guard against it.
        let state = if state == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            state
        };
        Self { state }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    #[test]
    fn rng_is_deterministic() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);

        for _ in 0..5 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn rng_avoids_degenerate_zero_state() {
        // The seed that would zero the internal state must not produce an all-zero stream.
        let mut r = Rng::new(0x9E37_79B9_7F4A_7C15);
        assert_ne!(r.next_u64(), 0);
        assert_ne!(r.next_u64(), 0);
    }
}
