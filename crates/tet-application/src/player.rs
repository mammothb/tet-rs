use tet_domain::{Board, MinoType, Queue, Rng};

use crate::{Piece, ports::bot::BotTransport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Clone, Copy)]
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
    /// RNG used to generate attack hole columns.
    pub attack_rng: R,
}

impl<R: Rng> Player<R> {
    /// Build a new human-controlled player with an empty board, a fresh
    /// 5-piece queue, and the given RNGs for queue generation and attack
    /// hole column selection. Spawns a T-piece as the active piece
    /// (matches guideline convention).
    ///
    /// Two RNGs because `Player<R>` stores both as owned values: one in
    /// `queue: Queue<R>`, the other as `attack_rng: R`. Since `R: Rng` doesn't
    /// require `Clone`, we can't share one.
    pub fn new_human(queue_rng: R, attack_rng: R) -> Self {
        Self {
            board: Board::new(10, 25),
            current: Piece::spawn(MinoType::T),
            queue: Queue::new(queue_rng, 5),
            hold: None,
            hold_used: false,
            score: 0,
            lines: 0,
            combo: 0,
            b2b: false,
            lock_delay: 0,
            phase: Phase::Playing,
            controller: Controller::Human,
            pending_garbage: Vec::new(),
            attack_rng,
        }
    }

    /// Build a new bot-controlled player with the given `BotTransport`.
    /// Same two-RNG split as `new_human`.
    pub fn new_bot(queue_rng: R, attack_rng: R, bot: Box<dyn BotTransport>) -> Self {
        Self {
            board: Board::new(10, 25),
            current: Piece::spawn(MinoType::T),
            queue: Queue::new(queue_rng, 5),
            hold: None,
            hold_used: false,
            score: 0,
            lines: 0,
            combo: 0,
            b2b: false,
            lock_delay: 0,
            phase: Phase::Playing,
            controller: Controller::Bot(bot),
            pending_garbage: Vec::new(),
            attack_rng,
        }
    }
}
