use tet_application::{
    BotError, BotMove, BotSchedule, BotSpeed, GameTime, InvalidBotSpeed, PlayerSnapshot,
};
use tet_infrastructure::bot::BotType;
use tokio::sync::oneshot;

use crate::bot::worker::BotHandle;

type PendingCalculation = (GameTime, oneshot::Receiver<Result<Vec<BotMove>, BotError>>);

/// Summary of fixed-speed commit activity.
pub(crate) struct FixedCommitStats {
    pub(crate) configured_pps: f32,
    pub(crate) commits: u64,
    pub(crate) effective_pps: f64,
}

/// Runtime state for one in-process bot.
pub(crate) struct BotRuntime {
    player_idx: usize,
    bot_type: BotType,
    name: String,
    schedule: BotSchedule,
    started_at: GameTime,
    fixed_commit_count: u64,
    handle: BotHandle,
    pending_calculation: Option<PendingCalculation>,
    prepared: Option<Vec<BotMove>>,
}

impl BotRuntime {
    pub(crate) fn new(
        player_idx: usize,
        bot_type: BotType,
        name: String,
        speed: BotSpeed,
        handle: BotHandle,
        now: GameTime,
    ) -> Result<Self, InvalidBotSpeed> {
        Ok(Self {
            player_idx,
            bot_type,
            name,
            schedule: BotSchedule::new(speed, now)?,
            started_at: now,
            fixed_commit_count: 0,
            handle,
            pending_calculation: None,
            prepared: None,
        })
    }

    pub(crate) fn player_idx(&self) -> usize {
        self.player_idx
    }

    pub(crate) fn bot_type(&self) -> BotType {
        self.bot_type
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Whether this bot has neither a pending calculation nor a prepared move.
    pub(crate) fn is_idle(&self) -> bool {
        self.pending_calculation.is_none() && self.prepared.is_none()
    }

    /// Whether this bot holds a calculated move waiting to be committed.
    #[cfg(test)]
    pub(crate) fn has_prepared(&self) -> bool {
        self.prepared.is_some()
    }

    /// Start calculating the next move, unless calculation or a prepared move
    /// already exists.
    pub(crate) fn request_preparation(&mut self, snapshot: PlayerSnapshot, now: GameTime) {
        if !self.is_idle() {
            return;
        }

        let reply = self.handle.request_suggestion(snapshot);
        self.pending_calculation = Some((now, reply));
    }

    /// Poll the pending calculation.
    ///
    /// A successful calculation is stored as the bot's prepared move. Errors
    /// leave no prepared move behind.
    pub(crate) fn poll_calculation(&mut self) -> Option<(GameTime, Result<(), BotError>)> {
        let (sent_at, mut reply) = self.pending_calculation.take()?;
        match reply.try_recv() {
            Ok(Ok(moves)) => {
                self.prepared = Some(moves);
                Some((sent_at, Ok(())))
            }
            Ok(Err(error)) => Some((sent_at, Err(error))),
            Err(oneshot::error::TryRecvError::Empty) => {
                self.pending_calculation = Some((sent_at, reply));
                None
            }
            Err(oneshot::error::TryRecvError::Closed) => Some((sent_at, Err(BotError::Exited))),
        }
    }

    /// Take the prepared move only when a movement permit is available.
    pub(crate) fn take_prepared_if_permitted(&mut self) -> Option<Vec<BotMove>> {
        if !self.schedule.has_permit() {
            return None;
        }
        self.prepared.take()
    }

    /// Refresh fixed-speed deadlines. Returns `true` when a full cadence
    /// interval was missed.
    pub(crate) fn refresh_schedule(&mut self, now: GameTime) -> bool {
        self.schedule.refresh_fixed_deadline(now)
    }

    /// Configured PPS for a fixed-speed bot.
    pub(crate) fn configured_pps(&self) -> Option<f32> {
        match self.schedule.speed() {
            BotSpeed::TurnBased => None,
            BotSpeed::FixedPps { pieces_per_second } => Some(pieces_per_second),
        }
    }

    /// Record a successful fixed-speed commit and calculate effective PPS.
    pub(crate) fn record_fixed_commit(&mut self, now: GameTime) -> Option<FixedCommitStats> {
        let configured_pps = self.configured_pps()?;
        self.fixed_commit_count += 1;
        let elapsed_secs = now.saturating_millis_since(self.started_at) as f64 / 1_000.0;
        let effective_pps = if elapsed_secs > 0.0 {
            self.fixed_commit_count as f64 / elapsed_secs
        } else {
            0.0
        };
        Some(FixedCommitStats {
            configured_pps,
            commits: self.fixed_commit_count,
            effective_pps,
        })
    }

    /// Record a piece lock and grant a turn permit when the human locked.
    pub(crate) fn on_piece_locked(&mut self, player_idx: usize, human_idx: usize) {
        if player_idx == human_idx {
            self.schedule.on_human_piece_locked();
        }
    }

    /// Whether this bot currently has a movement permit.
    #[cfg(test)]
    pub(crate) fn has_permit(&self) -> bool {
        self.schedule.has_permit()
    }

    /// Consume one movement permit after a successful commit.
    pub(crate) fn consume_permit(&mut self) -> bool {
        self.schedule.consume_permit()
    }

    /// Drain errors reported by the worker outside the suggestion reply.
    pub(crate) fn drain_worker_errors(&mut self) -> Vec<BotError> {
        self.handle.drain_errors()
    }

    /// Tell the bot worker to stop.
    pub(crate) fn stop(&mut self) {
        self.handle.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use async_trait::async_trait;
    use tet_application::{BotPieceLocation, BotSpin, BotTransport, Phase, Piece};
    use tet_domain::{Board, MinoType, Orientation, Ruleset};

    struct StubBot {
        fail_update: bool,
        fail_suggest: bool,
        moves: Vec<BotMove>,
        update_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl BotTransport for StubBot {
        fn start(&mut self, _rules: &Ruleset) -> Result<(), BotError> {
            Ok(())
        }

        async fn update(&mut self, _snapshot: &PlayerSnapshot) -> Result<(), BotError> {
            self.update_count.fetch_add(1, Ordering::SeqCst);
            if self.fail_update {
                Err(BotError::Io)
            } else {
                Ok(())
            }
        }

        async fn suggest(&mut self) -> Result<Vec<BotMove>, BotError> {
            if self.fail_suggest {
                Err(BotError::Exited)
            } else {
                Ok(self.moves.clone())
            }
        }

        async fn stop(&mut self) {}
    }

    fn snapshot() -> PlayerSnapshot {
        PlayerSnapshot {
            board: Board::new(10, 25),
            queue: Vec::new(),
            hold: None,
            combo: 0,
            b2b: false,
            current: Piece::spawn(MinoType::T),
            phase: Phase::Playing,
            pending_garbage: Vec::new(),
        }
    }

    fn suggested_move() -> BotMove {
        BotMove {
            location: BotPieceLocation {
                kind: MinoType::T,
                orientation: Orientation::North,
                x: 4,
                y: 19,
            },
            spin: BotSpin::None,
            hold: false,
        }
    }

    fn runtime(fail_update: bool, fail_suggest: bool) -> (BotRuntime, Arc<AtomicUsize>) {
        runtime_with_moves(fail_update, fail_suggest, Vec::new())
    }

    fn runtime_with_moves(
        fail_update: bool,
        fail_suggest: bool,
        moves: Vec<BotMove>,
    ) -> (BotRuntime, Arc<AtomicUsize>) {
        runtime_with_speed(fail_update, fail_suggest, moves, BotSpeed::TurnBased)
    }

    fn runtime_with_speed(
        fail_update: bool,
        fail_suggest: bool,
        moves: Vec<BotMove>,
        speed: BotSpeed,
    ) -> (BotRuntime, Arc<AtomicUsize>) {
        let update_count = Arc::new(AtomicUsize::new(0));
        let bot = StubBot {
            fail_update,
            fail_suggest,
            moves,
            update_count: Arc::clone(&update_count),
        };
        let handle =
            crate::bot::spawn_bot_worker(&tokio::runtime::Handle::current(), Box::new(bot));
        let runtime = BotRuntime::new(
            7,
            BotType::Blockfish,
            "Blockfish".to_owned(),
            speed,
            handle,
            GameTime::ZERO,
        )
        .expect("valid speed");
        (runtime, update_count)
    }

    async fn wait_for_calculation(runtime: &mut BotRuntime) -> (GameTime, Result<(), BotError>) {
        for _ in 0..100 {
            if let Some(result) = runtime.poll_calculation() {
                return result;
            }
            tokio::task::yield_now().await;
        }
        panic!("calculation did not complete");
    }

    #[tokio::test]
    async fn request_creates_one_pending_calculation() {
        let (mut runtime, update_count) = runtime(false, false);
        runtime.request_preparation(snapshot(), GameTime::ZERO);
        assert!(!runtime.is_idle());

        let _ = wait_for_calculation(&mut runtime).await;

        assert_eq!(update_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn duplicate_request_does_not_start_second_calculation() {
        let (mut runtime, update_count) = runtime(false, false);
        runtime.request_preparation(snapshot(), GameTime::ZERO);
        runtime.request_preparation(snapshot(), GameTime::ZERO);

        let _ = wait_for_calculation(&mut runtime).await;

        assert_eq!(update_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn completed_calculation_is_held_as_prepared_move() {
        let move_to_prepare = suggested_move();
        let (mut runtime, _) = runtime_with_moves(false, false, vec![move_to_prepare]);
        runtime.request_preparation(snapshot(), GameTime::from_millis(100));

        let (sent_at, result) = wait_for_calculation(&mut runtime).await;

        assert_eq!(sent_at, GameTime::from_millis(100));
        assert!(result.is_ok());
        assert!(runtime.has_prepared());
        assert!(!runtime.is_idle());
        assert!(runtime.take_prepared_if_permitted().is_none());
        assert!(runtime.has_prepared());

        runtime.on_piece_locked(7, 7);
        let prepared = runtime
            .take_prepared_if_permitted()
            .expect("permitted prepared move should exist");
        assert_eq!(prepared.len(), 1);
        assert_eq!(prepared[0].location.kind, move_to_prepare.location.kind);
        assert_eq!(
            prepared[0].location.orientation,
            move_to_prepare.location.orientation
        );
        assert_eq!(prepared[0].location.x, move_to_prepare.location.x);
        assert_eq!(prepared[0].location.y, move_to_prepare.location.y);
        assert!(!runtime.has_prepared());
        assert!(runtime.is_idle());
    }

    #[tokio::test]
    async fn prepared_move_blocks_new_calculation() {
        let (mut runtime, update_count) = runtime_with_moves(false, false, vec![suggested_move()]);
        runtime.request_preparation(snapshot(), GameTime::ZERO);
        let _ = wait_for_calculation(&mut runtime).await;
        assert!(runtime.has_prepared());

        runtime.request_preparation(snapshot(), GameTime::ZERO);

        assert_eq!(update_count.load(Ordering::SeqCst), 1);
        assert!(runtime.has_prepared());
    }

    #[tokio::test]
    async fn taking_prepared_move_allows_next_calculation() {
        let (mut runtime, update_count) = runtime_with_moves(false, false, vec![suggested_move()]);
        runtime.request_preparation(snapshot(), GameTime::ZERO);
        let _ = wait_for_calculation(&mut runtime).await;
        runtime.on_piece_locked(7, 7);
        let _ = runtime
            .take_prepared_if_permitted()
            .expect("prepared move should exist");

        runtime.request_preparation(snapshot(), GameTime::ZERO);
        let _ = wait_for_calculation(&mut runtime).await;

        assert_eq!(update_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn bot_lock_does_not_grant_turn_permit() {
        let (mut runtime, _) = runtime(false, false);

        runtime.on_piece_locked(8, 7);

        assert!(!runtime.has_permit());
    }

    #[tokio::test]
    async fn human_lock_grants_one_consumable_turn_permit() {
        let (mut runtime, _) = runtime(false, false);

        runtime.on_piece_locked(7, 7);

        assert!(runtime.has_permit());
        assert!(runtime.consume_permit());
        assert!(!runtime.has_permit());
        assert!(!runtime.consume_permit());
    }

    #[tokio::test]
    async fn fixed_deadline_creates_permit_without_human_lock() {
        let (mut runtime, _) = runtime_with_speed(
            false,
            false,
            Vec::new(),
            BotSpeed::FixedPps {
                pieces_per_second: 2.0,
            },
        );

        assert!(!runtime.refresh_schedule(GameTime::ZERO));
        assert!(runtime.has_permit());
    }

    #[tokio::test]
    async fn fixed_bot_waits_for_prepared_move_without_human_lock() {
        let (mut runtime, _) = runtime_with_speed(
            false,
            false,
            vec![suggested_move()],
            BotSpeed::FixedPps {
                pieces_per_second: 2.0,
            },
        );
        runtime.refresh_schedule(GameTime::ZERO);
        runtime.request_preparation(snapshot(), GameTime::ZERO);

        assert!(runtime.take_prepared_if_permitted().is_none());
        let _ = wait_for_calculation(&mut runtime).await;
        assert!(runtime.take_prepared_if_permitted().is_some());
    }

    #[tokio::test]
    async fn fixed_deadline_reports_full_period_miss() {
        let (mut runtime, _) = runtime_with_speed(
            false,
            false,
            Vec::new(),
            BotSpeed::FixedPps {
                pieces_per_second: 2.0,
            },
        );

        assert!(!runtime.refresh_schedule(GameTime::ZERO));
        assert!(runtime.refresh_schedule(GameTime::from_millis(1_000)));
    }

    #[tokio::test]
    async fn fixed_commit_reports_effective_pps() {
        let (mut runtime, _) = runtime_with_speed(
            false,
            false,
            Vec::new(),
            BotSpeed::FixedPps {
                pieces_per_second: 2.0,
            },
        );

        let stats = runtime
            .record_fixed_commit(GameTime::from_millis(1_000))
            .expect("fixed bot should report commit stats");
        assert_eq!(stats.commits, 1);
        assert!((stats.effective_pps - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn suggestion_error_leaves_no_prepared_move() {
        let (mut runtime, _) = runtime(false, true);
        runtime.request_preparation(snapshot(), GameTime::ZERO);

        let (_, result) = wait_for_calculation(&mut runtime).await;

        assert!(matches!(result, Err(BotError::Exited)));
        assert!(!runtime.has_prepared());
        assert!(runtime.is_idle());
    }

    #[tokio::test]
    async fn worker_error_is_propagated() {
        let (mut runtime, _) = runtime(true, false);
        runtime.request_preparation(snapshot(), GameTime::ZERO);

        let (_, result) = wait_for_calculation(&mut runtime).await;
        assert!(result.is_ok());
        assert!(matches!(
            runtime.drain_worker_errors().as_slice(),
            [BotError::Io]
        ));
    }
}
