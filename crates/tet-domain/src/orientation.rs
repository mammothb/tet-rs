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
        let next = (current + rot.sign()).rem_euclid(4) as u8;
        match next {
            0 => Self::North,
            1 => Self::East,
            2 => Self::South,
            3 => Self::West,
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Rotation;

    use rstest::rstest;

    #[rstest]
    #[case::north_cw(Orientation::North, Rotation::CW, Orientation::East)]
    #[case::north_ccw(Orientation::North, Rotation::CCW, Orientation::West)]
    #[case::east_cw(Orientation::East, Rotation::CW, Orientation::South)]
    #[case::east_ccw(Orientation::East, Rotation::CCW, Orientation::North)]
    #[case::south_cw(Orientation::South, Rotation::CW, Orientation::West)]
    #[case::south_ccw(Orientation::South, Rotation::CCW, Orientation::East)]
    #[case::west_cw(Orientation::West, Rotation::CW, Orientation::North)]
    #[case::west_ccw(Orientation::West, Rotation::CCW, Orientation::South)]
    fn rotate_advances_one_step(
        #[case] start: Orientation,
        #[case] rot: Rotation,
        #[case] expected: Orientation,
    ) {
        assert_eq!(start.rotate(rot), expected);
    }

    #[rstest]
    #[case::full_cw_from_north(Orientation::North, [Rotation::CW; 4], Orientation::North)]
    #[case::full_ccw_from_north(Orientation::North, [Rotation::CCW; 4], Orientation::North)]
    #[case::full_cw_from_east(Orientation::East, [Rotation::CW; 4], Orientation::East)]
    #[case::full_cw_from_south(Orientation::South, [Rotation::CW; 4], Orientation::South)]
    #[case::full_cw_from_west(Orientation::West, [Rotation::CW; 4], Orientation::West)]
    fn four_rotations_return_to_origin(
        #[case] start: Orientation,
        #[case] rotations: [Rotation; 4],
        #[case] expected: Orientation,
    ) {
        let result = rotations
            .into_iter()
            .fold(start, super::Orientation::rotate);
        assert_eq!(result, expected);
    }
}
