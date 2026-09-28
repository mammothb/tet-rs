use std::time::{Duration, Instant};

use tet_domain::{Board, MinoType, Queue, Rng, Ruleset};

use crate::{
    GRAVITY_MS, LOCK_DELAY_FRAMES, Piece, TickResult,
    ports::bot::BotTransport,
    tick::{TSpinStatus, post_lock, step_gravity, try_lock},
};

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
    /// Time of the last gravity shift. Used by `step_player` to decide when
    /// the active piece should fall one cell. Per-player state — each
    /// `Player` ticks its own clock so gravity cadence stays in the
    /// application layer rather than the composition root.
    pub last_gravity_at: Instant,
}

impl<R: Rng> Player<R> {
    /// Build a new human-controlled player with an empty board, a fresh
    /// 5-piece queue, and the given RNGs for queue generation and attack
    /// hole column selection. The active piece is drawn from the queue
    /// (first bag piece).
    ///
    /// Two RNGs because `Player<R>` stores both as owned values: one in
    /// `queue: Queue<R>`, the other as `attack_rng: R`. Since `R: Rng` doesn't
    /// require `Clone`, we can't share one.
    pub fn new_human(queue_rng: R, attack_rng: R) -> Self {
        let now = Instant::now();
        let mut queue = Queue::new(queue_rng, 5);
        let current = Piece::spawn(queue.take());
        Self {
            board: Board::new(10, 25),
            current,
            queue,
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
            last_gravity_at: now,
        }
    }

    /// Build a new bot-controlled player with the given `BotTransport`.
    /// Same two-RNG split as `new_human`. Active piece is drawn from the
    /// queue (first bag piece).
    pub fn new_bot(queue_rng: R, attack_rng: R, bot: Box<dyn BotTransport>) -> Self {
        let now = Instant::now();
        let mut queue = Queue::new(queue_rng, 5);
        let current = Piece::spawn(queue.take());
        Self {
            board: Board::new(10, 25),
            current,
            queue,
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
            last_gravity_at: now,
        }
    }

    /// Advance game state by one frame. Each `Player` ticks its own gravity
    /// clock — the composition root doesn't drive gravity timing. `now` is
    /// passed in so tests don't need to mock `Instant::now()`.
    pub fn step_player(&mut self, ruleset: &Ruleset, now: Instant) -> TickResult {
        let mut result = TickResult {
            lines_cleared: 0,
            tspin: TSpinStatus::None,
            piece_locked: false,
        };

        if self.phase != Phase::Playing {
            return result;
        }

        // 1. Gravity: shift down if GRAVITY_MS has elapsed since last.
        let gravity_interval = Duration::from_millis(u64::from(GRAVITY_MS));
        if now.duration_since(self.last_gravity_at) >= gravity_interval {
            step_gravity(self);
            self.last_gravity_at = now;
        }

        // 2. Lock-delay accounting + lock when delay expires.
        if crate::tick::piece_can_fall(self) {
            self.lock_delay = 0;
        } else {
            self.lock_delay += 1;
            if self.lock_delay >= LOCK_DELAY_FRAMES {
                result = try_lock(self);
                post_lock(self, &result, ruleset);
            }
        }

        result
    }
}
