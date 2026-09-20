use crate::{MinoType, Orientation, Rotation, Vec2, v2};

// J, L, S, Z, T Kicks
static JLSZT_CW_KICKS: [[Vec2; 5]; 4] = [
    // Target 0 (L -> 0)
    v2![[0, 0], [-1, 0], [-1, -1], [0, 2], [-1, 2]],
    // Target R (0 -> R)
    v2![[0, 0], [-1, 0], [-1, 1], [0, -2], [-1, -2]],
    // Target 2 (R -> 2)
    v2![[0, 0], [1, 0], [1, -1], [0, 2], [1, 2]],
    // Target L (2 -> L)
    v2![[0, 0], [1, 0], [1, 1], [0, -2], [1, -2]],
];

static JLSZT_CCW_KICKS: [[Vec2; 5]; 4] = [
    // Target 0 (R -> 0)
    v2![[0, 0], [1, 0], [1, -1], [0, 2], [1, 2],],
    // Target R (2 -> R)
    v2![[0, 0], [-1, 0], [-1, 1], [0, -2], [-1, -2],],
    // Target 2 (L -> 2)
    v2![[0, 0], [-1, 0], [-1, -1], [0, 2], [-1, 2],],
    // Target L (0 -> L)
    v2![[0, 0], [1, 0], [1, 1], [0, -2], [1, -2],],
];

// I Kicks
static I_CW_KICKS: [[Vec2; 5]; 4] = [
    // Target 0 (L -> 0)
    v2![[0, 0], [1, 0], [-2, 0], [1, -2], [-2, 1],],
    // Target R (0 -> R)
    v2![[0, 0], [-2, 0], [1, 0], [-2, -1], [1, 2],],
    // Target 2 (R -> 2)
    v2![[0, 0], [-1, 0], [2, 0], [-1, 2], [2, -1],],
    // Target L (2 -> L)
    v2![[0, 0], [2, 0], [-1, 0], [2, 1], [-1, -2],],
];

static I_CCW_KICKS: [[Vec2; 5]; 4] = [
    // Target 0 (R -> 0)
    v2![[0, 0], [2, 0], [-1, 0], [2, 1], [-1, -2],],
    // Target R (2 -> R)
    v2![[0, 0], [1, 0], [-2, 0], [1, -2], [-2, 1],],
    // Target 2 (L -> 2)
    v2![[0, 0], [-2, 0], [1, 0], [-2, -1], [1, 2],],
    // Target L (0 -> L)
    v2![[0, 0], [-1, 0], [2, 0], [-1, 2], [2, -1],],
];

static NO_KICKS: [Vec2; 5] = [v2![0, 0]; 5];

/// Look up the 5 kick tests for a piece transition target
#[must_use]
pub fn get_kick_tests(
    piece: MinoType,
    target_orient: Orientation,
    dir: Rotation,
) -> &'static [Vec2; 5] {
    if piece == MinoType::O {
        return &NO_KICKS;
    }

    let target_idx = target_orient as usize;

    match (piece, dir) {
        (MinoType::I, Rotation::CW) => &I_CW_KICKS[target_idx],
        (MinoType::I, Rotation::CCW) => &I_CCW_KICKS[target_idx],
        (_, Rotation::CW) => &JLSZT_CW_KICKS[target_idx],
        (_, Rotation::CCW) => &JLSZT_CCW_KICKS[target_idx],
    }
}
