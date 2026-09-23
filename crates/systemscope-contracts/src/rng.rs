//! Deterministic randomness (`docs/m0-design.md` §5.2).
//!
//! The runtime owns one generator per component and hands it out through
//! `ctx.rng()`. Every derived value is computed by the provided methods here, so the
//! number of raw draws each call consumes is part of the contract.

use core::num::NonZeroU64;

/// A per-component random stream.
pub trait SimRng {
    /// Returns the next 64 uniformly distributed bits.
    fn next_u64(&mut self) -> u64;

    /// Returns a uniform integer in `0..n`.
    ///
    /// Rejection sampling: with `t = n.wrapping_neg() % n`, draw `x` until `x >= t`, then
    /// return `x % n`. This is part of the contract; implementations must not override it.
    fn below(&mut self, n: NonZeroU64) -> u64 {
        let n = n.get();
        let threshold = n.wrapping_neg() % n;
        loop {
            let x = self.next_u64();
            if x >= threshold {
                return x % n;
            }
        }
    }

    /// Returns `true` with probability `num / den` (always `true` when `num >= den`).
    fn chance(&mut self, num: u64, den: NonZeroU64) -> bool {
        self.below(den) < num
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Replays a fixed list of raw draws.
    struct Script(Vec<u64>);

    impl SimRng for Script {
        fn next_u64(&mut self) -> u64 {
            self.0.remove(0)
        }
    }

    fn nz(n: u64) -> NonZeroU64 {
        NonZeroU64::new(n).unwrap()
    }

    #[test]
    fn below_rejects_the_biased_low_zone() {
        // n = 2^63 + 1: t = 2^63 - 1, so draws below t are rejected.
        let n = (1u64 << 63) + 1;
        let t = n.wrapping_neg() % n;
        assert_eq!(t, (1u64 << 63) - 1);
        let mut rng = Script(vec![0, t - 1, t, 7]);
        assert_eq!(rng.below(nz(n)), t % n);
        assert_eq!(rng.0, [7]);
    }

    #[test]
    fn below_consumes_one_draw_for_powers_of_two() {
        let mut rng = Script(vec![u64::MAX, 5]);
        assert_eq!(rng.below(nz(8)), 7);
        assert_eq!(rng.below(nz(1)), 0);
        assert!(rng.0.is_empty());
    }

    #[test]
    fn chance_compares_against_below() {
        // For n = 10, t = 2^64 mod 10 = 6: draws 12 and 13 are accepted as 2 and 3.
        assert_eq!(10u64.wrapping_neg() % 10, 6);
        let mut rng = Script(vec![12, 13]);
        assert!(rng.chance(3, nz(10)));
        assert!(!rng.chance(3, nz(10)));
    }
}
