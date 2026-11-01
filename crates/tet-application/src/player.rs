use tet_domain::{Board, MinoType, Queue, Rng, Ruleset};

use crate::constants::{GRAVITY_MS, LOCK_DELAY_MS};
use crate::piece::Piece;
use crate::tick::{TSpinStatus, TickResult, post_lock, step_gravity, try_lock};
use crate::time::GameTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Playing,
    GameOver,
}

/// Scoring and combo tracking for one player
#[derive(Clone, Copy, Debug, Default)]
pub struct ScoreState {
    pub score: i32,
    pub lines: u32,
    pub combo: i32,
    pub b2b: bool,
}

/// Garbage that has been launched at this player but not yet inserted.
///
/// Garbage is queued: it lands on the player's next lock that does **not**
/// clear lines. A lock that clears lines cancels rows from this queue instead.
#[derive(Clone, Copy)]
pub struct PendingGarbage {
    /// Number of rows in this attack.
    pub count: u8,
    /// Column left open in each of those rows.
    pub hole: u8,
}

/// In-flight garbage queue and the RNG for attack hole column selection
pub struct GarbageState {
    /// In-flight garbage targeting this player. New attacks are pushed onto
    /// the back; cancellation peels from the back (most recent first).
    pub pending: Vec<PendingGarbage>,
    /// RNG used to generate attack hole columns.
    pub attack_rng: Box<dyn Rng>,
}

pub struct Player {
    pub board: Board,
    pub current: Piece,
    pub queue: Queue,
    pub hold: Option<MinoType>,
    pub hold_used: bool, // true after a hold swap; reset on next spawn-from-queue
    pub scoring: ScoreState,
    /// When the active piece first became grounded (couldn't fall). `None`
    /// while the piece is in the air. `step_player` locks the piece if this
    /// timestamp is older than `LOCK_DELAY_MS`.
    pub lock_grounded_at: Option<GameTime>,
    pub phase: Phase,
    /// True for a bot-controlled player.
    ///
    /// Bot pieces do not fall and never auto-lock: the host applies the bot's
    /// suggested placement and locks the piece explicitly. The piece therefore
    /// stays where it spawned until the bot answers, which keeps the board the
    /// bot planned on identical to the board it applies to.
    pub bot_controlled: bool,
    pub garbage: GarbageState,
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
            scoring: ScoreState::default(),
            lock_grounded_at: None,
            phase: Phase::Playing,
            bot_controlled: false,
            garbage: GarbageState {
                pending: Vec::new(),
                attack_rng,
            },
            last_gravity_at: GameTime::ZERO,
        }
    }

    /// Build a new bot-controlled player with the given `BotTransport`.
    /// Same two-RNG split as `new_human`. Active piece is drawn from the
    /// queue (first bag piece).
    #[must_use]
    pub fn new_bot(queue_rng: Box<dyn Rng>, attack_rng: Box<dyn Rng>) -> Self {
        let mut queue = Queue::new(queue_rng, 5);
        let current = Piece::spawn(queue.take());
        Self {
            board: Board::new(10, 25),
            current,
            queue,
            hold: None,
            hold_used: false,
            scoring: ScoreState::default(),
            lock_grounded_at: None,
            phase: Phase::Playing,
            bot_controlled: true,
            garbage: GarbageState {
                pending: Vec::new(),
                attack_rng,
            },
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

        // Bot pieces do not fall and never auto-lock. The host applies the
        // bot's suggested placement and locks the piece itself.
        if self.bot_controlled {
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

#[cfg(test)]
mod test {
    use super::*;

    use rstest::rstest;
    use tet_domain::Queue;

    use crate::time::GameTime;

    /// Deterministic RNG that cycles through 1..=1000. Same as `bag.rs`.
    struct StubRng {
        values: Vec<u32>,
        idx: usize,
    }

    impl Rng for StubRng {
        fn next_u32(&mut self) -> u32 {
            let v = self.values[self.idx % self.values.len()];
            self.idx += 1;
            v
        }
    }

    fn player(bot_controlled: bool) -> Player {
        let mut queue = Queue::new(
            Box::new(StubRng {
                values: (1..=1000).collect(),
                idx: 0,
            }),
            5,
        );
        let current = Piece::spawn(queue.take());
        Player {
            board: Board::new(10, 25),
            current,
            queue,
            hold: None,
            hold_used: false,
            scoring: ScoreState::default(),
            lock_grounded_at: None,
            phase: Phase::Playing,
            bot_controlled,
            garbage: GarbageState {
                pending: Vec::new(),
                attack_rng: Box::new(StubRng {
                    values: (1..=1000).collect(),
                    idx: 0,
                }),
            },
            last_gravity_at: GameTime::ZERO,
        }
    }

    /// A bot's piece must not fall: the board the bot plans on has to be the
    /// board it applies to.
    #[rstest]
    fn bot_piece_does_not_fall() {
        let mut p = player(true);
        let spawn = p.current.pos;
        // Well past the gravity interval (1 s) and the lock delay (500 ms).
        p.step_player(&Ruleset::guideline(), GameTime::from_millis(5_000));
        assert_eq!(p.current.pos, spawn);
    }

    /// A bot's piece must not auto-lock either — the host locks it when it
    /// applies the bot's move.
    #[rstest]
    fn bot_piece_does_not_auto_lock() {
        let mut p = player(true);
        // Ground the piece so the lock timer would start.
        p.current.pos = tet_domain::Vec2::new(3, -1);
        let result = p.step_player(&Ruleset::guideline(), GameTime::from_millis(5_000));
        assert!(!result.piece_locked);
        assert_eq!(p.current.pos, tet_domain::Vec2::new(3, -1));
    }

    /// A human's piece still falls.
    #[rstest]
    fn human_piece_falls() {
        let mut p = player(false);
        let spawn = p.current.pos;
        p.step_player(&Ruleset::guideline(), GameTime::from_millis(5_000));
        assert_eq!(p.current.pos, spawn + tet_domain::Vec2::new(0, -1));
    }
}
