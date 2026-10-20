use std::path::Path;

use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub controls: ControlSettings,
    pub timing: TimingSettings,
    pub display: DisplaySettings,
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        match fs_err::read_to_string(path) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|e| {
                eprintln!("warning: bad settings.toml: {e}, using defaults");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct ControlSettings {
    pub move_left: String,
    pub move_right: String,
    pub soft_drop: String,
    pub hard_drop: String,
    pub rotate_cw: String,
    pub rotate_ccw: String,
    pub hold: String,
}

impl Default for ControlSettings {
    fn default() -> Self {
        Self {
            move_left: "Left".to_string(),
            move_right: "Right".to_string(),
            soft_drop: "Down".to_string(),
            hard_drop: "Space".to_string(),
            rotate_cw: "F".to_string(),
            rotate_ccw: "D".to_string(),
            hold: "S".to_string(),
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct TimingSettings {
    pub das: u16,
    pub arr: u16,
    pub sdf_interval: u16,
}

impl Default for TimingSettings {
    fn default() -> Self {
        Self {
            das: 267,
            arr: 17,
            sdf_interval: 67,
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct DisplaySettings {
    pub grid_style: String,
    pub ghost_alpha: u8,
    pub skin: Option<String>,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            grid_style: "full".to_string(),
            ghost_alpha: 128,
            skin: Some("default".to_string()),
        }
    }
}
