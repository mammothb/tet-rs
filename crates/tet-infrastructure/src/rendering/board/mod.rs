//! Board rendering — outline, locked cells, grid lines.

pub mod cells;
pub mod grid;
pub mod outline;

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// Draw the playfield in this order (matches the previous inline
/// `render_board`):
///
/// 1. Faint grid base lines + corner-cross gradient
///    ([`grid::draw`]).
/// 2. Locked cells ([`cells::draw_locked`]).
/// 3. Outer outline **last** ([`outline::draw_outer`]) — so the
///    border overwrites the faint grid color at the rect edges with
///    the brighter structure color.
pub fn draw(vp: &Viewport, theme: &Theme, canvas: &mut dyn Canvas) {
    grid::draw(vp, theme, canvas);
    cells::draw_locked(vp, theme, canvas);
    outline::draw_outer(vp, theme, canvas);
}
