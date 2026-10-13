//! Outer rectangle outline of the playfield.

use crate::rendering::backend::canvas::{Canvas, PaletteKey};
use crate::rendering::layout::plan_board;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// Stroke the 4 edges of the board's outer rectangle. Drawn LAST by
/// `board::draw` so the border overwrites the faint grid line color
/// at the rect edges with the brighter structure color.
pub fn draw_outer(vp: &Viewport, _theme: &Theme, canvas: &mut dyn Canvas) {
    let plan = plan_board(vp.layout(), vp.ruleset(), vp.dpi());
    canvas.stroke_rect(plan.outer, PaletteKey::Structure);
}
