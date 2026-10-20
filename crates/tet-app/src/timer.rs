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

use tet_application::Input;

use crate::settings::TimingSettings;

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

struct DirState {
    /// Accumulated ms since key was first pressed.
    das_accumulator_ms: f32,
    /// ARR accumulator (preserved while frozen).
    arr_accumulator_ms: f32,
    /// Whether DAS has already charged
    das_charged: bool,
}

impl DirState {
    fn new() -> Self {
        Self {
            das_accumulator_ms: 0.0,
            arr_accumulator_ms: 0.0,
            das_charged: false,
        }
    }
}

pub struct Timer {
    das_ms: f32,
    arr_ms: f32,
    sdf_interval_ms: f32,
    left: Option<DirState>,
    right: Option<DirState>,
    /// Which direction moved most recently (last key pressed).
    active: Option<Direction>,
    /// Carry-over ms when `SDF_FRAMES` doesn't divide cleanly into the frame delta.
    soft_drop_accumulator_ms: f32,
}

impl Timer {
    pub fn new(settings: &TimingSettings) -> Self {
        Self {
            das_ms: f32::from(settings.das),
            arr_ms: f32::from(settings.arr),
            sdf_interval_ms: f32::from(settings.sdf_interval),
            left: None,
            right: None,
            active: None,
            soft_drop_accumulator_ms: 0.0,
        }
    }

    /// One tick of the timer. Returns inputs to apply to the human.
    ///
    /// `frame_delta_ms`: time elapsed since the last call to `tick`. Used to
    /// accumulate partial ARR and soft-drop frames.
    pub fn tick(&mut self, state: InputState, frame_delta_ms: f32) -> TimerTick {
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
        let left_just_pressed = state.move_left_held && self.left.is_none();
        let right_just_pressed = state.move_right_held && self.right.is_none();

        if left_just_pressed {
            self.left = Some(DirState::new());
            inputs.push(Input::MoveLeft);
            self.active = Some(Direction::Left);
        }
        if right_just_pressed {
            self.right = Some(DirState::new());
            inputs.push(Input::MoveRight);
            self.active = Some(Direction::Right);
        }

        if !state.move_left_held {
            let was_active = self.active == Some(Direction::Left);
            self.left = None;
            if was_active {
                self.active = if self.right.is_some() {
                    Some(Direction::Right)
                } else {
                    None
                };
            }
        }
        if !state.move_right_held {
            let was_active = self.active == Some(Direction::Right);
            self.right = None;
            if was_active {
                self.active = if self.left.is_some() {
                    Some(Direction::Left)
                } else {
                    None
                };
            }
        }

        if let Some(dir) = self.active {
            let just_pressed = match dir {
                Direction::Left => left_just_pressed,
                Direction::Right => right_just_pressed,
            };
            if !just_pressed
                && let Some(dir_state) = match dir {
                    Direction::Left => self.left.as_mut(),
                    Direction::Right => self.right.as_mut(),
                }
            {
                if dir_state.das_charged {
                    dir_state.arr_accumulator_ms += frame_delta_ms;
                    while dir_state.arr_accumulator_ms >= self.arr_ms {
                        inputs.push(dir.into_input());
                        dir_state.arr_accumulator_ms -= self.arr_ms;
                    }
                } else {
                    dir_state.das_accumulator_ms += frame_delta_ms;
                    if dir_state.das_accumulator_ms >= self.das_ms {
                        dir_state.das_charged = true;
                        inputs.push(dir.into_input());
                    }
                }
            }
        }

        // 3. Soft drop.
        if state.soft_drop_held {
            self.soft_drop_accumulator_ms += frame_delta_ms;
            while self.soft_drop_accumulator_ms >= self.sdf_interval_ms {
                inputs.push(Input::SoftDrop);
                self.soft_drop_accumulator_ms -= self.sdf_interval_ms;
            }
        } else {
            self.soft_drop_accumulator_ms = 0.0;
        }

        TimerTick { inputs }
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

    use rstest::*;

    /// 60fps frame delta in ms.
    const FRAME_MS: f32 = 1000.0 / 60.0;

    /// Default state: nothing held, nothing pressed.
    fn no_input() -> InputState {
        InputState::default()
    }

    #[fixture]
    #[once]
    fn timing_settings() -> TimingSettings {
        TimingSettings::default()
    }

    // -------- edge-triggered inputs --------

    #[rstest]
    fn rotate_cw_passes_through_immediately(timing_settings: &TimingSettings) {
        let mut timer = Timer::new(timing_settings);
        let state = InputState {
            rotate_cw_pressed: true,
            ..no_input()
        };
        let tick = timer.tick(state, FRAME_MS);
        assert_eq!(tick.inputs, vec![Input::RotateCW]);
    }

    #[rstest]
    fn multiple_edge_inputs_emit_in_order(timing_settings: &TimingSettings) {
        let mut timer = Timer::new(timing_settings);
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
    fn no_inputs_when_nothing_held_or_pressed(timing_settings: &TimingSettings) {
        let mut timer = Timer::new(timing_settings);
        let tick = timer.tick(no_input(), FRAME_MS);
        assert!(tick.inputs.is_empty());
    }

    // -------- horizontal shift (DAS / ARR) --------

    #[rstest]
    fn holding_left_shifts_once_immediately(timing_settings: &TimingSettings) {
        let mut timer = Timer::new(timing_settings);
        let state = InputState {
            move_left_held: true,
            ..no_input()
        };
        let tick = timer.tick(state, FRAME_MS);
        assert_eq!(tick.inputs, vec![Input::MoveLeft]);
    }

    #[rstest]
    fn holding_left_does_not_repeat_during_das_delay(timing_settings: &TimingSettings) {
        // DAS = 16 frames ≈ 267ms. We're well within DAS at frame 2.
        let mut timer = Timer::new(timing_settings);
        let state = InputState {
            move_left_held: true,
            ..no_input()
        };
        let _ = timer.tick(state, FRAME_MS); // first shift
        let tick = timer.tick(state, FRAME_MS); // second frame, still in DAS
        assert!(tick.inputs.is_empty());
    }

    #[rstest]
    fn releasing_left_resets_das(timing_settings: &TimingSettings) {
        let mut timer = Timer::new(timing_settings);
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
    fn switching_direction_shifts_and_resets_das(timing_settings: &TimingSettings) {
        let mut timer = Timer::new(timing_settings);
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
    fn das_blocks_repeat_within_window(timing_settings: &TimingSettings) {
        let mut timer = Timer::new(timing_settings);
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

    // -------- simultaneous hold (last key wins with revert) --------

    #[rstest]
    fn both_pressed_same_frame_right_wins(timing_settings: &TimingSettings) {
        let mut timer = Timer::new(timing_settings);
        let state = InputState {
            move_left_held: true,
            move_right_held: true,
            ..no_input()
        };
        let tick = timer.tick(state, FRAME_MS);
        assert_eq!(tick.inputs, vec![Input::MoveLeft, Input::MoveRight]);
        let tick = timer.tick(state, FRAME_MS);
        assert!(tick.inputs.is_empty());
    }

    #[rstest]
    fn holding_left_then_pressing_right_shifts_right(timing_settings: &TimingSettings) {
        let mut timer = Timer::new(timing_settings);
        let left = InputState {
            move_left_held: true,
            ..no_input()
        };
        let both = InputState {
            move_left_held: true,
            move_right_held: true,
            ..no_input()
        };
        let _ = timer.tick(left, FRAME_MS);
        let tick = timer.tick(both, FRAME_MS);
        assert_eq!(tick.inputs, vec![Input::MoveRight]);
    }

    #[rstest]
    fn releasing_right_while_left_held_reverts_to_left(timing_settings: &TimingSettings) {
        let mut timer = Timer::new(timing_settings);
        let left = InputState {
            move_left_held: true,
            ..no_input()
        };
        let both = InputState {
            move_left_held: true,
            move_right_held: true,
            ..no_input()
        };
        let _ = timer.tick(left, FRAME_MS);
        let _ = timer.tick(both, FRAME_MS);
        let tick = timer.tick(left, FRAME_MS);
        assert!(tick.inputs.is_empty());
    }

    #[rstest]
    fn revert_to_left_preserves_das_charge(timing_settings: &TimingSettings) {
        let mut timer = Timer::new(timing_settings);
        let left = InputState {
            move_left_held: true,
            ..no_input()
        };
        let both = InputState {
            move_left_held: true,
            move_right_held: true,
            ..no_input()
        };
        let _ = timer.tick(left, FRAME_MS);
        let tick = timer.tick(left, 300.0);
        assert!(
            tick.inputs.contains(&Input::MoveLeft),
            "left should has DAS-charged and emitted a shift"
        );
        let _ = timer.tick(both, FRAME_MS);
        let tick = timer.tick(left, 20.0);
        let left_shifts = tick
            .inputs
            .iter()
            .filter(|i| **i == Input::MoveLeft)
            .count();
        assert!(
            left_shifts > 0,
            "left should resume ARR after revert, got {left_shifts} shifts"
        );
    }

    #[rstest]
    fn release_both_simultaneously_clears_all_state(timing_settings: &TimingSettings) {
        let mut timer = Timer::new(timing_settings);
        let both = InputState {
            move_left_held: true,
            move_right_held: true,
            ..no_input()
        };
        let _ = timer.tick(both, FRAME_MS);
        let tick = timer.tick(no_input(), FRAME_MS);
        assert!(tick.inputs.is_empty());

        let left = InputState {
            move_left_held: true,
            ..no_input()
        };
        let tick = timer.tick(left, FRAME_MS);
        assert_eq!(tick.inputs, vec![Input::MoveLeft]);
    }

    #[rstest]
    fn tap_right_while_holding_left_fires(timing_settings: &TimingSettings) {
        let mut timer = Timer::new(timing_settings);
        let left = InputState {
            move_left_held: true,
            ..no_input()
        };
        let both = InputState {
            move_left_held: true,
            move_right_held: true,
            ..no_input()
        };
        let _ = timer.tick(left, FRAME_MS);
        let tick = timer.tick(both, FRAME_MS);
        assert_eq!(tick.inputs, vec![Input::MoveRight]);

        let _ = timer.tick(left, FRAME_MS);
        let tick = timer.tick(both, FRAME_MS);
        assert_eq!(tick.inputs, vec![Input::MoveRight]);
    }

    #[rstest]
    fn soft_drop_still_fires_during_both_held(timing_settings: &TimingSettings) {
        let mut timer = Timer::new(timing_settings);
        let state = InputState {
            move_left_held: true,
            move_right_held: true,
            soft_drop_held: true,
            ..no_input()
        };
        let _ = timer.tick(state, FRAME_MS);
        let tick = timer.tick(state, 100.0);
        assert!(
            tick.inputs.contains(&Input::SoftDrop),
            "soft drop should fire even with both directions held"
        );
    }

    // -------- soft drop --------

    #[rstest]
    fn soft_drop_fires_at_sdf_cadence(timing_settings: &TimingSettings) {
        // SDF=15 → soft drop every ~67ms. With a 100ms frame delta,
        // we should fire exactly 1 soft drop per tick.
        let mut timer = Timer::new(timing_settings);
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
    fn releasing_soft_drop_resets_accumulator(timing_settings: &TimingSettings) {
        let mut timer = Timer::new(timing_settings);
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
    fn soft_drop_emits_one_per_sdf_interval(timing_settings: &TimingSettings) {
        // SDF=15 → interval ≈67ms. With a 200ms frame delta, we should
        // emit ~3 soft drops (200 / 67 ≈ 2.99).
        let mut timer = Timer::new(timing_settings);
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
