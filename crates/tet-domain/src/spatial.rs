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
#[repr(i8)]
pub enum Rotation {
    CW = 1,
    CCW = -1,
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
