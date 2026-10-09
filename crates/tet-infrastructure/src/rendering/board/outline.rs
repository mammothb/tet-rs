//! Outer rectangle outline of the playfield.

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// Stroke the 4 edges of the board's outer rectangle.
///
/// **Stub** — Phase 11 calls `canvas.stroke_rect(plan.outer,
/// PaletteKey::Structure)`. The outline is drawn LAST by
/// `board::draw` so the border overwrites the faint grid line color
/// at the rect edges with the brighter structure color.
pub fn draw_outer(_vp: &Viewport, _theme: &Theme, _canvas: &mut dyn Canvas) {
    // No-op until Phase 11 fills the body in.
}
