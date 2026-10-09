//! Stats line below the board (combo + b2b).

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::font::Fonts;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// **Stub** — Phase 14 builds a list of lines
/// (`format!("combo: {}", combo)`, `format!("b2b: {}", b2b)`) and
/// draws each via `canvas.text(line, x, y, FontWeight::Regular,
/// PaletteKey::Text)`. Logical f32 coords (matches legacy).
pub fn draw(_vp: &Viewport, _theme: &Theme, _fonts: &Fonts, _canvas: &mut dyn Canvas) {
    // No-op until Phase 14 fills the body in.
}
