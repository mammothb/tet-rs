//! Bot worker plumbing.
//!
//! Spawns one bot subprocess per call and translates between the session's
//! high-level commands (`Update`, `Suggest`, `Stop`) and the bot's TBP
//! protocol. Each worker runs on a tokio task; the main thread communicates
//! via channels and never blocks on bot I/O.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};

use tet_application::{BotError, BotMove, BotTransport, PlayerSnapshot};
use tet_infrastructure::BotSubprocess;
use tokio::sync::{mpsc as tmpsc, oneshot};

/// Send requests to a bot's worker. Cloning is cheap (channel senders clone).
#[derive(Clone)]
#[allow(dead_code)] // Fields used by main.rs; suppress scaffold warnings.
pub struct BotHandle {
    /// Sending a request is non-blocking (channel is unbounded).
    pub req_tx: tmpsc::UnboundedSender<BotRequest>,
    /// Receiving a response is sync — the main thread polls each frame.
    /// Wrapped in `Arc<Mutex<_>>` so `BotHandle: Clone` works.
    pub resp_rx: Arc<Mutex<mpsc::Receiver<BotResponse>>>,
}

/// Requests sent from the main thread to a bot's worker.
#[allow(dead_code)] // Variants used by main.rs; suppress scaffold warnings.
pub enum BotRequest {
    Update(PlayerSnapshot),
    Suggest(oneshot::Sender<Result<Vec<BotMove>, BotError>>),
    Stop,
}

/// Responses from a bot's worker (Suggest replies go via `oneshot`, not here).
#[allow(dead_code)] // Variants used by main.rs; suppress scaffold warnings.
pub enum BotResponse {
    Ack,
    Error(BotError),
}

/// In-flight `Suggest` reply. The receiver stays in the main thread's pending
/// list until the worker sends through it (or we drop the slot on cleanup).
pub type PendingSuggest = (usize, oneshot::Receiver<Result<Vec<BotMove>, BotError>>);

/// Spawn one worker task per bot.
///
/// `bot_path: PathBuf` (owned) because the spawned async task is `'static`
/// — it must own all its captured data, no borrows from the caller.
#[allow(dead_code, unused_variables)] // Scaffold: wired by main.rs when bots are enabled.
pub fn spawn_bot_worker(runtime: &tokio::runtime::Handle, bot_path: PathBuf) -> BotHandle {
    // Request channel: worker awaits, main sends (non-blocking).
    // `tokio::sync::mpsc` because the worker's `recv()` is async.
    let (req_tx, mut req_rx) = tmpsc::unbounded_channel::<BotRequest>();
    // Response channel: main polls `try_recv()` (sync), worker sends
    // (non-blocking). `std::sync::mpsc` because the main thread is sync.
    let (resp_tx, resp_rx) = mpsc::channel::<BotResponse>();
    let resp_rx = Arc::new(Mutex::new(resp_rx));

    runtime.spawn(async move {
        let mut bot = BotSubprocess::spawn(&bot_path).await?;
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
