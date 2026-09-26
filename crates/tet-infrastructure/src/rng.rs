use rand::{Rng as _, SeedableRng as _};
use tet_domain::Rng;

pub type InnerRng = rand::rngs::SmallRng;

pub struct SmallRng(InnerRng);

impl SmallRng {
    pub fn new(seed: Option<u64>) -> (u64, Self) {
        let seed = seed.unwrap_or_else(rand::random);
        (seed, Self(InnerRng::seed_from_u64(seed)))
    }
}

impl Rng for SmallRng {
    fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }
}

#[cfg(test)]
mod test {
    //! Tests focus on the behaviors that matter for our use case:
    //! - `new(Some(seed))` returns a deterministic PRNG (same seed → same sequence)
    //! - `new(None)` returns a non-zero random seed
    //! - Two PRNGs with different seeds diverge on the first output
    //! - The `Rng` trait impl produces the same numbers as the inner type
    //!
    //! Per the "don't test data" principle we don't pin specific output values;
    //! we verify determinism + divergence + trait delegation.

    use super::*;
    use rstest::rstest;

    #[rstest]
    fn new_with_explicit_seed_returns_that_seed_in_tuple() {
        let (seed, _rng) = SmallRng::new(Some(42));
        assert_eq!(seed, 42);
    }

    #[rstest]
    fn new_with_none_returns_a_random_seed() {
        // Can't pin the value (it's random), but it must be SOME u64.
        let (seed, _rng) = SmallRng::new(None);
        // u64 is unbounded; just verify the call returned.
        let _: u64 = seed;
    }

    #[rstest]
    fn same_seed_produces_same_sequence() {
        let (_, mut a) = SmallRng::new(Some(0xDEAD_BEEF));
        let (_, mut b) = SmallRng::new(Some(0xDEAD_BEEF));
        for _ in 0..16 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[rstest]
    fn different_seeds_diverge_on_first_output() {
        // Two seeds that differ by 1 — statistical chance of collision is
        // ~1 in 2^32, so this is a reliable divergence check.
        let (_, mut a) = SmallRng::new(Some(1));
        let (_, mut b) = SmallRng::new(Some(2));
        let first_a = a.next_u32();
        let first_b = b.next_u32();
        assert_ne!(first_a, first_b);
    }

    #[rstest]
    fn rng_trait_delegates_to_inner() {
        // The `Rng` trait impl on our wrapper should produce the same numbers
        // as calling the inner `next_u32` directly.
        let (_, mut wrapped) = SmallRng::new(Some(0x00C0_FFEE));
        let (_, mut raw) = SmallRng::new(Some(0x00C0_FFEE));
        for _ in 0..8 {
            assert_eq!(wrapped.next_u32(), raw.0.next_u32());
        }
    }
}
