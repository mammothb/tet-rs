use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use crate::MinoType;

#[macro_export]
macro_rules! v2 {
    ($x:expr, $y:expr $(,)?) => {
        $crate::Vec2::new($x, $y)
    };
    ($([$x:expr, $y:expr]),+ $(,)?) => {
        [$($crate::Vec2::new($x, $y)),+]
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Vec2 {
    pub x: i8,
    pub y: i8,
}

impl Vec2 {
    #[must_use]
    pub const fn new(x: i8, y: i8) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub const fn add_const(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }

    #[must_use]
    pub const fn sub_const(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Add for Vec2 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign for Vec2 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub for Vec2 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl SubAssign for Vec2 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl Neg for Vec2 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Cell {
    #[default]
    Empty,
    Garbage,
    Block(MinoType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rotation {
    CW,
    CCW,
}

impl Rotation {
    #[must_use]
    pub const fn sign(self) -> i8 {
        match self {
            Self::CW => 1,
            Self::CCW => -1,
        }
    }
}

impl Neg for Rotation {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self::Output {
        match self {
            Self::CW => Self::CCW,
            Self::CCW => Self::CW,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::MinoType;
    use rstest::rstest;

    // -------- Vec2 --------

    #[rstest]
    #[case::origin(Vec2::new(0, 0), 0, 0)]
    #[case::positive(Vec2::new(3, 5), 3, 5)]
    #[case::negative(Vec2::new(-2, -7), -2, -7)]
    fn vec2_new_stores_components(#[case] v: Vec2, #[case] x: i8, #[case] y: i8) {
        assert_eq!((v.x, v.y), (x, y));
    }

    #[rstest]
    #[case::pos_pos(Vec2::new(1, 2), Vec2::new(3, 4), Vec2::new(4, 6))]
    #[case::pos_neg(Vec2::new(5, 5), Vec2::new(-2, -3), Vec2::new(3, 2))]
    #[case::zero(Vec2::new(7, 8), Vec2::new(0, 0), Vec2::new(7, 8))]
    fn vec2_add(#[case] a: Vec2, #[case] b: Vec2, #[case] expected: Vec2) {
        assert_eq!(a + b, expected);
    }

    #[rstest]
    #[case::pos_pos(Vec2::new(5, 7), Vec2::new(2, 3), Vec2::new(3, 4))]
    #[case::neg_result(Vec2::new(2, 3), Vec2::new(5, 7), Vec2::new(-3, -4))]
    fn vec2_sub(#[case] a: Vec2, #[case] b: Vec2, #[case] expected: Vec2) {
        assert_eq!(a - b, expected);
    }

    #[rstest]
    #[case::pos(Vec2::new(3, 5), Vec2::new(-3, -5))]
    #[case::neg(Vec2::new(-2, -7), Vec2::new(2, 7))]
    #[case::origin(Vec2::new(0, 0), Vec2::new(0, 0))]
    fn vec2_neg(#[case] v: Vec2, #[case] expected: Vec2) {
        assert_eq!(-v, expected);
    }

    #[rstest]
    #[case(Vec2::new(1, 2), Vec2::new(3, 4), Vec2::new(4, 6))]
    #[case(Vec2::new(5, 5), Vec2::new(-2, -3), Vec2::new(3, 2))]
    fn add_const_matches_operator(#[case] a: Vec2, #[case] b: Vec2, #[case] expected: Vec2) {
        assert_eq!(a.add_const(b), expected);
        assert_eq!(a.add_const(b), a + b);
    }

    #[rstest]
    #[case(Vec2::new(5, 7), Vec2::new(2, 3), Vec2::new(3, 4))]
    #[case(Vec2::new(2, 3), Vec2::new(5, 7), Vec2::new(-3, -4))]
    fn sub_const_matches_operator(#[case] a: Vec2, #[case] b: Vec2, #[case] expected: Vec2) {
        assert_eq!(a.sub_const(b), expected);
        assert_eq!(a.sub_const(b), a - b);
    }

    #[rstest]
    #[case(Vec2::new(1, 2), Vec2::new(3, 4), Vec2::new(4, 6))]
    #[case(Vec2::new(5, 5), Vec2::new(-2, -3), Vec2::new(3, 2))]
    fn vec2_add_assign(#[case] mut a: Vec2, #[case] b: Vec2, #[case] expected: Vec2) {
        a += b;
        assert_eq!(a, expected);
    }

    #[rstest]
    #[case(Vec2::new(5, 7), Vec2::new(2, 3), Vec2::new(3, 4))]
    #[case(Vec2::new(2, 3), Vec2::new(5, 7), Vec2::new(-3, -4))]
    fn vec2_sub_assign(#[case] mut a: Vec2, #[case] b: Vec2, #[case] expected: Vec2) {
        a -= b;
        assert_eq!(a, expected);
    }

    // -------- Cell --------

    #[test]
    fn cell_default_is_empty() {
        assert_eq!(Cell::default(), Cell::Empty);
    }

    #[rstest]
    #[case::same_empty(Cell::Empty, Cell::Empty, true)]
    #[case::empty_vs_garbage(Cell::Empty, Cell::Garbage, false)]
    #[case::garbage_vs_block(Cell::Garbage, Cell::Block(MinoType::I), false)]
    #[case::same_block(Cell::Block(MinoType::I), Cell::Block(MinoType::I), true)]
    #[case::diff_block(Cell::Block(MinoType::I), Cell::Block(MinoType::O), false)]
    fn cell_equality(#[case] a: Cell, #[case] b: Cell, #[case] expected: bool) {
        assert_eq!(a == b, expected);
    }

    // -------- Rotation --------

    #[rstest]
    #[case::cw(Rotation::CW, 1)]
    #[case::ccw(Rotation::CCW, -1)]
    fn rotation_sign(#[case] r: Rotation, #[case] expected: i8) {
        assert_eq!(r.sign(), expected);
    }

    #[rstest]
    #[case::neg_cw(Rotation::CW, Rotation::CCW)]
    #[case::neg_ccw(Rotation::CCW, Rotation::CW)]
    fn rotation_neg_swaps(#[case] r: Rotation, #[case] expected: Rotation) {
        assert_eq!(-r, expected);
    }

    #[rstest]
    #[case(Rotation::CW)]
    #[case(Rotation::CCW)]
    fn rotation_double_neg_is_identity(#[case] r: Rotation) {
        assert_eq!(-(-r), r);
    }

    // -------- v2! macro --------

    #[rstest]
    #[case::pos(v2![3, 5], Vec2::new(3, 5))]
    #[case::neg(v2![-2, -7], Vec2::new(-2, -7))]
    #[case::zero(v2![0, 0], Vec2::new(0, 0))]
    fn v2_macro_pair(#[case] got: Vec2, #[case] expected: Vec2) {
        assert_eq!(got, expected);
    }

    #[test]
    fn v2_macro_array() {
        let arr: [Vec2; 3] = v2![[0, 1], [1, 2], [2, 3]];
        assert_eq!(arr, [Vec2::new(0, 1), Vec2::new(1, 2), Vec2::new(2, 3)]);
    }

    #[test]
    fn v2_macro_array_trailing_comma() {
        let arr: [Vec2; 3] = v2![[0, 1], [1, 2], [2, 3],];
        assert_eq!(arr, [Vec2::new(0, 1), Vec2::new(1, 2), Vec2::new(2, 3)]);
    }
}
