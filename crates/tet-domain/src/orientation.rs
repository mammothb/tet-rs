use crate::Rotation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Orientation {
    #[default]
    North = 0,
    East = 1,
    South = 2,
    West = 3,
}

impl Orientation {
    /// Advance orientation by a rotation action direction
    #[must_use]
    pub fn rotate(self, rot: Rotation) -> Self {
        let current = self as i8;
        let next = (current + rot as i8).rem_euclid(4) as u8;
        match next {
            0 => Self::North,
            1 => Self::East,
            2 => Self::South,
            3 => Self::West,
            _ => unreachable!(),
        }
    }
}
