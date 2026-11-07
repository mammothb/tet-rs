//! Bot worker plumbing.
//!
//! Runs each in-process bot on a Tokio task and translates between the
//! session's high-level commands (`Update`, `Suggest`, `Stop`) and the bot's
//! `BotTransport` API. The main thread communicates via channels and never
//! blocks on bot computation.

use std::sync::{Mutex, mpsc};

use tet_application::{BotError, BotMove, BotTransport, PlayerSnapshot};
use tokio::sync::{mpsc as tmpsc, oneshot};

/// Handle to one bot worker.
pub(crate) struct BotHandle {
    /// Sending a request is non-blocking (channel is unbounded).
    req_tx: tmpsc::UnboundedSender<BotRequest>,
    /// Receiving a response is sync — the main thread polls each frame.
    resp_rx: Mutex<mpsc::Receiver<BotResponse>>,
}

/// Requests sent from the main thread to a bot's worker.
enum BotRequest {
    Update(PlayerSnapshot),
    Suggest(oneshot::Sender<Result<Vec<BotMove>, BotError>>),
    Stop,
}

/// Responses from the bot's worker (Suggest replies go via `oneshot`, not here).
enum BotResponse {
    Ack,
    Error(BotError),
}

impl BotHandle {
    /// Request one suggestion for `snapshot`.
    pub(crate) fn request_suggestion(
        &self,
        snapshot: PlayerSnapshot,
    ) -> oneshot::Receiver<Result<Vec<BotMove>, BotError>> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.req_tx.send(BotRequest::Update(snapshot)).ok();
        self.req_tx.send(BotRequest::Suggest(reply_tx)).ok();
        reply_rx
    }

    /// Drain worker acknowledgements and return any reported errors.
    pub(crate) fn drain_errors(&self) -> Vec<BotError> {
        let mut errors = Vec::new();
        let receiver = self.resp_rx.lock().unwrap();
        while let Ok(response) = receiver.try_recv() {
            if let BotResponse::Error(error) = response {
                errors.push(error);
            }
        }
        errors
    }

    /// Tell the worker to stop.
    pub(crate) fn stop(&self) {
        self.req_tx.send(BotRequest::Stop).ok();
    }
}

/// Spawn one worker task for an in-process bot.
///
/// The bot is moved into the spawned task, which must own its data for the
/// task's `'static` lifetime.
pub(crate) fn spawn_bot_worker(
    runtime: &tokio::runtime::Handle,
    bot: Box<dyn BotTransport + Send>,
) -> BotHandle {
    // Request channel: worker awaits, main sends (non-blocking).
    // `tokio::sync::mpsc` because the worker's `recv()` is async.
    let (req_tx, mut req_rx) = tmpsc::unbounded_channel::<BotRequest>();
    // Response channel: main polls `try_recv()` (sync), worker sends
    // (non-blocking). `std::sync::mpsc` because the main thread is sync.
    let (resp_tx, resp_rx) = mpsc::channel::<BotResponse>();
    let resp_rx = Mutex::new(resp_rx);

    runtime.spawn(async move {
        let mut bot = bot;
        while let Some(req) = req_rx.recv().await {
            match req {
                BotRequest::Update(snap) => match bot.update(&snap).await {
                    Ok(()) => {
                        let _ = resp_tx.send(BotResponse::Ack);
                    }
                    Err(e) => {
                        let _ = resp_tx.send(BotResponse::Error(e));
                    }
                },
                BotRequest::Suggest(reply) => {
                    // Suggest result flows through the oneshot so the caller
                    // can match the reply to the request. Ack / errors for
                    // non-suggest operations come through `BotResponse`.
                    let moves = bot.suggest().await;
                    let _ = reply.send(moves);
                }
                BotRequest::Stop => {
                    bot.stop().await;
                    return Ok::<_, BotError>(());
                }
            }
        }
        Ok(())
    });

    BotHandle { req_tx, resp_rx }
}
