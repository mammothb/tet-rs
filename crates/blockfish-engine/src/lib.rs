mod common;
mod config;
mod finesse;
mod matrix;
mod place;
mod shape;

pub mod ai;

pub use common::{Color, Input, Orientation};
pub use config::{Config, Parameters};
pub use matrix::BasicMatrix;

/// Returns Blockfish's build version string.
pub fn version() -> &'static str {
    "DEV_BUILD"
}
