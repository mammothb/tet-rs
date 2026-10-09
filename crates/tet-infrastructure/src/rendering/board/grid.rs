//! Grid lines (faint base + corner-cross gradient).

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// Draw the grid for the current `vp.layout().grid_style`.
/// Faint base lines first, then the alpha-layered "cross"
/// segments at intersections that fade from dim to bright toward
/// each corner pixel.
///
/// **Stub** — Phase 11 iterates `plan_board` outputs and calls
/// `canvas.stroke_h_line` / `canvas.stroke_v_line` /
/// `canvas.stroke_rect` for each layer. The corner-cross gradient
/// should live as a separate inner helper for testability.
pub fn draw(_vp: &Viewport, _theme: &Theme, _canvas: &mut dyn Canvas) {
    // No-op until Phase 11 fills the body in.
}
