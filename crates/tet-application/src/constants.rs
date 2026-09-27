pub const LOCK_DELAY_FRAMES: u16 = 30;
pub const DAS_FRAMES: u16 = 16;
pub const ARR_FRAMES: u16 = 1;
/// Soft Drop Factor: soft drop fires `SDF_FRAMES` times per gravity interval.
/// SDF=15 → soft drop = 15 cells/sec when held (vs 1 cell/sec normal gravity).
/// Standard guideline value.
pub const SDF_FRAMES: u16 = 15;
pub const GARBAGE_DELAY_FRAMES: u8 = 8;
pub const GRAVITY_MS: u32 = 1000;
pub const ATTACK_FOR_LINES: [u8; 5] = [0, 0, 1, 2, 4];
