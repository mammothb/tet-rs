//! Stats line below the board (combo + b2b).
#![allow(clippy::cast_precision_loss)]

use crate::rendering::backend::canvas::{Canvas, FontWeight, PaletteKey};
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// Renders stats lines.
pub fn draw(vp: &Viewport, theme: &Theme, canvas: &mut dyn Canvas) {
    let layout = vp.layout();
    let ruleset = vp.ruleset();
    let (ox, oy) = (layout.origin.0, layout.origin.1);
    // Logical f32 throughout
    let stats_y = oy
        + layout.cell_px * ruleset.num_visible_rows as f32
        + theme.hud.font_size_px
        + theme.hud.label_padding_px;

    let stats_texts: [&str; 2] = [
        &format!("combo: {}", vp.snapshot().combo),
        &format!("b2b: {}", vp.snapshot().b2b),
    ];
    let mut y_offset = 0.0;
    for text in stats_texts {
        canvas.text(
            text,
            ox,
            stats_y + y_offset,
            FontWeight::Regular,
            PaletteKey::Text,
        );
        y_offset += theme.hud.font_size_px;
    }
}
