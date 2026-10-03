mod bot_worker;
mod timer;

use std::time::Instant;

use macroquad::prelude::*;
use tet_application::{
    Frame, GameSession, Layout, Player, PlayerSnapshot, PlayerView, Renderer, session,
};
use tet_domain::{Rng, Ruleset, Vec2};
use tet_infrastructure::{MacroquadRenderer, SmallRng};
use tokio::sync::oneshot;

use bot_worker::{BotHandle, BotRequest, BotResponse, PendingSuggest};
use timer::{InputState, Timer};

/// Read the keyboard state and produce an `InputState` for the timer.
fn collect_input_state() -> InputState {
    InputState {
        move_left_held: is_key_down(KeyCode::Left),
        move_right_held: is_key_down(KeyCode::Right),
        soft_drop_held: is_key_down(KeyCode::Down),
        rotate_cw_pressed: is_key_pressed(KeyCode::F),
        rotate_ccw_pressed: is_key_pressed(KeyCode::D),
        hard_drop_pressed: is_key_pressed(KeyCode::Space),
        hold_pressed: is_key_pressed(KeyCode::S),
    }
}

/// Compose a `Frame` from the current session state. Each player gets a
/// `PlayerView` with a layout positioned side-by-side across the screen.
///
/// Single-player for now: one view, positioned at (16, 16) with the
/// standard guideline layout. Multi-player is a separate concern.
fn assemble_frame<R: Rng>(session: &GameSession<R>, human_idx: usize) -> Frame {
    let mut views = Vec::with_capacity(session.players.len());
    let base_layout = Layout::default();
    // Origin starts AFTER the hold box on the left, so the hold box fits
    // on screen without overlapping the board.
    let (ox, oy) = (16.0 + base_layout.hold_box_width(), 16.0);
    #[allow(clippy::cast_precision_loss)]
    let x_spacing = base_layout.board_w(session.ruleset.num_cols) + 80.0;

    for i in 0..session.players.len() {
        let snapshot = session.snapshot(i);
        let layout = Layout {
            #[allow(clippy::cast_precision_loss)]
            origin: (ox + i as f32 * x_spacing, oy),
            ..base_layout
        };
        let label = if i == human_idx { "human" } else { "bot" };
        let ghost = if matches!(snapshot.phase, tet_application::Phase::Playing) {
            Some(ghost_position_from_snapshot(&snapshot))
        } else {
            None
        };

        views.push(PlayerView {
            snapshot,
            label,
            layout,
            ghost,
        });
    }

    Frame {
        ruleset: session.ruleset.clone(),
        views,
    }
}

/// Compute the landing position from a snapshot. Iterates downward until
/// the piece would collide with the board; returns the last valid `y`.
fn ghost_position_from_snapshot(snapshot: &PlayerSnapshot) -> Vec2 {
    let pos = snapshot.current.pos;
    let mut y = pos.y;
    loop {
        let next_y = y - 1;
        let candidate = Vec2::new(pos.x, next_y);
        let cells: Vec<Vec2> = snapshot
            .current
            .cells()
            .map(|c| Vec2::new(c.x, c.y + (next_y - pos.y)))
            .collect();
        if snapshot.board.collides(&cells) {
            return Vec2::new(pos.x, y);
        }
        y = candidate.y;
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
    let mut timer = Timer::new();
    let mut last_frame_at = Instant::now();
    let mut renderer = MacroquadRenderer;

    loop {
        let frame_delta_ms = last_frame_at.elapsed().as_secs_f32() * 1000.0;
        last_frame_at = Instant::now();

        // Drain any pending bot responses (non-blocking)
        for (session_idx, handle) in &bot_handles {
            while let Ok(resp) = handle.resp_rx.lock().unwrap().try_recv() {
                match resp {
                    BotResponse::Ack => {}
                    BotResponse::Error(e) => tracing::warn!("bot {session_idx} error: {e}"),
                }
            }
        }

        // Timer translates keyboard state into inputs.
        let input_state = collect_input_state();
        let tick = timer.tick(input_state, frame_delta_ms);

        // Apply timer-emitted inputs to the human only.
        for input in tick.inputs {
            session.apply_input(human_idx, input);
        }

        // Frame tick: garbage arrival, per-player game state (including
        // gravity timing), line-clear distribution.
        session.step_frame(Instant::now());

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
                _ => None,
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

        // Render via the port.
        let frame = assemble_frame(&session, human_idx);
        renderer.render(&frame);

        next_frame().await;

        // Game-over cleanup
        if session.is_finished() {
            for (_, handle) in &bot_handles {
                let _ = handle.req_tx.send(BotRequest::Stop);
            }
            break;
        }
    }
}
