//! Ghost piece: landing-position math and outline rendering.

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;
use tet_domain::Vec2;

/// Compute the (col, row) offset from the current piece's
/// position to the projected ghost landing position. Pure function
/// — exposed for testing and for any future caller that wants the
/// offset without rendering.
///
/// **Stub** — Phase 12 returns
/// `(ghost.x - current.x, ghost.y - current.y)`.
#[must_use]
pub fn offset(_current: Vec2, _ghost: Vec2) -> (i32, i32) {
    // No-op until Phase 12 fills the body in.
    (0, 0)
}

/// Render the ghost piece at its projected landing position.
///
/// **Stub** — Phase 12 fills each cell using the offset above and
/// a `PieceBox`-shaped pixel-rect loop, drawing a translucent fill
/// plus an opaque outline.
pub fn draw(_vp: &Viewport, _theme: &Theme, _ghost: Vec2, _canvas: &mut dyn Canvas) {
    // No-op until Phase 12 fills the body in.
}
