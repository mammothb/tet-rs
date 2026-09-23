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
