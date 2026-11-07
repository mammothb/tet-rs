#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
#![windows_subsystem = "windows"]

mod bot;
mod display;
mod input;
mod settings;
mod timer;

use std::{path::Path, process, time::Instant};

use crate::bot::{BotRuntime, spawn_bot_worker};
use crate::display::{parse_grid_style, resolve_skin_path};
use crate::input::{ControlKeys, collect_input_state};
use crate::settings::{ControlSettings, Settings};
use crate::timer::Timer;
use macroquad::prelude::*;
use tet_application::{GameSession, GameTime, Player, session, tick};
use tet_domain::Ruleset;
use tet_infrastructure::bot::blockfish::BlockfishBot;
use tet_infrastructure::{
    Fonts, Frame, GridStyle, Layout, MacroquadCanvas, MacroquadRenderer, PlayerView, SmallRng,
    Theme,
};

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
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
        .expect("build tokio runtime");

    // Load settings
    let settings = Settings::load(Path::new("settings.toml")).unwrap_or_else(|error| {
        eprintln!("error loading settings: {error}");
        process::exit(1);
    });
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
    let mut bot_runtimes: Vec<BotRuntime> = Vec::new();

    let bf_bot = Box::new(BlockfishBot::new());
    let bf_bot_handle = spawn_bot_worker(runtime.handle(), bf_bot);
    let (_, bf_bot_queue_rng) = SmallRng::new(None);
    let (_, bf_bot_attack_rng) = SmallRng::new(None);
    let bf_bot_idx = session.add_player(Player::new_bot(
        Box::new(bf_bot_queue_rng),
        Box::new(bf_bot_attack_rng),
    ));
    bot_runtimes.push(BotRuntime::new(bf_bot_idx, bf_bot_handle));
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
        for runtime in &mut bot_runtimes {
            let session_idx = runtime.player_idx();
            for error in runtime.drain_worker_errors() {
                tracing::warn!("bot {session_idx} error: {error}");
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

        // For each bot: start preparing the next move when idle (non-blocking)
        for runtime in &mut bot_runtimes {
            if runtime.is_idle() {
                let snap = session.snapshot(runtime.player_idx());
                runtime.request_preparation(snap, now);
            }
        }

        // Commit any prepared bot moves, then start preparing the next move.
        for runtime in &mut bot_runtimes {
            let idx = runtime.player_idx();
            let Some((sent_at, result)) = runtime.poll_calculation() else {
                continue;
            };

            let moves = match result {
                Ok(()) => {
                    let age_ms = now.saturating_millis_since(sent_at);
                    tracing::debug!(
                        target: "bot",
                        player = idx,
                        age_ms,
                        "bot suggestion prepared",
                    );
                    runtime
                        .take_prepared()
                        .expect("successful calculation must produce a prepared move")
                }
                Err(error) => {
                    eprintln!("{error}");
                    continue;
                }
            };

            for mv in moves {
                match session::apply_bot_move(&mut session.players[idx], mv) {
                    Some(piece) => {
                        tracing::debug!(
                            target: "bot",
                            player = idx,
                            kind = %piece.kind.as_char(),
                            orientation = ?piece.orientation,
                            pos = ?piece.pos,
                            "bot move applied",
                        );
                        // Lock through the session so a line clear attacks the
                        // opponents.
                        session.lock_piece(idx);
                        break;
                    }
                    None => {
                        tracing::debug!(
                            target: "bot",
                            player = idx,
                            kind = %mv.location.kind.as_char(),
                            orientation = ?mv.location.orientation,
                            "bot move rejected: placement collides",
                        );
                    }
                }
            }

            let snap = session.snapshot(idx);
            runtime.request_preparation(snap, now);
        }

        // Render via the port.
        let frame = assemble_frame(&session, base_layout, human_idx);
        renderer.render(&frame);

        next_frame().await;

        // Game-over cleanup
        if session.is_finished() {
            for runtime in &mut bot_runtimes {
                runtime.stop();
            }
            break;
        }
    }
}
