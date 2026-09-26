//! Integration tests for `BotSubprocess`.
//!
//! Spawns the `echo_bot` binary (defined in `tests/fixtures/echo_bot.rs` and
//! registered as a `[[bin]]` in `Cargo.toml`) and exercises the real TBP
//! handshake + round-trip through actual subprocess I/O.
//!
//! These tests are the only way to verify `spawn`, `send`, `recv`, and the
//! `BotTransport` impl without trait-mocking the child process.

use std::path::Path;

use tet_application::ports::bot::BotTransport;
use tet_application::{Piece, PlayerSnapshot};
use tet_domain::{Board, Cell, MinoType, Vec2};
use tet_infrastructure::BotSubprocess;

/// Locate the `echo_bot` binary via cargo's runtime env var.
fn echo_bot_path() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_echo_bot"))
}

/// Empty snapshot — board and queue empty, no hold, no combo.
fn empty_snapshot() -> PlayerSnapshot {
    PlayerSnapshot {
        board: Board::new(10, 25),
        queue: vec![],
        hold: None,
        combo: 0,
        b2b: false,
        current: Piece::spawn(MinoType::T),
        phase: tet_application::Phase::Playing,
        pending_garbage: vec![],
    }
}

#[tokio::test]
async fn spawn_completes_handshake_and_captures_info() {
    let mut bot = BotSubprocess::spawn(echo_bot_path())
        .await
        .expect("spawn should succeed with a well-behaved echo bot");

    assert_eq!(bot.info.name, "echo_bot");
    assert_eq!(bot.info.version, "0.0.0");
    assert_eq!(bot.info.author, "tet-rs tests");

    // Clean shutdown so we don't leave a zombie during the test run.
    bot.stop().await;
}

#[tokio::test]
async fn start_is_infallible_after_spawn() {
    let mut bot = BotSubprocess::spawn(echo_bot_path())
        .await
        .expect("spawn should succeed");
    assert!(bot.start(&tet_domain::Ruleset::guideline()).is_ok());
    bot.stop().await;
}

#[tokio::test]
async fn update_sends_start_message_without_error() {
    let mut bot = BotSubprocess::spawn(echo_bot_path())
        .await
        .expect("spawn should succeed");
    // echo_bot ignores `start` messages; we just verify our send path works.
    bot.update(&empty_snapshot())
        .await
        .expect("update should succeed");
    bot.stop().await;
}

#[tokio::test]
async fn suggest_round_trips_a_move_through_the_subprocess() {
    let mut bot = BotSubprocess::spawn(echo_bot_path())
        .await
        .expect("spawn should succeed");

    let moves = bot.suggest().await.expect("suggest should succeed");
    assert_eq!(moves.len(), 1, "echo_bot replies with exactly one move");
    // echo_bot sends TBP-center (4, 19). For T-piece North, rotation_center_offset
    // is (1, 1), so the bbox anchor is (3, 18). That's what our domain uses.
    assert_eq!(moves[0].location.kind, MinoType::T);
    assert_eq!(
        moves[0].location.orientation,
        tet_domain::Orientation::North
    );
    assert_eq!(moves[0].location.x, 3);
    assert_eq!(moves[0].location.y, 18);
    assert!(matches!(
        moves[0].spin,
        tet_application::ports::bot::BotSpin::None
    ));

    bot.stop().await;
}

#[tokio::test]
async fn multiple_suggests_each_get_a_reply() {
    // Catches a class of bugs where `recv` consumes one too many or too few
    // lines, causing subsequent reads to desync.
    let mut bot = BotSubprocess::spawn(echo_bot_path())
        .await
        .expect("spawn should succeed");

    for i in 0..5 {
        let moves = bot
            .suggest()
            .await
            .unwrap_or_else(|e| panic!("suggest #{i} should succeed: {e:?}"));
        assert_eq!(moves.len(), 1, "suggest #{i} should return one move");
    }

    bot.stop().await;
}

#[tokio::test]
async fn recv_after_kill_returns_exited_error() {
    // Kill the bot, then verify the next `recv` returns `BotError::Exited`
    // (zero-byte read on EOF).
    let mut bot = BotSubprocess::spawn(echo_bot_path())
        .await
        .expect("spawn should succeed");

    // Kill the process and wait for it to exit (closing stdout in the process).
    bot.stop().await;

    // Give the kernel a moment to actually close the pipes.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let result = bot.suggest().await;
    assert!(
        matches!(result, Err(tet_application::BotError::Exited)),
        "expected BotError::Exited after kill, got {result:?}"
    );
}

#[tokio::test]
async fn spawn_fails_when_binary_does_not_exist() {
    let result = BotSubprocess::spawn(Path::new("/nonexistent/path/to/bot")).await;
    assert!(
        matches!(result, Err(tet_application::BotError::Io)),
        "expected BotError::Io for missing binary, got {result:?}"
    );
}

/// Extra coverage: a populated snapshot (Block cell on the board, queue, hold)
/// flows through `update` and the echo bot accepts it.
#[tokio::test]
async fn update_with_populated_snapshot() {
    let mut bot = BotSubprocess::spawn(echo_bot_path())
        .await
        .expect("spawn should succeed");

    let mut snap = empty_snapshot();
    snap.board.set(Vec2::new(0, 0), Cell::Block(MinoType::T));
    snap.queue = vec![MinoType::I, MinoType::O];
    snap.hold = Some(MinoType::S);
    snap.combo = 3;
    snap.b2b = true;

    bot.update(&snap).await.expect("update should succeed");
    // echo_bot ignored the start, but our send path succeeded.
    bot.stop().await;
}
