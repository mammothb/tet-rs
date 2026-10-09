//! Queue preview panel — right of the playfield. Renders up to
//! `Ruleset::num_preview` pieces, vertically stacked.

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::font::Fonts;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// **Stub** — Phase 13 draws each queue box at `(bx + bw + spacing,
/// by + i * (box + spacing))` with a `PieceBox::new(...)` per cell,
/// and the "NEXT" label above the column.
pub fn draw(_vp: &Viewport, _theme: &Theme, _fonts: &Fonts, _canvas: &mut dyn Canvas) {
    // No-op until Phase 13 fills the body in.
}
