/// Time-based lock delay. The piece locks this many milliseconds after it
/// first touches the ground (or its last successful move/rotation while
/// grounded).
pub const LOCK_DELAY_MS: u32 = 500;
pub const DAS_FRAMES: u16 = 16;
pub const ARR_FRAMES: u16 = 1;
/// Soft Drop Factor: soft drop fires `SDF_FRAMES` times per gravity interval.
/// SDF=15 → soft drop = 15 cells/sec when held (vs 1 cell/sec normal gravity).
/// Standard guideline value.
pub const SDF_FRAMES: u16 = 15;
pub const GARBAGE_DELAY_FRAMES: u8 = 8;
/// Milliseconds per row of gravity. Constant for the whole game.
pub const GRAVITY_MS: u32 = 1000;
pub const ATTACK_FOR_LINES: [u8; 5] = [0, 0, 1, 2, 4];
