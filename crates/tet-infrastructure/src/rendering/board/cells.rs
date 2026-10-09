//! Locked cells on the playfield.

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// Iterate `vp.snapshot().board.rows()` (bottom-to-top) and fill
/// each non-empty cell with the appropriate `PaletteKey`.
///
/// **Stub** — Phase 11 iterates via `viewport.cell_pixel(col, row)`
/// for top-left pixel coords and calls
/// `canvas.fill_rect(rect, palette_for(cell))`.
pub fn draw_locked(_vp: &Viewport, _theme: &Theme, _canvas: &mut dyn Canvas) {
    // No-op until Phase 11 fills the body in.
}
