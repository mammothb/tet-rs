/// Monotonic milliseconds since game start. Provided by the composition root
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GameTime(u64);

impl GameTime {
    pub const ZERO: Self = Self(0);

    #[must_use]
    pub const fn from_millis(ms: u64) -> Self {
        Self(ms)
    }

    #[must_use]
    pub const fn as_millis(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn saturating_millis_since(self, rhs: Self) -> u64 {
        self.0.saturating_sub(rhs.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // 1. Parameterized test for saturating subtraction (combines normal, equal, underflow, and extreme cases)
    #[rstest]
    #[case(100, 40, 60)] // Normal case: t1 is later than t2
    #[case(100, 100, 0)] // Equal case: same time instance
    #[case(40, 100, 0)] // Underflow case: measuring backwards
    #[case(u64::MAX, 0, u64::MAX)] // Extreme boundary: max time minus zero
    #[case(u64::MAX, u64::MAX, 0)] // Extreme boundary: max time minus itself
    fn test_saturating_millis_since(
        #[case] lhs_ms: u64,
        #[case] rhs_ms: u64,
        #[case] expected: u64,
    ) {
        let t1 = GameTime::from_millis(lhs_ms);
        let t2 = GameTime::from_millis(rhs_ms);
        assert_eq!(t1.saturating_millis_since(t2), expected);
    }

    // 2. Parameterized test for basic round-trip conversion
    #[rstest]
    #[case(GameTime::ZERO, 0)]
    #[case(GameTime::from_millis(42), 42)]
    #[case(GameTime::from_millis(u64::MAX), u64::MAX)]
    fn test_constructors_and_conversions(#[case] time: GameTime, #[case] expected_millis: u64) {
        assert_eq!(time.as_millis(), expected_millis);
    }
}
