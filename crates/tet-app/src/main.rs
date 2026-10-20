#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
#![windows_subsystem = "windows"]

mod bot_worker;
mod display;
mod input;
mod settings;
mod timer;

use std::{path::Path, time::Instant};

use macroquad::prelude::*;
use tet_application::{GameSession, GameTime, Player, session, tick};
use tet_domain::Ruleset;
use tet_infrastructure::{
    Fonts, Frame, GridStyle, Layout, MacroquadCanvas, MacroquadRenderer, PlayerView, SmallRng,
    Theme,
};
use tokio::sync::oneshot;

use crate::bot_worker::{BotHandle, BotRequest, BotResponse, PendingSuggest};
use crate::display::{parse_grid_style, resolve_skin_path};
use crate::input::{ControlKeys, collect_input_state};
use crate::settings::{ControlSettings, Settings};
use crate::timer::Timer;

/// Compose a `Frame` from the current session state. Each player gets a
/// `PlayerView` with a layout positioned side-by-side across the screen.
///
/// Single-player for now: one view, positioned at (16, 16) with the
/// standard guideline layout. Multi-player is a separate concern.
fn assemble_frame(session: &GameSession, base_layout: Layout, human_idx: usize) -> Frame {
    let mut views = Vec::with_capacity(session.players.len());
    // Origin starts AFTER the hold box on the left, so the hold box fits
    // on screen without overlapping the board.
    let (ox, oy) = (16.0 + base_layout.hold_box_width(), 16.0);
    let x_spacing = base_layout.board_w(session.ruleset.num_cols) + 80.0;

    for i in 0..session.players.len() {
        let snapshot = session.snapshot(i);
        let layout = Layout {
            origin: (ox + i as f32 * x_spacing, oy),
            ..base_layout
        };
        let label = if i == human_idx { "human" } else { "bot" };
        let ghost = if matches!(snapshot.phase, tet_application::Phase::Playing) {
            Some(tick::project_ghost_pos(&snapshot.board, snapshot.current))
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

#[macroquad::main("tet-rs")]
async fn main() {
    let _runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
        .expect("build tokio runtime");

    // Load settings
    let settings = Settings::load(Path::new("settings.toml"));
    // Control settings
    let keys = ControlKeys::from_settings(&settings.controls).unwrap_or_else(|e| {
        eprintln!("warning: bad key binding: {e}, using defaults");
        ControlKeys::from_settings(&ControlSettings::default()).unwrap()
    });
    // Timing settings
    let mut timer = Timer::new(&settings.timing);
    // Display settings
    let grid_style = parse_grid_style(&settings.display.grid_style).unwrap_or(GridStyle::Full);
    let base_layout = Layout {
        grid_style,
        ..Layout::default()
    };
    let skin_path = resolve_skin_path(settings.display.skin.as_ref());
    let theme = Theme::dark(settings.display.ghost_alpha);
    let fonts = Fonts::load().expect("Fira Code fonts are embedded in the binary");
    let canvas = MacroquadCanvas::new(theme, fonts, skin_path.as_deref()).await;
    let mut renderer = MacroquadRenderer::new(canvas);

    let ruleset = Ruleset::guideline();
    let mut session = GameSession::new(ruleset);
    let (_, queue_rng) = SmallRng::new(None);
    let (_, attack_rng) = SmallRng::new(None);
    let human_idx =
        session.add_player(Player::new_human(Box::new(queue_rng), Box::new(attack_rng)));
    let bot_handles: Vec<(usize, BotHandle)> = Vec::new();

    let mut pending_suggests: Vec<PendingSuggest> = Vec::new();
    let game_start = Instant::now();
    let mut last_frame_at = Instant::now();

    // Init tracing so we can see what values the renderer is computing.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("tet_render=trace")),
        )
        .with_target(true)
        .init();

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
        let input_state = collect_input_state(&keys);
        let tick = timer.tick(input_state, frame_delta_ms);
        let now = GameTime::from_millis(game_start.elapsed().as_millis() as u64);

        // Apply timer-emitted inputs to the human only.
        for input in tick.inputs {
            session.apply_input(human_idx, input, now);
        }

        // Frame tick: garbage arrival, per-player game state (including
        // gravity timing), line-clear distribution.
        session.step_frame(now);

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
        let frame = assemble_frame(&session, base_layout, human_idx);
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
