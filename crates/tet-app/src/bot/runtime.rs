use tet_application::{BotError, BotMove, GameTime, PlayerSnapshot};
use tokio::sync::oneshot;

use crate::bot::worker::BotHandle;

type PendingSuggest = (GameTime, oneshot::Receiver<Result<Vec<BotMove>, BotError>>);

/// Runtime state for one in-process bot.
pub(crate) struct BotRuntime {
    player_idx: usize,
    handle: BotHandle,
    pending: Option<PendingSuggest>,
}

impl BotRuntime {
    pub(crate) fn new(player_idx: usize, handle: BotHandle) -> Self {
        Self {
            player_idx,
            handle,
            pending: None,
        }
    }

    pub(crate) fn player_idx(&self) -> usize {
        self.player_idx
    }

    /// Whether this bot can accept another suggestion request.
    pub(crate) fn is_idle(&self) -> bool {
        self.pending.is_none()
    }

    /// Request one suggestion, unless another request is already pending.
    pub(crate) fn request_suggestion(&mut self, snapshot: PlayerSnapshot, now: GameTime) {
        if !self.is_idle() {
            return;
        }

        let reply = self.handle.request_suggestion(snapshot);
        self.pending = Some((now, reply));
    }

    /// Poll the pending suggestion, returning its request time and result.
    pub(crate) fn poll_suggestion(&mut self) -> Option<(GameTime, Result<Vec<BotMove>, BotError>)> {
        let (sent_at, mut reply) = self.pending.take()?;
        match reply.try_recv() {
            Ok(result) => Some((sent_at, result)),
            Err(oneshot::error::TryRecvError::Empty) => {
                self.pending = Some((sent_at, reply));
                None
            }
            Err(oneshot::error::TryRecvError::Closed) => Some((sent_at, Err(BotError::Exited))),
        }
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
    use tet_application::{BotTransport, Phase, Piece};
    use tet_domain::{Board, MinoType, Ruleset};

    struct StubBot {
        fail_update: bool,
        fail_suggest: bool,
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
                Ok(Vec::new())
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

    fn runtime(fail_update: bool, fail_suggest: bool) -> (BotRuntime, Arc<AtomicUsize>) {
        let update_count = Arc::new(AtomicUsize::new(0));
        let bot = StubBot {
            fail_update,
            fail_suggest,
            update_count: Arc::clone(&update_count),
        };
        let handle =
            crate::bot::spawn_bot_worker(&tokio::runtime::Handle::current(), Box::new(bot));
        (BotRuntime::new(7, handle), update_count)
    }

    async fn wait_for_suggestion(
        runtime: &mut BotRuntime,
    ) -> (GameTime, Result<Vec<BotMove>, BotError>) {
        for _ in 0..100 {
            if let Some(result) = runtime.poll_suggestion() {
                return result;
            }
            tokio::task::yield_now().await;
        }
        panic!("suggestion did not complete");
    }

    #[tokio::test]
    async fn request_creates_one_pending_suggestion() {
        let (mut runtime, update_count) = runtime(false, false);
        runtime.request_suggestion(snapshot(), GameTime::ZERO);
        assert!(!runtime.is_idle());

        let _ = wait_for_suggestion(&mut runtime).await;

        assert_eq!(update_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn duplicate_request_does_not_start_second_calculation() {
        let (mut runtime, update_count) = runtime(false, false);
        runtime.request_suggestion(snapshot(), GameTime::ZERO);
        runtime.request_suggestion(snapshot(), GameTime::ZERO);

        let _ = wait_for_suggestion(&mut runtime).await;

        assert_eq!(update_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn completed_suggestion_is_collected() {
        let (mut runtime, _) = runtime(false, false);
        runtime.request_suggestion(snapshot(), GameTime::from_millis(100));

        let (sent_at, result) = wait_for_suggestion(&mut runtime).await;

        assert_eq!(sent_at, GameTime::from_millis(100));
        assert!(result.is_ok());
        assert!(runtime.is_idle());
    }

    #[tokio::test]
    async fn suggestion_error_is_propagated() {
        let (mut runtime, _) = runtime(false, true);
        runtime.request_suggestion(snapshot(), GameTime::ZERO);

        let (_, result) = wait_for_suggestion(&mut runtime).await;

        assert!(matches!(result, Err(BotError::Exited)));
        assert!(runtime.is_idle());
    }

    #[tokio::test]
    async fn worker_error_is_propagated() {
        let (mut runtime, _) = runtime(true, false);
        runtime.request_suggestion(snapshot(), GameTime::ZERO);

        let (_, result) = wait_for_suggestion(&mut runtime).await;
        assert!(result.is_ok());
        assert!(matches!(
            runtime.drain_worker_errors().as_slice(),
            [BotError::Io]
        ));
    }
}
