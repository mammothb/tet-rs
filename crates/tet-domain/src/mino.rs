use crate::{Orientation, Vec2, v2};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MinoType {
    J,
    L,
    S,
    Z,
    T,
    I,
    O,
}

impl MinoType {
    #[must_use]
    pub const fn origin(self) -> Vec2 {
        match self {
            MinoType::I => v2![3, 17],
            _ => v2![3, 18],
        }
    }

    /// Local 4-block bounding box coordinates matching the YAML spec
    #[must_use]
    pub const fn local_coords(self, orientation: Orientation) -> [Vec2; 4] {
        match (self, orientation) {
            // J
            (MinoType::J, Orientation::O0) => v2![[0, 1], [1, 1], [2, 1], [0, 2]],
            (MinoType::J, Orientation::OR) => v2![[1, 0], [1, 1], [1, 2], [2, 2]],
            (MinoType::J, Orientation::O2) => v2![[2, 0], [0, 1], [1, 1], [2, 1]],
            (MinoType::J, Orientation::OL) => v2![[0, 0], [1, 0], [1, 1], [1, 2]],

            // L
            (MinoType::L, Orientation::O0) => v2![[0, 1], [1, 1], [2, 1], [2, 2]],
            (MinoType::L, Orientation::OR) => v2![[1, 0], [2, 0], [1, 1], [1, 2]],
            (MinoType::L, Orientation::O2) => v2![[0, 0], [0, 1], [1, 1], [2, 1]],
            (MinoType::L, Orientation::OL) => v2![[1, 0], [1, 1], [0, 2], [1, 2]],

            // S
            (MinoType::S, Orientation::O0) => v2![[0, 1], [1, 1], [1, 2], [2, 2]],
            (MinoType::S, Orientation::OR) => v2![[2, 0], [1, 1], [2, 1], [1, 2]],
            (MinoType::S, Orientation::O2) => v2![[0, 0], [1, 0], [1, 1], [2, 1]],
            (MinoType::S, Orientation::OL) => v2![[1, 0], [0, 1], [1, 1], [0, 2]],

            // Z
            (MinoType::Z, Orientation::O0) => v2![[1, 1], [2, 1], [0, 2], [1, 2]],
            (MinoType::Z, Orientation::OR) => v2![[1, 0], [1, 1], [2, 1], [2, 2]],
            (MinoType::Z, Orientation::O2) => v2![[1, 0], [2, 0], [0, 1], [1, 1]],
            (MinoType::Z, Orientation::OL) => v2![[0, 0], [0, 1], [1, 1], [1, 2]],

            // T
            (MinoType::T, Orientation::O0) => v2![[0, 1], [1, 1], [2, 1], [1, 2]],
            (MinoType::T, Orientation::OR) => v2![[1, 0], [1, 1], [2, 1], [1, 2]],
            (MinoType::T, Orientation::O2) => v2![[1, 0], [0, 1], [1, 1], [2, 1]],
            (MinoType::T, Orientation::OL) => v2![[1, 0], [0, 1], [1, 1], [1, 2]],

            // I
            (MinoType::I, Orientation::O0) => v2![[0, 2], [1, 2], [2, 2], [3, 2]],
            (MinoType::I, Orientation::OR) => v2![[2, 0], [2, 1], [2, 2], [2, 3]],
            (MinoType::I, Orientation::O2) => v2![[0, 1], [1, 1], [2, 1], [3, 1]],
            (MinoType::I, Orientation::OL) => v2![[1, 0], [1, 1], [1, 2], [1, 3]],

            // O (Same for all orientations)
            (MinoType::O, _) => v2![[1, 1], [2, 1], [1, 2], [2, 2]],
        }
    }
}
