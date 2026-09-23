use tet_domain::{MinoType, Orientation, Rotation, Vec2};

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
