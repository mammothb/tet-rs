use crate::{Orientation, Rotation, Vec2, v2};

pub type KickTable = [&'static [Vec2]; 4];

pub struct Mino {
    pub coords: [[Vec2; 4]; 4],
    pub origin: Vec2,
    /// References to static kick tables: [CW, CCW]
    pub kicks: [&'static KickTable; 2],
}

impl Mino {
    #[must_use]
    pub fn kicks(&self, target: Orientation, rot: Rotation) -> &'static [Vec2] {
        let rot_idx = match rot {
            Rotation::CW => 0,
            Rotation::CCW => 1,
        };
        let target_idx = target as usize;

        self.kicks[rot_idx][target_idx]
    }
}

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
    pub const SIZE: usize = 7;

    pub const ALL: [MinoType; MinoType::SIZE] = [
        MinoType::I,
        MinoType::O,
        MinoType::T,
        MinoType::L,
        MinoType::J,
        MinoType::S,
        MinoType::Z,
    ];

    #[must_use]
    pub const fn mino(self) -> &'static Mino {
        match self {
            MinoType::J => &MINO_J,
            MinoType::L => &MINO_L,
            MinoType::S => &MINO_S,
            MinoType::Z => &MINO_Z,
            MinoType::T => &MINO_T,
            MinoType::I => &MINO_I,
            MinoType::O => &MINO_O,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            MinoType::J => "J",
            MinoType::L => "L",
            MinoType::S => "S",
            MinoType::Z => "Z",
            MinoType::T => "T",
            MinoType::I => "I",
            MinoType::O => "O",
        }
    }

    #[must_use]
    pub fn coords(self, orientation: Orientation) -> &'static [Vec2; 4] {
        &self.mino().coords[orientation as usize]
    }

    #[must_use]
    pub fn origin(self) -> Vec2 {
        self.mino().origin
    }
}

static JLSZT_CW: KickTable = [
    // Target 0 (L -> 0)
    &v2![[0, 0], [-1, 0], [-1, -1], [0, 2], [-1, 2]],
    // Target R (0 -> R)
    &v2![[0, 0], [-1, 0], [-1, 1], [0, -2], [-1, -2]],
    // Target 2 (R -> 2)
    &v2![[0, 0], [1, 0], [1, -1], [0, 2], [1, 2]],
    // Target L (2 -> L)
    &v2![[0, 0], [1, 0], [1, 1], [0, -2], [1, -2]],
];

static JLSZT_CCW: KickTable = [
    // Target 0 (R -> 0)
    &v2![[0, 0], [1, 0], [1, -1], [0, 2], [1, 2]],
    // Target R (2 -> R)
    &v2![[0, 0], [-1, 0], [-1, 1], [0, -2], [-1, -2]],
    // Target 2 (L -> 2)
    &v2![[0, 0], [-1, 0], [-1, -1], [0, 2], [-1, 2]],
    // Target L (0 -> L)
    &v2![[0, 0], [1, 0], [1, 1], [0, -2], [1, -2]],
];

static I_CW: KickTable = [
    // Target 0 (L -> 0)
    &v2![[0, 0], [1, 0], [-2, 0], [1, -2], [-2, 1]],
    // Target R (0 -> R)
    &v2![[0, 0], [-2, 0], [1, 0], [-2, -1], [1, 2]],
    // Target 2 (R -> 2)
    &v2![[0, 0], [-1, 0], [2, 0], [-1, 2], [2, -1]],
    // Target L (2 -> L)
    &v2![[0, 0], [2, 0], [-1, 0], [2, 1], [-1, -2]],
];

static I_CCW: KickTable = [
    // Target 0 (R -> 0)
    &v2![[0, 0], [2, 0], [-1, 0], [2, 1], [-1, -2]],
    // Target R (2 -> R)
    &v2![[0, 0], [1, 0], [-2, 0], [1, -2], [-2, 1]],
    // Target 2 (L -> 2)
    &v2![[0, 0], [-2, 0], [1, 0], [-2, -1], [1, 2]],
    // Target L (0 -> L)
    &v2![[0, 0], [-1, 0], [2, 0], [-1, 2], [2, -1]],
];

static NO_KICKS: KickTable = [&[v2![0, 0]], &[v2![0, 0]], &[v2![0, 0]], &[v2![0, 0]]];

static MINO_J: Mino = Mino {
    origin: v2![3, 18],
    coords: [
        v2![[0, 1], [1, 1], [2, 1], [0, 2]],
        v2![[1, 0], [1, 1], [1, 2], [2, 2]],
        v2![[2, 0], [0, 1], [1, 1], [2, 1]],
        v2![[0, 0], [1, 0], [1, 1], [1, 2]],
    ],
    kicks: [&JLSZT_CW, &JLSZT_CCW],
};

static MINO_L: Mino = Mino {
    origin: v2![3, 18],
    coords: [
        v2![[0, 1], [1, 1], [2, 1], [2, 2]],
        v2![[1, 0], [2, 0], [1, 1], [1, 2]],
        v2![[0, 0], [0, 1], [1, 1], [2, 1]],
        v2![[1, 0], [1, 1], [0, 2], [1, 2]],
    ],
    kicks: [&JLSZT_CW, &JLSZT_CCW],
};

static MINO_S: Mino = Mino {
    origin: v2![3, 18],
    coords: [
        v2![[0, 1], [1, 1], [1, 2], [2, 2]],
        v2![[2, 0], [1, 1], [2, 1], [1, 2]],
        v2![[0, 0], [1, 0], [1, 1], [2, 1]],
        v2![[1, 0], [0, 1], [1, 1], [0, 2]],
    ],
    kicks: [&JLSZT_CW, &JLSZT_CCW],
};

static MINO_Z: Mino = Mino {
    origin: v2![3, 18],
    coords: [
        v2![[1, 1], [2, 1], [0, 2], [1, 2]],
        v2![[1, 0], [1, 1], [2, 1], [2, 2]],
        v2![[1, 0], [2, 0], [0, 1], [1, 1]],
        v2![[0, 0], [0, 1], [1, 1], [1, 2]],
    ],
    kicks: [&JLSZT_CW, &JLSZT_CCW],
};

static MINO_T: Mino = Mino {
    origin: v2![3, 18],
    coords: [
        v2![[0, 1], [1, 1], [2, 1], [1, 2]],
        v2![[1, 0], [1, 1], [2, 1], [1, 2]],
        v2![[1, 0], [0, 1], [1, 1], [2, 1]],
        v2![[1, 0], [0, 1], [1, 1], [1, 2]],
    ],
    kicks: [&JLSZT_CW, &JLSZT_CCW],
};

static MINO_I: Mino = Mino {
    origin: v2![3, 17],
    coords: [
        v2![[0, 2], [1, 2], [2, 2], [3, 2]],
        v2![[2, 0], [2, 1], [2, 2], [2, 3]],
        v2![[0, 1], [1, 1], [2, 1], [3, 1]],
        v2![[1, 0], [1, 1], [1, 2], [1, 3]],
    ],
    kicks: [&I_CW, &I_CCW],
};

static MINO_O: Mino = Mino {
    origin: v2![3, 18],
    coords: [
        v2![[1, 1], [2, 1], [1, 2], [2, 2]],
        v2![[1, 1], [2, 1], [1, 2], [2, 2]],
        v2![[1, 1], [2, 1], [1, 2], [2, 2]],
        v2![[1, 1], [2, 1], [1, 2], [2, 2]],
    ],
    kicks: [&NO_KICKS, &NO_KICKS],
};
