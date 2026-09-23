use std::collections::VecDeque;

use crate::MinoType;

pub trait Rng {
    fn next_u32(&mut self) -> u32;
}

const BAG_SIZE: usize = 7;
const ALL_PIECES: [MinoType; BAG_SIZE] = [
    MinoType::I,
    MinoType::O,
    MinoType::T,
    MinoType::L,
    MinoType::J,
    MinoType::S,
    MinoType::Z,
];

#[derive(Debug)]
pub struct Bag<R: Rng> {
    pieces: [MinoType; BAG_SIZE],
    idx: u8,
    rng: R,
}

impl<R: Rng> Bag<R> {
    pub fn new(rng: R) -> Self {
        let mut rng = rng;
        let mut pieces = ALL_PIECES;
        shuffle(&mut pieces, &mut rng);
        Self {
            pieces,
            idx: 0,
            rng,
        }
    }

    pub fn take(&mut self) -> MinoType {
        let p = self.pieces[self.idx as usize];
        self.idx += 1;
        if self.idx as usize == BAG_SIZE {
            self.pieces = ALL_PIECES;
            shuffle(&mut self.pieces, &mut self.rng);
            self.idx = 0;
        }
        p
    }
}

pub struct Queue<R: Rng> {
    bag: Bag<R>,
    buf: Vec<MinoType>,
}

impl<R: Rng> Queue<R> {
    pub fn new(rng: R, buf_size: u8) -> Self {
        let mut bag = Bag::new(rng);
        let mut buf = Vec::with_capacity(buf_size as usize);
        for _ in 0..buf_size {
            buf.push(bag.take());
        }
        Self { bag, buf }
    }

    pub fn peek(&self) -> &[MinoType] {
        &self.buf
    }

    pub fn take(&mut self) -> MinoType {
        let p = self.buf.remove(0);
        self.buf.push(self.bag.take());
        p
    }
}

fn shuffle<R: Rng + ?Sized>(arr: &mut [MinoType], rng: &mut R) {
    for i in (1..arr.len()).rev() {
        let j = (rng.next_u32() as usize) % (i + 1);
        arr.swap(i, j);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::collections::HashSet;

    use rstest::rstest;

    /// Deterministic RNG that cycles through a fixed sequence of u32 values.
    struct StubRng {
        values: Vec<u32>,
        idx: usize,
    }

    impl StubRng {
        fn counter() -> Self {
            Self {
                values: (1..=1000).collect(),
                idx: 0,
            }
        }
    }

    impl Rng for StubRng {
        fn next_u32(&mut self) -> u32 {
            let v = self.values[self.idx % self.values.len()];
            self.idx += 1;
            v
        }
    }

    // -------- Bag --------

    #[rstest]
    fn new_bag_contains_all_seven_pieces() {
        let bag = Bag::new(StubRng::counter());
        let unique: HashSet<MinoType> = bag.pieces.iter().copied().collect();
        assert_eq!(unique.len(), 7);
    }

    #[rstest]
    fn take_returns_first_piece_in_bag() {
        let bag = Bag::new(StubRng::counter());
        let first = bag.pieces[0];
        let mut bag = bag;
        assert_eq!(bag.take(), first);
    }

    #[rstest]
    fn take_advances_index() {
        let mut bag = Bag::new(StubRng::counter());
        let _ = bag.take();
        assert_eq!(bag.idx, 1);
        let _ = bag.take();
        assert_eq!(bag.idx, 2);
    }

    #[rstest]
    fn take_seven_times_yields_all_seven_pieces() {
        let mut bag = Bag::new(StubRng::counter());
        let mut seen = HashSet::new();
        for _ in 0..7 {
            seen.insert(bag.take());
        }
        assert_eq!(seen.len(), 7);
    }

    #[rstest]
    fn bag_refills_after_seven_takes() {
        let mut bag = Bag::new(StubRng::counter());
        for _ in 0..7 {
            bag.take();
        }
        // After 7 takes the bag should have refilled: idx back to 0 and
        // pieces still contains all 7 unique types.
        assert_eq!(bag.idx, 0);
        let unique: HashSet<MinoType> = bag.pieces.iter().copied().collect();
        assert_eq!(unique.len(), 7);
    }

    // -------- Queue --------

    #[rstest]
    #[case::one(1)]
    #[case::three(3)]
    #[case::five(5)]
    #[case::seven(7)]
    fn new_queue_has_requested_size(#[case] n: u8) {
        let q = Queue::new(StubRng::counter(), n);
        assert_eq!(q.buf.len(), n as usize);
    }

    #[rstest]
    fn new_queue_contains_unique_pieces() {
        let q = Queue::new(StubRng::counter(), 7);
        let unique: HashSet<MinoType> = q.buf.iter().copied().collect();
        assert_eq!(unique.len(), 7);
    }

    #[rstest]
    fn peek_does_not_consume() {
        let q = Queue::new(StubRng::counter(), 3);
        let _ = q.peek();
        let _ = q.peek();
        let _ = q.peek();
        assert_eq!(q.buf.len(), 3);
    }

    #[rstest]
    fn take_rotates_front_to_back() {
        let mut q = Queue::new(StubRng::counter(), 3);
        let initial = q.buf.clone();
        let taken = q.take();
        // Front of initial is removed and returned.
        assert_eq!(taken, initial[0]);
        // Remaining elements shift left; size is preserved.
        assert_eq!(&q.buf[..2], &initial[1..]);
        assert_eq!(q.buf.len(), 3);
    }

    #[rstest]
    fn queue_size_invariant_after_many_takes() {
        let mut q = Queue::new(StubRng::counter(), 5);
        for _ in 0..20 {
            q.take();
            assert_eq!(q.buf.len(), 5);
        }
    }

    #[rstest]
    fn queue_cycles_through_all_pieces_across_refills() {
        let mut q = Queue::new(StubRng::counter(), 5);
        let mut seen = HashSet::new();
        // 50 takes spans multiple bag refills; all 7 types must appear.
        for _ in 0..50 {
            seen.insert(q.take());
        }
        assert_eq!(seen.len(), 7);
    }
}
