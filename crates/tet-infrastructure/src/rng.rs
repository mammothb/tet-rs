use rand::{Rng as _, SeedableRng as _};
use tet_domain::Rng;

pub type InnerRng = rand::rngs::SmallRng;

pub struct SmallRng(InnerRng);

impl SmallRng {
    pub fn new(seed: Option<u64>) -> (u64, Self) {
        let seed = seed.unwrap_or_else(|| rand::random());
        (seed, Self(InnerRng::seed_from_u64(seed)))
    }
}

impl Rng for SmallRng {
    fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }
}
