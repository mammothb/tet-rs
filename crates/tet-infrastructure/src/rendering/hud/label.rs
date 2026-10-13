//! Player label above the queue column (e.g. "human", "bot", or
//! a TBP-supplied name).
#![allow(clippy::cast_precision_loss)]

use crate::rendering::backend::canvas::{Canvas, FontWeight, PaletteKey};
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// Returns early if `view.label.is_empty()`, else draws `view.label` at the
/// queue column x-offset and the board's bottom-row y via `canvas.text`.
pub fn draw(vp: &Viewport, theme: &Theme, label: &str, canvas: &mut dyn Canvas) {
    if label.is_empty() {
        return;
    }
    let layout = vp.layout();
    let ruleset = vp.ruleset();
    let (ox, oy) = (layout.origin.0, layout.origin.1);
    let queue_x =
        ox + layout.cell_px * ruleset.num_cols as f32 + theme.hud.queue_preview_spacing_px;
    // Logical f32 — matches existing label-rendering convention.
    let label_y =
        oy + layout.cell_px * ruleset.num_visible_rows as f32 - theme.hud.label_padding_px;

    canvas.text(
        label,
        queue_x,
        label_y,
        FontWeight::SemiBold,
        PaletteKey::Text,
    );
}
