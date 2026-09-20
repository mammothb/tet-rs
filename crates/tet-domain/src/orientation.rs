use crate::Rotation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Orientation {
    #[default]
    O0 = 0,
    OR = 1,
    O2 = 2,
    OL = 3,
}

impl Orientation {
    /// Advance orientation by a rotation action direction
    #[must_use]
    pub fn rotate(self, dir: Rotation) -> Self {
        let current = self as i8;
        let next = (current + dir as i8).rem_euclid(4) as u8;
        match next {
            0 => Self::O0,
            1 => Self::OR,
            2 => Self::O2,
            3 => Self::OL,
            _ => unreachable!(),
        }
    }
}
