use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};

use macroquad::prelude::*;
use tet_application::{
    BotError, BotMove, BotTransport, GameSession, Input, Player, PlayerSnapshot, session,
};
use tet_domain::{Cell, MinoType, Ruleset};
use tet_infrastructure::{BotSubprocess, SmallRng};
use tokio::sync::{mpsc as tmpsc, oneshot};

#[derive(Clone)]
struct BotHandle {
    // Sending a request is non-blocking (channel is unbounded).
    req_tx: tmpsc::UnboundedSender<BotRequest>,
    // Receiving a response IS blocking — polled each frame.
    resp_rx: Arc<Mutex<mpsc::Receiver<BotResponse>>>,
}

#[allow(dead_code)]
enum BotRequest {
    Update(PlayerSnapshot),
    Suggest(oneshot::Sender<Result<Vec<BotMove>, BotError>>),
    Stop,
}

#[allow(dead_code)]
enum BotResponse {
    Ack,
    Error(BotError),
}

/// In-flight `Suggest` reply. The receiver stays in our pending list until
/// the worker actually sends through it (or we drop the slot on cleanup).
type PendingSuggest = (usize, oneshot::Receiver<Result<Vec<BotMove>, BotError>>);

/// Spawn one worker task per bot. Hoisted to module level so the
/// items-after-statements lint is happy (no items between let bindings).
///
/// `bot_path: PathBuf` (owned) because the spawned async task is `'static`
/// — it must own all its captured data, no borrows from the caller.
#[allow(dead_code)]
fn spawn_bot_worker(runtime: &tokio::runtime::Handle, bot_path: PathBuf) -> BotHandle {
    // Request channel: worker awaits, main sends (non-blocking).
    // `tokio::sync::mpsc` because the worker's `recv()` is async.
    let (req_tx, mut req_rx) = tmpsc::unbounded_channel::<BotRequest>();
    // Response channel: main polls `try_recv()` (sync), worker sends
    // (non-blocking). `std::sync::mpsc` because the main thread is sync.
    // Receiver is wrapped in `Arc<Mutex<_>>` so `BotHandle: Clone`.
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
                    // Send the move result directly through the oneshot;
                    // ACK / errors flow through the Result, not a separate
                    // response variant.
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

fn piece_color(t: MinoType) -> Color {
    match t {
        MinoType::I => Color::new(0.0, 0.85, 0.85, 1.0), // cyan
        MinoType::O => Color::new(0.95, 0.85, 0.0, 1.0), // yellow
        MinoType::T => Color::new(0.55, 0.0, 0.55, 1.0), // purple
        MinoType::S => Color::new(0.0, 0.85, 0.0, 1.0),  // green
        MinoType::Z => Color::new(0.85, 0.0, 0.0, 1.0),  // red
        MinoType::J => Color::new(0.0, 0.0, 0.85, 1.0),  // blue
        MinoType::L => Color::new(0.95, 0.55, 0.0, 1.0), // orange
    }
}

fn handle_input() -> Input {
    if is_key_down(KeyCode::Left) {
        Input::MoveLeft
    } else if is_key_down(KeyCode::Right) {
        Input::MoveRight
    } else if is_key_down(KeyCode::Down) {
        Input::SoftDrop
    } else if is_key_pressed(KeyCode::Space) {
        Input::HardDrop
    } else if is_key_down(KeyCode::F) {
        Input::RotateCW
    } else if is_key_down(KeyCode::D) {
        Input::RotateCCW
    } else if is_key_pressed(KeyCode::S) {
        Input::Hold
    } else {
        Input::None
    }
}

#[macroquad::main("tet-rs")]
async fn main() {
    let _runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
        .expect("build tokio runtime");

    let ruleset = Ruleset::guideline();
    let mut session = GameSession::<SmallRng>::new(ruleset);
    let (_, queue_rng) = SmallRng::new(None);
    let (_, attack_rng) = SmallRng::new(None);
    let human_idx = session.add_player(Player::new_human(queue_rng, attack_rng));
    let bot_handles: Vec<(usize, BotHandle)> = Vec::new();

    let mut pending_suggests: Vec<PendingSuggest> = Vec::new();

    loop {
        // Drain any pending bot responses (non-blocking)
        for (session_idx, handle) in &bot_handles {
            while let Ok(resp) = handle.resp_rx.lock().unwrap().try_recv() {
                match resp {
                    BotResponse::Ack => {}
                    BotResponse::Error(e) => tracing::warn!("bot {session_idx} error: {e}"),
                }
            }
        }

        // Collect human input (no InputSource trait yet)
        let input = handle_input();
        session.apply_input(human_idx, input);

        // Step the game state
        session.step_frame();

        // For each bot: send update + suggest (non-blocking)
        for (session_idx, handle) in &bot_handles {
            let snap = session.snapshot(*session_idx);
            handle.req_tx.send(BotRequest::Update(snap)).ok();
            let (reply_tx, reply_rx) = oneshot::channel();
            handle.req_tx.send(BotRequest::Suggest(reply_tx)).ok();
            pending_suggests.push((*session_idx, reply_rx));
        }

        // Apply any completed bot moves
        let completed: Vec<_> = pending_suggests
            .drain(..)
            .filter_map(|(idx, mut rx)| match rx.try_recv() {
                Ok(Ok(moves)) => Some((idx, moves)),
                _ => None, // not ready yet, or bot errored
            })
            .collect();
        for (idx, moves) in completed {
            for mv in moves {
                if session::apply_bot_move(&mut session.players[idx], mv, &session.ruleset)
                    .is_some()
                {
                    break;
                }
            }
        }

        // Render (no Renderer trait yet)
        clear_background(BLACK);
        let cell_px = 24.0;
        let board_w = cell_px * 10.0;
        let board_h = cell_px * 25.0;
        #[allow(clippy::cast_precision_loss)]
        for (idx, player) in session.players.iter().enumerate() {
            let x0 = idx as f32 * (board_w + 16.0);
            draw_rectangle_lines(x0, 0.0, board_w, board_h, 1.0, GRAY);
            for (y, row) in player.board.rows().enumerate() {
                for (x, cell) in row.iter().enumerate() {
                    let screen_y = board_h - (y as f32 + 1.0) * cell_px;
                    let screen_x = x0 + x as f32 * cell_px;
                    let color = match cell {
                        Cell::Empty => continue, // skip drawing
                        Cell::Block(t) => piece_color(*t),
                        Cell::Garbage => GRAY,
                    };
                    draw_rectangle(screen_x, screen_y, cell_px, cell_px, color);
                }
            }
        }
        draw_text(
            format!("frame {}", session.frame),
            10.0,
            board_h + 24.0,
            24.0,
            WHITE,
        );

        next_frame().await;

        // Game-over cleanup
        if session.is_finished() {
            for (_, handle) in &bot_handles {
                let _ = handle.req_tx.send(BotRequest::Stop);
            }
            // Give workers a tick to exit cleanly. The next `next_frame().await`
            // would block forever since the macroquad main loop is still running;
            // for first slice we exit explicitly.
            break;
        }
    }
}
