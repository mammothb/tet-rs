use tet_domain::{Board, MinoType};

use crate::{PendingGarbage, Phase, Piece};

pub struct PlayerSnapshot {
    pub board: Board,
    pub queue: Vec<MinoType>,
    pub hold: Option<MinoType>,
    pub combo: i32,
    pub b2b: bool,
    pub current: Piece,
    pub phase: Phase,
    pub pending_garbage: Vec<PendingGarbage>,
}
