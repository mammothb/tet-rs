use tet_domain::{Board, MinoType, Queue, Rng, Ruleset};

use crate::ports::bot::BotTransport;
use crate::tick::{TSpinStatus, post_lock, step_gravity, try_lock};
use crate::{GRAVITY_MS, GameTime, LOCK_DELAY_MS, Piece, TickResult};

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

pub struct Player {
    pub board: Board,
    pub current: Piece,
    pub queue: Queue,
    pub hold: Option<MinoType>,
    pub hold_used: bool, // true after a hold swap; reset on next spawn-from-queue
    pub score: i32,
    pub lines: u32,
    pub combo: i32,
    pub b2b: bool,
    /// When the active piece first became grounded (couldn't fall). `None`
    /// while the piece is in the air. `step_player` locks the piece if this
    /// timestamp is older than `LOCK_DELAY_MS`.
    pub lock_grounded_at: Option<GameTime>,
    pub phase: Phase,
    pub controller: Controller,
    /// In-flight garbage targeting this player. New attacks are pushed onto
    /// the back; cancellation peels from the back (most recent first).
    pub pending_garbage: Vec<PendingGarbage>,
    /// RNG used to generate attack hole columns.
    pub attack_rng: Box<dyn Rng>,
    /// Time of the last gravity shift. Used by `step_player` to decide when
    /// the active piece should fall one cell. Per-player state — each
    /// `Player` ticks its own clock so gravity cadence stays in the
    /// application layer rather than the composition root.
    pub last_gravity_at: GameTime,
}

impl Player {
    /// Build a new human-controlled player with an empty board, a fresh
    /// 5-piece queue, and the given RNGs for queue generation and attack
    /// hole column selection. The active piece is drawn from the queue
    /// (first bag piece).
    ///
    /// Two RNGs: one for queue, the other for attack hole column selection.
    #[must_use]
    pub fn new_human(queue_rng: Box<dyn Rng>, attack_rng: Box<dyn Rng>) -> Self {
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
            lock_grounded_at: None,
            phase: Phase::Playing,
            controller: Controller::Human,
            pending_garbage: Vec::new(),
            attack_rng,
            last_gravity_at: GameTime::ZERO,
        }
    }

    /// Build a new bot-controlled player with the given `BotTransport`.
    /// Same two-RNG split as `new_human`. Active piece is drawn from the
    /// queue (first bag piece).
    #[must_use]
    pub fn new_bot(
        queue_rng: Box<dyn Rng>,
        attack_rng: Box<dyn Rng>,
        bot: Box<dyn BotTransport>,
    ) -> Self {
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
            lock_grounded_at: None,
            phase: Phase::Playing,
            controller: Controller::Bot(bot),
            pending_garbage: Vec::new(),
            attack_rng,
            last_gravity_at: GameTime::ZERO,
        }
    }

    /// Advance game state by one frame. Each `Player` ticks its own gravity
    /// clock — the composition root doesn't drive gravity timing. `now` is
    /// passed in so tests can control time deterministically.
    ///
    /// **Lock delay is time-based**, not frame-based. The piece locks
    /// `LOCK_DELAY_MS` after it first touches the ground (or its last
    /// successful move/rotation while grounded). See `LOCK_DELAY_MS` for why.
    pub fn step_player(&mut self, ruleset: &Ruleset, now: GameTime) -> TickResult {
        let mut result = TickResult {
            lines_cleared: 0,
            tspin: TSpinStatus::None,
            piece_locked: false,
        };

        if self.phase != Phase::Playing {
            return result;
        }

        // 1. Gravity: shift down if GRAVITY_MS has elapsed since last.
        if now.saturating_millis_since(self.last_gravity_at) >= u64::from(GRAVITY_MS) {
            step_gravity(self);
            self.last_gravity_at = now;
        }

        // 2. Lock-delay: start the timer the first frame the piece is
        //    grounded; lock once `LOCK_DELAY_MS` has elapsed. Resets are
        //    handled by `apply_horizontal_input`, `apply_rotation_input`, and
        //    `try_hold` — they set `lock_grounded_at = Some(now)`
        //    on success, restarting the timer.
        if crate::tick::piece_can_fall(self) {
            // Piece is in the air — no lock timer running.
            self.lock_grounded_at = None;
        } else {
            // Piece is grounded — start the timer if first grounded frame,
            // otherwise check if the lock delay has elapsed.
            let grounded_at = *self.lock_grounded_at.get_or_insert(now);
            if now.saturating_millis_since(grounded_at) >= u64::from(LOCK_DELAY_MS) {
                result = try_lock(self);
                post_lock(self, &result, ruleset);
                // Clear the timer so a future piece (if any) starts fresh.
                self.lock_grounded_at = None;
            }
        }

        result
    }
}
