use serde::Deserialize;
use tet_application::{ARR_MS, DAS_MS, SDF_INTERVAL_MS};

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub controls: ControlSettings,
    pub timing: TimingSettings,
    pub display: DisplaySettings,
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
            das: DAS_MS,
            arr: ARR_MS,
            sdf_interval: SDF_INTERVAL_MS,
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct DisplaySettings {
    pub grid_style: String,
    pub ghost_alpha: u8,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            grid_style: "full".to_string(),
            ghost_alpha: 128,
        }
    }
}
