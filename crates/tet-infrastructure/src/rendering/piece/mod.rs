//! Active piece + ghost rendering.

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;
use tet_domain::Vec2;

pub mod ghost;

/// Render the active tetromino. If `ghost` is `Some`, also render
/// the ghost piece at the projected landing position.
///
/// **Stub** — Phase 12 draws the active piece via
/// `viewport.cell_pixel` and delegates ghost drawing to
/// `ghost::draw`.
pub fn draw(_vp: &Viewport, _theme: &Theme, _ghost: Option<Vec2>, _canvas: &mut dyn Canvas) {
    // No-op until Phase 12 fills the body in.
}
