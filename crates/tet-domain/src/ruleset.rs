use crate::{MinoType, Orientation, Rotation, Vec2, mino::Mino};

#[derive(Debug)]
pub struct Ruleset {
    pub num_cols: usize,
    pub num_rows: usize,
    pub num_visible_rows: usize,
    pub num_preview: usize,
}

impl Ruleset {
    #[must_use]
    pub const fn guideline() -> Self {
        Self {
            num_cols: 10,
            num_rows: 25,
            num_visible_rows: 20,
            num_preview: 5,
        }
    }

    #[must_use]
    #[inline]
    pub fn mino(&self, kind: MinoType) -> &'static Mino {
        kind.mino()
    }

    #[must_use]
    #[inline]
    pub fn coords(&self, kind: MinoType, orientation: Orientation) -> &'static [Vec2; 4] {
        kind.coords(orientation)
    }

    #[must_use]
    #[inline]
    pub fn origin(&self, kind: MinoType) -> Vec2 {
        kind.origin()
    }

    #[must_use]
    #[inline]
    pub fn kicks(&self, kind: MinoType, target: Orientation, rot: Rotation) -> &'static [Vec2] {
        kind.mino().kicks(target, rot)
    }
}
