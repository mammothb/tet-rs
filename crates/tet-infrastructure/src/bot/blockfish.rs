use async_trait::async_trait;
use tet_application::{BotError, BotMove, BotPieceLocation, BotSpin, BotTransport, PlayerSnapshot};
use tet_domain::{Board, Cell, MinoType, Orientation, Ruleset};

pub struct BlockfishBot {
    config: blockfish::Config,
    shape_table: blockfish::ShapeTable,
    snapshot: Option<blockfish::Snapshot>,
}

impl BlockfishBot {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: blockfish::Config::default(),
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

fn to_bf_snapshot(snapshot: &PlayerSnapshot) -> blockfish::Snapshot {
    let hold = snapshot.hold.map(to_bf_color);
    let queue = snapshot.queue.iter().copied().map(to_bf_color).collect();
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
