//! Headless bot harness.
//!
//! Drives the real `GameSession` + real `BlockfishBot` with no renderer and
//! no frame clock, so bot behaviour can be checked deterministically and
//! quickly instead of by playing the game.
//!
//! Invariants asserted here:
//!
//! * **INV-1** the piece the bot plans to place is the piece that will be in
//!   play after the move: the piece currently in play, or — when the bot asks
//!   for a hold — the piece the hold brings up (the held piece, or the next
//!   queue piece when the hold slot is empty).
//! * **INV-3** when a move is applied, the cells that get locked are the
//!   cells the bot asked for. If the requested placement is not reachable
//!   (it overlaps the stack), the host may fall back to an SRS kick — that
//!   case is counted, not asserted, and is fixed by the input-based
//!   suggestion path.
//!
//! The per-piece `println!` trace and the summary counters are for humans;
//! pass/fail comes from the assertions at the end of the run.

use std::collections::HashSet;

use tet_application::{BotMove, BotTransport, GameSession, Piece, Player, session, tick};
use tet_domain::{MinoType, Ruleset, Vec2};
use tet_infrastructure::SmallRng;
use tet_infrastructure::bot::blockfish::BlockfishBot;

/// Search limit for the fast test. The default (50 000 nodes) costs ~1.6 s
/// per suggestion in a debug build, which would make this test unusable.
const FAST_SEARCH_LIMIT: usize = 5_000;

/// Number of pieces played by the fast test.
const FAST_PIECES: usize = 10;

/// Number of pieces played by the long test.
const LONG_PIECES: usize = 200;

/// Counters reported at the end of a run.
#[derive(Debug, Default)]
struct Stats {
    /// Turns where the bot planned for the piece that will be in play.
    planned_for_in_play: usize,
    /// Turns where the bot asked to hold.
    hold_moves: usize,
    /// Turns where the host applied the bot's move.
    applied: usize,
    /// Turns where the host rejected the bot's move.
    rejected: usize,
    /// Turns where the requested placement overlapped the stack, so the host
    /// had to fall back to an SRS kick.
    unreachable_requests: usize,
}

/// The four cells the bot asked for, in tet-rs board coordinates.
///
/// Mirrors the conversion `session::apply_bot_move` performs on the bot's
/// TBP-style centre position.
fn requested_cells(mv: BotMove) -> HashSet<(i8, i8)> {
    let center = MinoType::rotation_center_offset(mv.location.kind, mv.location.orientation);
    let pos = Vec2::new(mv.location.x - center.x, mv.location.y - center.y);
    let piece = Piece {
        kind: mv.location.kind,
        orientation: mv.location.orientation,
        pos,
    };
    piece.cells().into_iter().map(|c| (c.x, c.y)).collect()
}

/// Cells of a piece, as a set so ordering does not matter.
fn cell_set(piece: Piece) -> HashSet<(i8, i8)> {
    piece.cells().into_iter().map(|c| (c.x, c.y)).collect()
}

/// Play `piece_count` pieces with the given bot configuration.
async fn run(piece_count: usize, config: blockfish::Config) -> Stats {
    let mut session = GameSession::new(Ruleset::guideline());
    let (_, queue_rng) = SmallRng::new(Some(42));
    let (_, attack_rng) = SmallRng::new(Some(43));
    let idx = session.add_player(Player::new_bot(Box::new(queue_rng), Box::new(attack_rng)));
    let mut bot = BlockfishBot::with_config(config);
    let mut stats = Stats::default();
    let mut violations: Vec<String> = Vec::new();
    let mut failing_turns: Vec<usize> = Vec::new();

    for piece_no in 0..piece_count {
        let snap = session.snapshot(idx);
        bot.update(&snap).await.expect("bot update failed");
        let moves = bot.suggest().await.expect("bot suggest failed");
        let Some(mv) = moves.first().copied() else {
            panic!("piece {piece_no}: bot returned no moves");
        };

        // INV-1: the bot must plan for the piece that will be in play.
        //
        // Without a hold that is the piece in play now. With a hold it is
        // the piece the hold brings up: the held piece, or — when the hold
        // slot is empty — the next piece in the queue.
        let expected_kind = if mv.hold {
            stats.hold_moves += 1;
            snap.hold.unwrap_or_else(|| snap.queue[0])
        } else {
            snap.current.kind
        };
        if mv.location.kind == expected_kind {
            stats.planned_for_in_play += 1;
        } else {
            failing_turns.push(piece_no);
            violations.push(format!(
                "piece {piece_no}: bot planned for {:?} but {:?} will be in play \
                 (in play: {:?}, hold: {:?}, queue: {:?})",
                mv.location.kind, expected_kind, snap.current.kind, snap.hold, snap.queue,
            ));
        }

        let requested = requested_cells(mv);
        let requested_cells: Vec<Vec2> = requested.iter().map(|&(x, y)| Vec2::new(x, y)).collect();
        let requested_legal = !session.players[idx].board.collides(&requested_cells);

        let applied = session::apply_bot_move(&mut session.players[idx], mv, &session.ruleset);
        if let Some(piece) = applied {
            if requested_legal {
                // INV-3: what gets locked is exactly what was asked for.
                if cell_set(piece) != requested {
                    failing_turns.push(piece_no);
                    violations.push(format!(
                        "piece {piece_no}: host locked {:?} but the bot asked for {:?}",
                        piece.cells(),
                        requested_cells,
                    ));
                }
            } else {
                // The requested placement overlaps the stack; the host fell
                // back to an SRS kick. Only require a legal placement.
                assert!(
                    !session.players[idx].board.collides(&piece.cells()),
                    "piece {piece_no}: host applied an illegal placement",
                );
                stats.unreachable_requests += 1;
            }
            stats.applied += 1;
        } else {
            stats.rejected += 1;
            // A rejected move leaves the piece where it is; in the game it
            // then falls under gravity.
            let ghost =
                tick::project_ghost_pos(&session.players[idx].board, session.players[idx].current);
            session.players[idx].current.pos = ghost;
        }

        let result = tick::try_lock(&mut session.players[idx]);
        tick::post_lock(&mut session.players[idx], &result, &session.ruleset);

        println!(
            "piece {piece_no}: in_play={} planned={} hold={} -> {}",
            snap.current.kind.as_char(),
            mv.location.kind.as_char(),
            mv.hold,
            if applied.is_some() {
                "applied"
            } else {
                "rejected"
            },
        );
    }

    failing_turns.sort_unstable();
    failing_turns.dedup();

    println!(
        "summary: pieces={piece_count} planned_for_in_play={} hold_moves={} applied={} \
         rejected={} unreachable_requests={} violations={} failing_turns={}",
        stats.planned_for_in_play,
        stats.hold_moves,
        stats.applied,
        stats.rejected,
        stats.unreachable_requests,
        violations.len(),
        failing_turns.len(),
    );

    assert!(
        violations.is_empty(),
        "bot broke {} invariant(s) on {} of {piece_count} pieces:\n{}",
        violations.len(),
        failing_turns.len(),
        violations.join("\n"),
    );

    stats
}

/// Fast test: a handful of pieces with a reduced search limit.
#[tokio::test]
async fn bot_plays_the_piece_that_is_in_play() {
    let config = blockfish::Config {
        search_limit: FAST_SEARCH_LIMIT,
        ..blockfish::Config::default()
    };
    run(FAST_PIECES, config).await;
}

/// Long test: 200 pieces with the default configuration.
///
/// Ignored by default — it takes ~20 s per 20 pieces in a debug build. Run it
/// with:
///
/// ```text
/// cargo test --release -p tet-infrastructure --test blockfish_bot -- --ignored -- --nocapture
/// ```
#[tokio::test]
#[ignore = "long run; use --release"]
async fn bot_plays_200_pieces() {
    run(LONG_PIECES, blockfish::Config::default()).await;
}
