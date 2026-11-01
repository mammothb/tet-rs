#![allow(clippy::cast_possible_truncation)]

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
//! * **INV-2** the placement is a legal resting position on the host board:
//!   its cells do not overlap the stack and the piece cannot fall another row.
//! * **INV-3** the cells the host locks are the cells *blockfish* placed — not
//!   the cells the host understood the bot to mean. The ground truth is
//!   recomputed here with an independent snapshot build and search, so this
//!   covers the whole adapter pipeline: snapshot construction, the search,
//!   the placement reconstruction and the coordinate conversion.
//! * **INV-4** the board the bot planned on is the board the host has: the
//!   host board must not change between the snapshot and the apply. Nothing
//!   may land under the bot while it thinks.
//!
//! The per-piece `println!` trace and the summary counters are for humans;
//! pass/fail comes from the assertions at the end of the run.

use std::collections::HashSet;

use tet_application::{
    BotTransport, GameSession, GameTime, Input, Phase, Player, PlayerSnapshot, session,
};
use tet_domain::{Cell, Ruleset, Vec2};
use tet_infrastructure::SmallRng;
use tet_infrastructure::bot::blockfish::BlockfishBot;

/// Search limit for the fast test. The default (50 000 nodes) costs ~1.6 s
/// per suggestion in a debug build, which would make this test unusable.
const FAST_SEARCH_LIMIT: usize = 5_000;

/// Number of pieces played by the fast test.
const FAST_PIECES: usize = 10;

/// Number of pieces played by the long test.
const LONG_PIECES: usize = 200;

/// Padding for the scratch matrix used to read a placement's cells back.
const PAD: i16 = 5;

/// Scratch matrix width. Must be wide enough for the widest shape (4) plus
/// the padding on both sides.
const SCRATCH_COLS: u16 = 15;

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
    /// Turns where the bot's placement overlapped the stack.
    illegal_placements: usize,
}

/// Hash of a board's contents, to detect any change to it.
fn board_hash(board: &tet_domain::Board) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for row in board.rows() {
        for cell in row {
            let value = match cell {
                Cell::Empty => 0u8,
                Cell::Garbage => 1,
                Cell::Block(kind) => 2 + kind.as_char() as u8,
            };
            hash ^= u64::from(value);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

/// Height of the tallest occupied column.
fn stack_height(board: &tet_domain::Board) -> usize {
    board
        .rows()
        .enumerate()
        .filter(|(_, row)| row.iter().any(|c| *c != Cell::Empty))
        .map(|(y, _)| y + 1)
        .max()
        .unwrap_or(0)
}

/// The trivial baseline: hard-drop every piece in its spawn column.
///
/// It survives only as long as that column can hold pieces, which is the
/// yardstick the bot has to beat.
fn baseline_pieces_survived() -> usize {
    let mut session = GameSession::new(Ruleset::guideline());
    let (_, queue_rng) = SmallRng::new(Some(42));
    let (_, attack_rng) = SmallRng::new(Some(43));
    let idx = session.add_player(Player::new_bot(Box::new(queue_rng), Box::new(attack_rng)));

    for piece_no in 0..LONG_PIECES {
        session.apply_input(idx, Input::HardDrop, GameTime::ZERO);
        if session.players[idx].phase == Phase::GameOver {
            return piece_no;
        }
    }
    LONG_PIECES
}

/// Blockfish's snapshot for a player state, rebuilt independently of the
/// adapter.
///
/// The queue rule is the adapter's: blockfish's `queue[0]` is the piece being
/// placed *now*, so the piece in play leads.
fn blockfish_snapshot(snap: &PlayerSnapshot) -> blockfish::Snapshot {
    let mut queue = vec![snap.current.kind];
    queue.extend_from_slice(&snap.queue);
    let mut matrix = blockfish::BasicMatrix::with_cols(snap.board.num_cols().into());
    for (i, row) in snap.board.rows().enumerate() {
        for (j, &cell) in row.iter().enumerate() {
            if cell != Cell::Empty {
                matrix.set((i as u16, j as u16));
            }
        }
    }
    blockfish::Snapshot {
        hold: snap
            .hold
            .map(|k| blockfish::Color::try_from(k.as_char()).unwrap()),
        queue: queue
            .iter()
            .map(|k| blockfish::Color::try_from(k.as_char()).unwrap())
            .collect(),
        matrix,
    }
}

/// The cells blockfish placed, in tet-rs board coordinates.
fn blockfish_cells(
    shtb: &blockfish::ShapeTable,
    color: blockfish::Color,
    tf: blockfish::Transform,
) -> HashSet<(i8, i8)> {
    let shape = shtb.shape(color).expect("colour has a shape");
    let mut mat = blockfish::BasicMatrix::with_cols(SCRATCH_COLS);
    shape.blit_to(&mut mat, (tf.0 + PAD, tf.1 + PAD, tf.2));
    let mut cells = HashSet::new();
    for i in 0..mat.rows() {
        for j in 0..mat.cols() {
            if mat.get((i, j)) {
                cells.insert((j as i8 - PAD as i8, i as i8 - PAD as i8));
            }
        }
    }
    cells
}

/// Cells of a piece, as a set so ordering does not matter.
fn cell_set(piece: tet_application::Piece) -> HashSet<(i8, i8)> {
    piece.cells().into_iter().map(|c| (c.x, c.y)).collect()
}

/// Play `piece_count` pieces with the given bot configuration.
async fn run(piece_count: usize, config: blockfish::Config) -> Stats {
    let mut session = GameSession::new(Ruleset::guideline());
    let (_, queue_rng) = SmallRng::new(Some(42));
    let (_, attack_rng) = SmallRng::new(Some(43));
    let idx = session.add_player(Player::new_bot(Box::new(queue_rng), Box::new(attack_rng)));
    let mut bot = BlockfishBot::with_config(config.clone());
    let shtb = blockfish::srs();
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

        // Ground truth: blockfish's own placement for this state.
        let truth = {
            let bf_state = blockfish::State::from(blockfish_snapshot(&snap));
            let trace = blockfish::ai::analyze(&shtb, &config, bf_state.clone())
                .expect("ground-truth search found a move")
                .1;
            let (color, tf, did_hold) =
                blockfish::ai::reconstruct_first_placement(&shtb, bf_state, &trace)
                    .expect("ground-truth placement");
            (color, tf, did_hold)
        };
        let (truth_color, truth_tf, truth_hold) = truth;
        let truth_cells = blockfish_cells(&shtb, truth_color, truth_tf);
        if truth_hold != mv.hold {
            failing_turns.push(piece_no);
            violations.push(format!(
                "piece {piece_no}: bot says hold={} but blockfish says hold={}",
                mv.hold, truth_hold,
            ));
        }

        let truth_cells_vec: Vec<Vec2> =
            truth_cells.iter().map(|&(x, y)| Vec2::new(x, y)).collect();

        // INV-2: the placement must be a legal resting position — it does
        // not overlap the stack, and the piece cannot fall another row.
        if session.players[idx].board.collides(&truth_cells_vec) {
            stats.illegal_placements += 1;
            failing_turns.push(piece_no);
            violations.push(format!(
                "piece {piece_no}: bot's placement {truth_cells_vec:?} overlaps the stack",
            ));
        } else {
            let one_row_down: Vec<Vec2> = truth_cells_vec
                .iter()
                .map(|c| Vec2::new(c.x, c.y - 1))
                .collect();
            if !session.players[idx].board.collides(&one_row_down) {
                failing_turns.push(piece_no);
                violations.push(format!(
                    "piece {piece_no}: bot's placement {truth_cells_vec:?} is not resting",
                ));
            }
        }

        // INV-4: the board the bot planned on is the board the host has.
        // Nothing may land under the bot while it thinks.
        let applied = session::apply_bot_move(&mut session.players[idx], mv);
        if board_hash(&session.players[idx].board) != board_hash(&snap.board) {
            failing_turns.push(piece_no);
            violations.push(format!(
                "piece {piece_no}: the board changed while the bot was thinking",
            ));
        }
        if let Some(piece) = applied {
            // INV-3: the locked cells are the cells blockfish placed.
            if cell_set(piece) != truth_cells {
                failing_turns.push(piece_no);
                violations.push(format!(
                    "piece {piece_no}: host locked {:?} but blockfish placed {:?}",
                    piece.cells(),
                    truth_cells_vec,
                ));
            }
            stats.applied += 1;
        } else {
            stats.rejected += 1;
            failing_turns.push(piece_no);
            violations.push(format!(
                "piece {piece_no}: host rejected the bot's placement {truth_cells_vec:?}",
            ));
            // A rejection means the bot's placement is not legal on the host
            // board — a bug, not a normal outcome. Stop here rather than
            // replaying the same rejected move forever.
            break;
        }

        session.lock_piece(idx);

        // The next piece spawns and must stay put: a bot's piece does not
        // fall, so the board the bot plans on is the board it applies to.
        let spawn_pos = session.players[idx].current.pos;
        session.step_frame(GameTime::from_millis(piece_no as u64 * 16));
        assert_eq!(
            session.players[idx].current.pos, spawn_pos,
            "piece {piece_no}: the bot's piece fell",
        );

        println!(
            "piece {piece_no}: in_play={} planned={} hold={} -> applied",
            snap.current.kind.as_char(),
            mv.location.kind.as_char(),
            mv.hold,
        );
    }

    failing_turns.sort_unstable();
    failing_turns.dedup();

    // Sanity: the bot must beat the trivial baseline — dropping every piece
    // in the spawn column, which top-outs long before the bot does.
    let baseline = baseline_pieces_survived();
    let height = stack_height(&session.players[idx].board);
    println!(
        "summary: pieces={piece_count} planned_for_in_play={} hold_moves={} applied={} \
         rejected={} illegal_placements={} violations={} failing_turns={} \
         stack_height={height} baseline_survived={baseline}",
        stats.planned_for_in_play,
        stats.hold_moves,
        stats.applied,
        stats.rejected,
        stats.illegal_placements,
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

    assert!(
        stats.applied >= baseline,
        "bot placed {} pieces but the trivial baseline only survives {}",
        stats.applied,
        baseline,
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
