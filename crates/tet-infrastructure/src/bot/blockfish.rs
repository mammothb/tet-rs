#![allow(clippy::cast_possible_truncation)]

use std::iter;

use async_trait::async_trait;
use tet_application::{BotError, BotMove, BotPieceLocation, BotSpin, BotTransport, PlayerSnapshot};
use tet_domain::{Board, Cell, MinoType, Orientation, Ruleset};

pub struct BlockfishBot {
    config: blockfish::Config,
    shape_table: blockfish::ShapeTable,
    snapshot: Option<blockfish::Snapshot>,
}

impl BlockfishBot {
    /// Bot with the default search configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(blockfish::Config::default())
    }

    /// Bot with an explicit search configuration.
    ///
    /// The search cost is dominated by `config.search_limit` (number of B*
    /// nodes). The default (50 000) costs ~87 ms per suggestion in a release
    /// build and ~1.6 s in a debug build, so tests should pass a smaller
    /// limit to stay fast.
    #[must_use]
    pub fn with_config(config: blockfish::Config) -> Self {
        Self {
            config,
            shape_table: blockfish::srs(),
            snapshot: None,
        }
    }
}

impl Default for BlockfishBot {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BotTransport for BlockfishBot {
    fn start(&mut self, _rules: &Ruleset) -> Result<(), BotError> {
        Ok(())
    }

    async fn update(&mut self, snapshot: &PlayerSnapshot) -> Result<(), BotError> {
        self.snapshot = Some(to_bf_snapshot(snapshot));
        Ok(())
    }

    async fn suggest(&mut self) -> Result<Vec<BotMove>, BotError> {
        let Some(snapshot) = self.snapshot.take() else {
            return Ok(vec![]);
        };
        let state = blockfish::State::from(snapshot);

        let Some((_, trace)) =
            blockfish::ai::analyze(&self.shape_table, &self.config, state.clone())
        else {
            return Ok(vec![]);
        };

        let placement =
            blockfish::ai::reconstruct_first_placement(&self.shape_table, state, &trace);
        let Some((color, transform, hold)) = placement else {
            return Ok(vec![]);
        };

        let kind = to_tet_mino_type(color);
        let orientation = to_tet_orientation(transform.2);
        let (x, y) = to_tet_center(kind, orientation, transform);

        Ok(vec![BotMove {
            location: BotPieceLocation {
                kind,
                orientation,
                x,
                y,
            },
            spin: BotSpin::None,
            hold,
        }])
    }

    async fn stop(&mut self) {}
}

fn to_bf_matrix(board: &Board) -> blockfish::BasicMatrix {
    let mut mat = blockfish::BasicMatrix::with_cols(board.num_cols().into());
    for (i, row) in board.rows().enumerate() {
        for (j, &cell) in row.iter().enumerate() {
            if cell != Cell::Empty {
                mat.set((i as u16, j as u16));
            }
        }
    }
    mat
}

fn to_bf_color(kind: MinoType) -> blockfish::Color {
    blockfish::Color::try_from(kind.as_char()).expect("mino always map to alphabet")
}

/// Pack a player snapshot into the format blockfish's search expects.
///
/// **Queue convention.** Blockfish's `Snapshot.queue[0]` is the piece being
/// placed *right now*; `queue[1..]` are the previews. Tet-rs's
/// `PlayerSnapshot.queue` is the preview queue — the piece in play has
/// already been taken out of it — so the piece in play is prepended here.
/// Skipping this shifts the whole plan by one piece.
fn to_bf_snapshot(snapshot: &PlayerSnapshot) -> blockfish::Snapshot {
    let hold = snapshot.hold.map(to_bf_color);
    let queue = iter::once(snapshot.current.kind)
        .chain(snapshot.queue.iter().copied())
        .map(to_bf_color)
        .collect();
    let matrix = to_bf_matrix(&snapshot.board);
    blockfish::Snapshot {
        hold,
        queue,
        matrix,
    }
}

fn to_tet_center(
    kind: MinoType,
    orientation: Orientation,
    transform: blockfish::Transform,
) -> (i8, i8) {
    let offset = MinoType::rotation_center_offset(kind, orientation);
    let x = transform.1 as i8 + offset.x;
    let y = transform.0 as i8 + offset.y;
    (x, y)
}

fn to_tet_mino_type(color: blockfish::Color) -> MinoType {
    MinoType::from_char(color.as_char()).expect("blockfish contains unknown char")
}

fn to_tet_orientation(orientation: blockfish::Orientation) -> Orientation {
    match orientation {
        blockfish::Orientation::R0 => Orientation::North,
        blockfish::Orientation::R1 => Orientation::East,
        blockfish::Orientation::R2 => Orientation::South,
        blockfish::Orientation::R3 => Orientation::West,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use tet_application::{Phase, Piece, PlayerSnapshot};
    use tet_domain::Board;

    fn snapshot(current: MinoType, queue: Vec<MinoType>, hold: Option<MinoType>) -> PlayerSnapshot {
        PlayerSnapshot {
            board: Board::new(10, 25),
            queue,
            hold,
            combo: 0,
            b2b: false,
            current: Piece::spawn(current),
            phase: Phase::Playing,
            pending_garbage: vec![],
        }
    }

    /// Blockfish's `queue[0]` is the piece being placed now, so the piece in
    /// play has to lead the queue — tet-rs's snapshot queue excludes it.
    #[test]
    fn to_bf_snapshot_puts_the_piece_in_play_first() {
        let snap = snapshot(
            MinoType::S,
            vec![MinoType::T, MinoType::I],
            Some(MinoType::O),
        );
        let bf = to_bf_snapshot(&snap);
        let queue: Vec<char> = bf.queue.iter().map(blockfish::Color::as_char).collect();
        assert_eq!(queue, vec!['S', 'T', 'I']);
        assert_eq!(bf.hold.map(|c| c.as_char()), Some('O'));
    }

    /// Same, with an empty hold slot: the piece in play still leads.
    #[test]
    fn to_bf_snapshot_puts_the_piece_in_play_first_without_hold() {
        let snap = snapshot(MinoType::L, vec![MinoType::J, MinoType::Z], None);
        let bf = to_bf_snapshot(&snap);
        let queue: Vec<char> = bf.queue.iter().map(blockfish::Color::as_char).collect();
        assert_eq!(queue, vec!['L', 'J', 'Z']);
        assert!(bf.hold.is_none());
    }

    /// The matrix is the board without the piece in play (blockfish places
    /// that piece itself), so the board must be copied verbatim.
    #[test]
    fn to_bf_snapshot_copies_the_board() {
        let mut snap = snapshot(MinoType::T, vec![], None);
        snap.board
            .set(tet_domain::Vec2::new(3, 0), Cell::Block(MinoType::I));
        let bf = to_bf_snapshot(&snap);
        assert_eq!(bf.matrix.cols(), 10);
        assert!(bf.matrix.get((0, 3)));
        assert!(!bf.matrix.get((0, 4)));
    }
}
