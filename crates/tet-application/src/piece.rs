use tet_domain::{MinoType, Orientation, Rotation, Vec2};

#[derive(Clone)]
pub struct Piece {
    pub kind: MinoType,
    pub orientation: Orientation,
    pub pos: Vec2,
}

impl Piece {
    /// Spawn at the guideline spawn position, North orientation.
    #[must_use]
    pub fn spawn(kind: MinoType) -> Self {
        Self {
            kind,
            orientation: Orientation::North,
            pos: kind.origin(),
        }
    }

    /// World-space cells for collision / locking / rendering.
    pub fn cells(&self) -> impl Iterator<Item = Vec2> + '_ {
        self.kind
            .coords(self.orientation)
            .iter()
            .map(move |&c| self.pos + c)
    }

    /// Translate the piece. Caller is responsible for collision checks.
    pub fn shift(&mut self, d: Vec2) {
        self.pos += d;
    }

    /// Change orientation. Does not apply kicks.
    pub fn rotate(&mut self, rot: Rotation) {
        self.orientation = self.orientation.rotate(rot);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use rstest::rstest;

    // -------- spawn --------

    #[rstest]
    #[case::j(MinoType::J, Vec2::new(3, 18))]
    #[case::l(MinoType::L, Vec2::new(3, 18))]
    #[case::s(MinoType::S, Vec2::new(3, 18))]
    #[case::z(MinoType::Z, Vec2::new(3, 18))]
    #[case::t(MinoType::T, Vec2::new(3, 18))]
    #[case::o(MinoType::O, Vec2::new(3, 18))]
    #[case::i(MinoType::I, Vec2::new(3, 17))] // I spawns one row lower
    fn spawn_sets_north_orientation_and_kind_origin(
        #[case] kind: MinoType,
        #[case] expected_pos: Vec2,
    ) {
        let p = Piece::spawn(kind);
        assert_eq!(p.kind, kind);
        assert_eq!(p.orientation, Orientation::North);
        assert_eq!(p.pos, expected_pos);
    }

    // -------- cells --------

    #[rstest]
    fn cells_yields_four_world_positions() {
        let p = Piece::spawn(MinoType::T);
        let cells: Vec<Vec2> = p.cells().collect();
        assert_eq!(cells.len(), 4);
        // First cell should equal pos + bbox-relative cell [0]
        let expected_first = p.pos + p.kind.coords(p.orientation)[0];
        assert_eq!(cells[0], expected_first);
    }

    // -------- shift --------

    #[rstest]
    fn shift_right_moves_pos_by_dx() {
        let mut p = Piece::spawn(MinoType::T);
        let original = p.pos;
        p.shift(Vec2::new(1, 0));
        assert_eq!(p.pos, original + Vec2::new(1, 0));
    }

    #[rstest]
    fn shift_left_moves_pos_by_negative_dx() {
        let mut p = Piece::spawn(MinoType::T);
        let original = p.pos;
        p.shift(Vec2::new(-1, 0));
        assert_eq!(p.pos, original + Vec2::new(-1, 0));
    }

    #[rstest]
    fn shift_up_moves_pos_by_positive_dy() {
        let mut p = Piece::spawn(MinoType::T);
        let original = p.pos;
        p.shift(Vec2::new(0, 1));
        assert_eq!(p.pos, original + Vec2::new(0, 1));
    }

    // -------- rotate --------

    #[rstest]
    fn rotate_cw_advances_orientation() {
        let mut p = Piece::spawn(MinoType::T);
        p.rotate(Rotation::CW);
        assert_eq!(p.orientation, Orientation::East);
    }

    #[rstest]
    fn rotate_ccw_retreats_orientation() {
        let mut p = Piece::spawn(MinoType::T);
        p.rotate(Rotation::CCW);
        assert_eq!(p.orientation, Orientation::West);
    }

    #[rstest]
    fn rotate_does_not_change_pos_or_kind() {
        let mut p = Piece::spawn(MinoType::T);
        let saved_pos = p.pos;
        let saved_kind = p.kind;
        p.rotate(Rotation::CW);
        assert_eq!(p.pos, saved_pos);
        assert_eq!(p.kind, saved_kind);
    }
}
