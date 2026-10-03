//! Font assets for the renderer. Loaded once at startup via
//! `include_bytes!` so the binary is self-contained — no runtime file
//! loading.

use macroquad::prelude::Font;
use macroquad::text::load_ttf_font_from_bytes;

/// Regular weight Fira Code (400). Used for general HUD text.
const FIRA_REGULAR: &[u8] = include_bytes!("../../../../assets/fonts/FiraCode-Regular.ttf");

/// `SemiBold` weight Fira Code (600). Used for prominent labels
/// (`HOLD`, `NEXT`, player names).
const FIRA_SEMIBOLD: &[u8] = include_bytes!("../../../../assets/fonts/FiraCode-SemiBold.ttf");

/// Bold weight Fira Code (700). Reserved for future use (high-priority
/// indicators like game-over overlays).
#[allow(dead_code)]
const FIRA_BOLD: &[u8] = include_bytes!("../../../../assets/fonts/FiraCode-Bold.ttf");

/// Loaded font handles. The renderer holds one of these and passes the
/// appropriate weight to each `draw_text_ex` call.
#[derive(Clone)]
pub struct Fonts {
    pub regular: Font,
    pub semibold: Font,
}

impl Fonts {
    /// Load fonts from the embedded bytes.
    ///
    /// # Errors
    ///
    /// Returns `macroquad::Error` if either TTF is malformed. The shipped
    /// bytes are well-formed so this should not happen in practice.
    pub fn load() -> Result<Self, macroquad::Error> {
        Ok(Self {
            regular: load_ttf_font_from_bytes(FIRA_REGULAR)?,
            semibold: load_ttf_font_from_bytes(FIRA_SEMIBOLD)?,
        })
    }
}
