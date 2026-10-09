//! Hold panel — left of the playfield. Renders the held piece
//! (if any) inside a 4-cell-wide outlined box with a "HOLD" label.

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::font::Fonts;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// **Stub** — Phase 13 draws the outlined box at
/// `vp.origin_pixel().0 - (box_size + 4)`, the "HOLD" label above
/// it, then iterates `PieceBox::new(...)` cells for the held
/// piece (if any).
pub fn draw(_vp: &Viewport, _theme: &Theme, _fonts: &Fonts, _canvas: &mut dyn Canvas) {
    // No-op until Phase 13 fills the body in.
}
