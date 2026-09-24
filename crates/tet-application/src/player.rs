use tet_domain::{Board, MinoType, Queue, Rng};

use crate::{Piece, ports::bot::BotTransport};

#[derive(PartialEq, Eq)]
pub enum Phase {
    Playing,
    GameOver,
}

pub enum Controller {
    Human,
    Bot(Box<dyn BotTransport>),
}

/// Garbage that has been launched at this player but not yet inserted.
/// `delay_remaining` counts down each frame; when it reaches 0, the rows are
/// pushed onto the board. While in flight, the player can cancel rows by
/// clearing lines.
pub struct PendingGarbage {
    pub count: u8,
    pub hole: u8,
    pub delay_remaining: u8,
}

pub struct Player<R: Rng> {
    pub board: Board,
    pub current: Piece,
    pub queue: Queue<R>,
    pub hold: Option<MinoType>,
    pub hold_used: bool, // true after a hold swap; reset on next spawn-from-queue
    pub score: i32,
    pub lines: u32,
    pub combo: i32,
    pub b2b: bool,
    pub lock_delay: u16,
    pub phase: Phase,
    pub controller: Controller,
    /// In-flight garbage targeting this player. New attacks are pushed onto
    /// the back; cancellation peels from the back (most recent first).
    pub pending_garbage: Vec<PendingGarbage>,
}
