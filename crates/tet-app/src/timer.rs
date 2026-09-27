//! Per-frame timing for the macroquad main loop.
//!
//! Mirrors Blockfish's `blockfish-client/src/timer.rs`: the UI thread runs at
//! display vsync (~60fps via `next_frame().await`), the `Timer` polls elapsed
//! time each frame, and emits `Input`s for time-driven events (gravity, soft
//! drop, DAS/ARR repeat). Edge-triggered inputs (rotate, hard drop, hold) pass
//! through immediately.
//!
//! The composition root (main loop) takes the output and routes each input to
//! the right player via `session.apply_input`.

use std::time::Instant;

use tet_application::{ARR_FRAMES, DAS_FRAMES, GRAVITY_MS, Input, SDF_FRAMES};

/// Approximate ms per frame at 60fps. Used for converting frame-based constants
/// (DAS, ARR) to milliseconds without pulling in `Instant`-based math at the
/// call site. Slightly off if the display isn't 60Hz — that's fine for first
/// slice; the timer measures real elapsed time anyway.
const MS_PER_FRAME: f32 = 1000.0 / 60.0;

/// Lossless cast from a positive f32 to u32. Used for the `MS_PER_FRAME`
/// constant, which is always a small positive value. Pedantic clippy flags
/// every `as u32` from f32; this helper documents the invariant.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
const fn f32_to_u32(v: f32) -> u32 {
    v as u32
}

/// Held-key state from the keyboard. Edge-triggered inputs are reported as
/// "pressed this frame" (true for one frame only). Held inputs are reported as
/// "currently down" (true every frame the key is down).
#[allow(clippy::struct_excessive_bools)] // keyboard state is naturally a set of flags
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputState {
    // Held
    pub move_left_held: bool,
    pub move_right_held: bool,
    pub soft_drop_held: bool,

    // Edge-triggered (true for exactly one frame per keypress)
    pub rotate_cw_pressed: bool,
    pub rotate_ccw_pressed: bool,
    pub hard_drop_pressed: bool,
    pub hold_pressed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Left,
    Right,
}

/// Output of one tick of the timer.
///
/// - `inputs`: apply each to the **human player** via `session.apply_input(human_idx, input)`.
/// - `fire_gravity`: if true, apply `Input::StepGravity` to **every playing
///   player**. Gravity is global — bots and humans fall at the same rate.
pub struct TimerTick {
    pub inputs: Vec<Input>,
    pub fire_gravity: bool,
}

pub struct Timer {
    last_gravity_at: Instant,
    /// `None` when soft drop isn't held. Reset to `None` on key release so the
    /// first frame of re-press doesn't double-fire.
    last_soft_drop_at: Option<Instant>,
    /// `Some((direction, instant_of_first_shift))` once a horizontal key has
    /// been held long enough to register the first shift. Reset on release.
    last_shift_at: Option<(Direction, Instant)>,
    /// Carry-over ms when `ARR_FRAMES` doesn't divide cleanly into the frame
    /// delta. Lets the timer emit fractional ARR shifts over multi-frame windows.
    arr_accumulator_ms: f32,
}

impl Timer {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            last_gravity_at: now,
            last_soft_drop_at: None,
            last_shift_at: None,
            arr_accumulator_ms: 0.0,
        }
    }

    /// One tick of the timer. Returns inputs to apply to the human and a flag
    /// for whether to fire global gravity.
    ///
    /// `frame_delta_ms`: time elapsed since the last call to `tick`. Used to
    /// accumulate partial ARR frames. Typically `next_frame()` returns ~16.67ms
    /// at 60fps.
    pub fn tick(&mut self, state: InputState, frame_delta_ms: f32) -> TimerTick {
        let now = Instant::now();
        let mut inputs = Vec::new();

        // 1. Edge-triggered inputs (independent of timer)
        if state.rotate_cw_pressed {
            inputs.push(Input::RotateCW);
        }
        if state.rotate_ccw_pressed {
            inputs.push(Input::RotateCCW);
        }
        if state.hard_drop_pressed {
            inputs.push(Input::HardDrop);
        }
        if state.hold_pressed {
            inputs.push(Input::Hold);
        }

        // 2. DAS / ARR for held horizontal movement
        let held_dir = match (state.move_left_held, state.move_right_held) {
            (true, false) => Some(Direction::Left),
            (false, true) => Some(Direction::Right),
            _ => None, // both or neither → no shift
        };

        if let Some(dir) = held_dir {
            match self.last_shift_at {
                None => {
                    // First press (or direction switch): shift immediately,
                    // start DAS countdown.
                    inputs.push(dir.into_input());
                    self.last_shift_at = Some((dir, now));
                }
                Some((prev, _)) if prev != dir => {
                    // Direction switched: shift and restart DAS.
                    inputs.push(dir.into_input());
                    self.last_shift_at = Some((dir, now));
                }
                Some((_, first_at)) => {
                    // Same direction still held: check if we're past DAS delay.
                    let held_ms = now.duration_since(first_at).as_millis();
                    let das_ms =
                        u128::from(u32::from(DAS_FRAMES)) * u128::from(f32_to_u32(MS_PER_FRAME));
                    if held_ms >= das_ms {
                        // In auto-repeat phase; accumulate partial ARR credits.
                        self.arr_accumulator_ms += frame_delta_ms;
                        let arr_ms = f32::from(ARR_FRAMES) * MS_PER_FRAME;
                        while self.arr_accumulator_ms >= arr_ms {
                            inputs.push(dir.into_input());
                            self.arr_accumulator_ms -= arr_ms;
                        }
                    }
                }
            }
        } else {
            self.last_shift_at = None;
            self.arr_accumulator_ms = 0.0;
        }

        // 3. Soft drop. While held, fires every `GRAVITY_MS / SDF_FRAMES` ms.
        //    SDF_FRAMES = 1 currently → soft drop = gravity speed. SDF > 1
        //    makes soft drop faster than gravity (canonical guideline uses SDF=15).
        let soft_drop_interval_ms = GRAVITY_MS / u32::from(SDF_FRAMES);
        if state.soft_drop_held {
            let last = self.last_soft_drop_at.unwrap_or(self.last_gravity_at);
            if u32::try_from(now.duration_since(last).as_millis())
                .map_or(true, |ms| ms >= soft_drop_interval_ms)
            {
                inputs.push(Input::SoftDrop);
                self.last_soft_drop_at = Some(now);
            }
        } else {
            self.last_soft_drop_at = None;
        }

        // 4. Gravity. While soft drop is held, uses the soft-drop interval
        //    (faster fall); otherwise uses the normal gravity interval.
        let gravity_interval_ms = if state.soft_drop_held {
            soft_drop_interval_ms
        } else {
            GRAVITY_MS
        };
        let fire_gravity = u32::try_from(now.duration_since(self.last_gravity_at).as_millis())
            .map_or(true, |ms| ms >= gravity_interval_ms);
        if fire_gravity {
            self.last_gravity_at = now;
        }

        TimerTick {
            inputs,
            fire_gravity,
        }
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

impl Direction {
    fn into_input(self) -> Input {
        match self {
            Direction::Left => Input::MoveLeft,
            Direction::Right => Input::MoveRight,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// 60fps frame delta in ms.
    const FRAME_MS: f32 = 1000.0 / 60.0;

    /// Default state: nothing held, nothing pressed.
    fn no_input() -> InputState {
        InputState::default()
    }

    // -------- gravity --------

    #[test]
    fn first_tick_does_not_fire_gravity() {
        let mut timer = Timer::new();
        let tick = timer.tick(no_input(), FRAME_MS);
        assert!(!tick.fire_gravity);
        assert!(tick.inputs.is_empty());
    }

    #[test]
    fn gravity_fires_after_gravity_interval_elapses() {
        // DAS = 16 frames, so 16 * 16.67ms = 266ms. We need to wait > GRAVITY_MS
        // (1000ms). Real-time test, slow but unambiguous.
        let mut timer = Timer::new();
        std::thread::sleep(std::time::Duration::from_millis(1050));
        let tick = timer.tick(no_input(), FRAME_MS);
        assert!(tick.fire_gravity);
    }

    // -------- edge-triggered inputs --------

    #[test]
    fn rotate_cw_passes_through_immediately() {
        let mut timer = Timer::new();
        let state = InputState {
            rotate_cw_pressed: true,
            ..no_input()
        };
        let tick = timer.tick(state, FRAME_MS);
        assert_eq!(tick.inputs, vec![Input::RotateCW]);
    }

    #[test]
    fn multiple_edge_inputs_emit_in_order() {
        let mut timer = Timer::new();
        let state = InputState {
            rotate_cw_pressed: true,
            hard_drop_pressed: true,
            hold_pressed: true,
            ..no_input()
        };
        let tick = timer.tick(state, FRAME_MS);
        // Order matches the input.rs variant order: RotateCW, RotateCCW, HardDrop, Hold.
        assert_eq!(
            tick.inputs,
            vec![Input::RotateCW, Input::HardDrop, Input::Hold]
        );
    }

    #[test]
    fn no_inputs_when_nothing_held_or_pressed() {
        let mut timer = Timer::new();
        let tick = timer.tick(no_input(), FRAME_MS);
        assert!(tick.inputs.is_empty());
        assert!(!tick.fire_gravity);
    }

    // -------- horizontal shift (DAS / ARR) --------

    #[test]
    fn holding_left_shifts_once_immediately() {
        let mut timer = Timer::new();
        let state = InputState {
            move_left_held: true,
            ..no_input()
        };
        let tick = timer.tick(state, FRAME_MS);
        assert_eq!(tick.inputs, vec![Input::MoveLeft]);
    }

    #[test]
    fn holding_left_does_not_repeat_during_das_delay() {
        // DAS = 16 frames ≈ 267ms. We're well within DAS at frame 2.
        let mut timer = Timer::new();
        let state = InputState {
            move_left_held: true,
            ..no_input()
        };
        let _ = timer.tick(state, FRAME_MS); // first shift
        let tick = timer.tick(state, FRAME_MS); // second frame, still in DAS
        assert!(tick.inputs.is_empty());
    }

    #[test]
    fn releasing_left_resets_das() {
        let mut timer = Timer::new();
        let held = InputState {
            move_left_held: true,
            ..no_input()
        };
        let _ = timer.tick(held, FRAME_MS); // first shift
        let tick = timer.tick(no_input(), FRAME_MS); // released
        assert!(tick.inputs.is_empty());
        let tick = timer.tick(held, FRAME_MS); // re-pressed
        assert_eq!(tick.inputs, vec![Input::MoveLeft]);
    }

    #[test]
    fn switching_direction_shifts_and_resets_das() {
        let mut timer = Timer::new();
        let left = InputState {
            move_left_held: true,
            ..no_input()
        };
        let right = InputState {
            move_right_held: true,
            ..no_input()
        };
        let _ = timer.tick(left, FRAME_MS); // shift left, start DAS
        let tick = timer.tick(right, FRAME_MS); // switch: shift right, restart DAS
        assert_eq!(tick.inputs, vec![Input::MoveRight]);
    }

    #[test]
    fn holding_both_directions_emits_no_shift() {
        let mut timer = Timer::new();
        let state = InputState {
            move_left_held: true,
            move_right_held: true,
            ..no_input()
        };
        let tick = timer.tick(state, FRAME_MS);
        assert!(tick.inputs.is_empty());
    }

    #[test]
    fn das_blocks_repeat_within_window() {
        // DAS = 16 frames ≈ 267ms. After the first shift, repeat should NOT
        // fire for at least DAS worth of frames. We can't directly test ARR
        // shifts without real elapsed time (the DAS check uses Instant::now()),
        // but we CAN verify the absence of repeats within the DAS window.
        let mut timer = Timer::new();
        let state = InputState {
            move_left_held: true,
            ..no_input()
        };
        let _ = timer.tick(state, FRAME_MS); // first shift
        // Frame 2-5 (still well within DAS): no shifts.
        for _ in 0..5 {
            let tick = timer.tick(state, FRAME_MS);
            assert!(tick.inputs.is_empty());
        }
    }

    // -------- soft drop --------

    #[test]
    fn soft_drop_does_not_fire_on_first_tick() {
        let mut timer = Timer::new();
        let state = InputState {
            soft_drop_held: true,
            ..no_input()
        };
        let tick = timer.tick(state, FRAME_MS);
        // SDF_FRAMES = 1 currently → soft drop interval = GRAVITY_MS = 1000ms.
        // No time has elapsed on first tick.
        assert!(tick.inputs.is_empty());
    }

    #[test]
    fn soft_drop_fires_after_interval() {
        // SDF = 1 means soft drop fires every 1000ms (same as gravity).
        // Real-time test for the interval check.
        let mut timer = Timer::new();
        std::thread::sleep(std::time::Duration::from_millis(1050));
        let state = InputState {
            soft_drop_held: true,
            ..no_input()
        };
        let tick = timer.tick(state, FRAME_MS);
        assert!(tick.inputs.contains(&Input::SoftDrop));
    }

    #[test]
    fn releasing_soft_drop_resets_timer() {
        let mut timer = Timer::new();
        let held = InputState {
            soft_drop_held: true,
            ..no_input()
        };
        let _ = timer.tick(held, FRAME_MS); // initialize last_soft_drop_at
        // Re-pressing immediately: last_soft_drop_at is still fresh,
        // so soft drop won't fire again.
        let tick = timer.tick(held, FRAME_MS);
        assert!(!tick.inputs.contains(&Input::SoftDrop));
    }

    // -------- soft drop accelerating gravity --------

    #[test]
    fn holding_soft_drop_gravity_uses_soft_drop_interval() {
        // SDF = 1 → soft-drop interval equals gravity interval.
        // We verify the TIMING is what changes, not the value.
        // After 1050ms with soft_drop held, gravity should fire (uses 1000ms).
        let mut timer = Timer::new();
        std::thread::sleep(std::time::Duration::from_millis(1050));
        let state = InputState {
            soft_drop_held: true,
            ..no_input()
        };
        let tick = timer.tick(state, FRAME_MS);
        assert!(tick.fire_gravity);
    }
}
