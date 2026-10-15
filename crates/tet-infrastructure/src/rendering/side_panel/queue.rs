//! Queue preview panel — right of the playfield. Renders up to
//! `Ruleset::num_preview` pieces, vertically stacked.

use tet_domain::Cell;

use crate::rendering::backend::canvas::{Canvas, FontWeight, PaletteKey};
use crate::rendering::layout::PixelRect;
use crate::rendering::side_panel::for_each_piece_cell;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// Render the queue preview column: "NEXT" label + up to
/// `ruleset.num_preview` boxes stacked vertically to the right of
/// the board. Queue cells are sized independently of the board's
/// `cell_px` (see `theme.hud.queue_preview_cell_px`).
pub fn draw(vp: &Viewport, theme: &Theme, canvas: &mut dyn Canvas) {
    let layout = vp.layout();
    let ruleset = vp.ruleset();
    let (ox, oy) = vp.origin_pixel();
    let cw = vp.cell_px_phys();
    let cell_phys = (theme.hud.queue_preview_cell_px * vp.dpi()).round() as i32;
    let box_phys = cell_phys * theme.hud.hold_box_cells;
    let bw = cw * ruleset.num_cols as i32;
    let spacing_phys = (theme.hud.queue_preview_spacing_px * vp.dpi()).round() as i32;
    let qx_phys = ox + bw + spacing_phys;
    let qy_phys = oy;

    // NEXT label: logical f32, same convention as the HOLD label.
    canvas.text(
        "NEXT",
        layout.origin.0 + layout.cell_px * ruleset.num_cols as f32,
        (qy_phys as f32) - theme.hud.label_padding_px,
        FontWeight::SemiBold,
        PaletteKey::Text,
    );

    let preview_count = ruleset.num_preview.min(vp.snapshot().queue.len());
    for i in 0..preview_count {
        let kind = vp.snapshot().queue[i];
        let qy_box_phys = qy_phys + i as i32 * (box_phys + spacing_phys);

        // Box outline — integer physical pixels.
        canvas.stroke_rect(
            PixelRect::new(qx_phys, qy_box_phys, box_phys, box_phys),
            PaletteKey::Structure,
        );

        let piece_cell = cell_phys;
        for_each_piece_cell(
            kind,
            (qx_phys, qy_box_phys),
            piece_cell,
            theme.hud.hold_box_cells,
            |rect| {
                canvas.draw_cell_sprite(rect, Cell::Block(kind), 255);
            },
        );
    }
}
