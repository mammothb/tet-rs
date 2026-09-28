//! Per-frame timing for the macroquad main loop.
//!
//! The Timer translates keyboard state into `Input` events. It's strictly an
//! input-concern component: edge-triggered events (rotate, hard drop, hold)
//! pass through immediately, DAS/ARR add repeats on held movement keys, and
//! soft drop emits at its own cadence while held.
//!
//! **Gravity timing lives in `tet-application::Player`, not here.** Each
//! `Player::step_player` ticks its own `last_gravity_at` clock. This split
//! mirrors Blockfish's architecture: the engine owns game timing, the UI owns
//! input timing. The two never bleed into each other, which prevents the
//! "soft drop coupling breaks gravity" class of bug.

use std::time::Instant;

use tet_application::{ARR_FRAMES, DAS_FRAMES, GRAVITY_MS, Input, SDF_FRAMES};

/// Approximate ms per frame at 60fps. Used for converting frame-based constants
/// (DAS, ARR) to milliseconds without pulling in `Instant`-based math at the
/// call site.
const MS_PER_FRAME: f32 = 1000.0 / 60.0;

/// Lossless cast from a positive f32 to u32. Used for the `MS_PER_FRAME`
/// constant, which is always a small positive value.
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

/// Output of one tick of the timer: a list of `Input` events to apply to the
/// human player via `session.apply_input(human_idx, input)`. No `fire_gravity`
/// — gravity is owned by `Player::step_player`.
pub struct TimerTick {
    pub inputs: Vec<Input>,
}

pub struct Timer {
    /// `Some((direction, instant_of_first_shift))` once a horizontal key has
    /// been held long enough to register the first shift. Reset on release.
    last_shift_at: Option<(Direction, Instant)>,
    /// Carry-over ms when ARR doesn't divide cleanly into the frame delta.
    arr_accumulator_ms: f32,
    /// Carry-over ms when `SDF_FRAMES` doesn't divide cleanly into the frame delta.
    soft_drop_accumulator_ms: f32,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            last_shift_at: None,
            arr_accumulator_ms: 0.0,
            soft_drop_accumulator_ms: 0.0,
        }
    }

    /// One tick of the timer. Returns inputs to apply to the human.
    ///
    /// `frame_delta_ms`: time elapsed since the last call to `tick`. Used to
    /// accumulate partial ARR and soft-drop frames.
    pub fn tick(&mut self, state: InputState, frame_delta_ms: f32) -> TimerTick {
        let now = Instant::now();
        let mut inputs = Vec::new();

        // 1. Edge-triggered inputs (independent of timer).
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

        // 2. DAS / ARR for held horizontal movement.
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

        // 3. Soft drop. While held, fires at SDF cadence (every
        //    `GRAVITY_MS / SDF_FRAMES` ms). With SDF=15, that's ~15 cells/sec
        //    on top of the 1 cell/sec gravity from `Player::step_player`.
        if state.soft_drop_held {
            #[allow(clippy::cast_precision_loss)]
            let sdf_interval_ms = GRAVITY_MS as f32 / f32::from(SDF_FRAMES);
            self.soft_drop_accumulator_ms += frame_delta_ms;
            while self.soft_drop_accumulator_ms >= sdf_interval_ms {
                inputs.push(Input::SoftDrop);
                self.soft_drop_accumulator_ms -= sdf_interval_ms;
            }
        } else {
            self.soft_drop_accumulator_ms = 0.0;
        }

        TimerTick { inputs }
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

    use rstest::rstest;

    /// 60fps frame delta in ms.
    const FRAME_MS: f32 = 1000.0 / 60.0;

    /// Default state: nothing held, nothing pressed.
    fn no_input() -> InputState {
        InputState::default()
    }

    // -------- edge-triggered inputs --------

    #[rstest]
    fn rotate_cw_passes_through_immediately() {
        let mut timer = Timer::new();
        let state = InputState {
            rotate_cw_pressed: true,
            ..no_input()
        };
        let tick = timer.tick(state, FRAME_MS);
        assert_eq!(tick.inputs, vec![Input::RotateCW]);
    }

    #[rstest]
    fn multiple_edge_inputs_emit_in_order() {
        let mut timer = Timer::new();
        let state = InputState {
            rotate_cw_pressed: true,
            hard_drop_pressed: true,
            hold_pressed: true,
            ..no_input()
        };
        let tick = timer.tick(state, FRAME_MS);
        assert_eq!(
            tick.inputs,
            vec![Input::RotateCW, Input::HardDrop, Input::Hold]
        );
    }

    #[rstest]
    fn no_inputs_when_nothing_held_or_pressed() {
        let mut timer = Timer::new();
        let tick = timer.tick(no_input(), FRAME_MS);
        assert!(tick.inputs.is_empty());
    }

    // -------- horizontal shift (DAS / ARR) --------

    #[rstest]
    fn holding_left_shifts_once_immediately() {
        let mut timer = Timer::new();
        let state = InputState {
            move_left_held: true,
            ..no_input()
        };
        let tick = timer.tick(state, FRAME_MS);
        assert_eq!(tick.inputs, vec![Input::MoveLeft]);
    }

    #[rstest]
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

    #[rstest]
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

    #[rstest]
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

    #[rstest]
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

    #[rstest]
    fn das_blocks_repeat_within_window() {
        let mut timer = Timer::new();
        let state = InputState {
            move_left_held: true,
            ..no_input()
        };
        let _ = timer.tick(state, FRAME_MS); // first shift
        for _ in 0..5 {
            let tick = timer.tick(state, FRAME_MS);
            assert!(tick.inputs.is_empty());
        }
    }

    // -------- soft drop --------

    #[rstest]
    fn soft_drop_fires_at_sdf_cadence() {
        // SDF=15 → soft drop every ~67ms. With a 100ms frame delta,
        // we should fire exactly 1 soft drop per tick.
        let mut timer = Timer::new();
        let state = InputState {
            soft_drop_held: true,
            ..no_input()
        };
        // First tick: soft drop fires (100ms >= 67ms threshold).
        let tick = timer.tick(state, 100.0);
        assert!(tick.inputs.contains(&Input::SoftDrop));
        // Should fire ~1 drop per 100ms tick (since interval is 67ms).
        let tick = timer.tick(state, 100.0);
        assert!(tick.inputs.contains(&Input::SoftDrop));
    }

    #[rstest]
    fn releasing_soft_drop_resets_accumulator() {
        let mut timer = Timer::new();
        let held = InputState {
            soft_drop_held: true,
            ..no_input()
        };
        let _ = timer.tick(held, 100.0); // accumulate + fire
        // Released: accumulator should reset to 0 so re-press doesn't
        // immediately double-fire.
        let _ = timer.tick(no_input(), FRAME_MS);
        // Re-pressed: no drops accumulate at frame 1 since accumulator is 0.
        let tick = timer.tick(held, FRAME_MS);
        assert!(!tick.inputs.contains(&Input::SoftDrop));
    }

    #[rstest]
    fn soft_drop_emits_one_per_sdf_interval() {
        // SDF=15 → interval ≈67ms. With a 200ms frame delta, we should
        // emit ~3 soft drops (200 / 67 ≈ 2.99).
        let mut timer = Timer::new();
        let state = InputState {
            soft_drop_held: true,
            ..no_input()
        };
        let tick = timer.tick(state, 200.0);
        let drop_count = tick
            .inputs
            .iter()
            .filter(|i| **i == Input::SoftDrop)
            .count();
        assert!(
            (2..=3).contains(&drop_count),
            "expected 2-3 drops, got {drop_count}"
        );
    }
}
