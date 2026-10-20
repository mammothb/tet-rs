use macroquad::input::{KeyCode, is_key_down, is_key_pressed};

use crate::{settings::ControlSettings, timer::InputState};

pub struct ControlKeys {
    pub move_left: KeyCode,
    pub move_right: KeyCode,
    pub soft_drop: KeyCode,
    pub hard_drop: KeyCode,
    pub rotate_cw: KeyCode,
    pub rotate_ccw: KeyCode,
    pub hold: KeyCode,
}

impl ControlKeys {
    pub fn from_settings(settings: &ControlSettings) -> Result<Self, String> {
        Ok(Self {
            move_left: parse_key(&settings.move_left)?,
            move_right: parse_key(&settings.move_right)?,
            soft_drop: parse_key(&settings.soft_drop)?,
            hard_drop: parse_key(&settings.hard_drop)?,
            rotate_cw: parse_key(&settings.rotate_cw)?,
            rotate_ccw: parse_key(&settings.rotate_ccw)?,
            hold: parse_key(&settings.hold)?,
        })
    }
}

/// Read the keyboard state and produce an `InputState` for the timer.
pub fn collect_input_state(keys: &ControlKeys) -> InputState {
    InputState {
        move_left_held: is_key_down(keys.move_left),
        move_right_held: is_key_down(keys.move_right),
        soft_drop_held: is_key_down(keys.soft_drop),
        hard_drop_pressed: is_key_pressed(keys.hard_drop),
        rotate_cw_pressed: is_key_pressed(keys.rotate_cw),
        rotate_ccw_pressed: is_key_pressed(keys.rotate_ccw),
        hold_pressed: is_key_pressed(keys.hold),
    }
}

fn parse_key(s: &str) -> Result<KeyCode, String> {
    match s {
        "Down" => Ok(KeyCode::Down),
        "Left" => Ok(KeyCode::Left),
        "Right" => Ok(KeyCode::Right),
        "Space" => Ok(KeyCode::Space),
        "D" => Ok(KeyCode::D),
        "F" => Ok(KeyCode::F),
        "S" => Ok(KeyCode::S),
        _ => Err(format!("unknown key: {s}")),
    }
}
