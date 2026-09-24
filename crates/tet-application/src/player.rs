use tet_domain::{Board, MinoType, Queue, Rng};

use crate::{Piece, ports::bot::BotTransport};

pub enum Phase {
    Playing,
    GameOver,
}

pub enum Controller {
    Human,
    Bot(Box<dyn BotTransport>),
}

pub struct Player<R: Rng> {
    pub board: Board,
    pub current: Piece,
    pub queue: Queue<R>,
    pub hold: Option<MinoType>,
    pub score: i32,
    pub lines: u32,
    pub combo: i32,
    pub b2b: bool,
    pub lock_delay: u16,
    pub phase: Phase,
    pub controller: Controller,
}
