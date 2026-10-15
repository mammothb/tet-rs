//! Hold panel — left of the playfield. Renders the held piece
//! (if any) inside a `theme.hud.hold_box_cells`-wide outlined box
//! with a "HOLD" label.

use tet_domain::Cell;

use crate::rendering::backend::canvas::{Canvas, FontWeight, PaletteKey};
use crate::rendering::layout::PixelRect;
use crate::rendering::side_panel::for_each_piece_cell;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// Render the hold panel: outlined box + "HOLD" label + (if the
/// player is holding a piece) the held piece's 4 cells.
pub fn draw(vp: &Viewport, theme: &Theme, canvas: &mut dyn Canvas) {
    let layout = vp.layout();
    let (ox, oy) = vp.origin_pixel();
    let cw = vp.cell_px_phys();
    let box_size = cw * theme.hud.hold_box_cells;
    let hold_x_phys = ox - (box_size + 4);
    let hold_y_phys = oy;

    // Box outline — integer physical pixels.
    canvas.stroke_rect(
        PixelRect::new(hold_x_phys, hold_y_phys, box_size, box_size),
        PaletteKey::Structure,
    );

    // Label sits above the box. Logical f32 coords (matches the
    // legacy `draw_label` convention — the canvas accepts logical
    // positions for text).
    canvas.text(
        "HOLD",
        layout.origin.0 - (box_size + 4) as f32,
        (hold_y_phys as f32) - theme.hud.label_padding_px,
        FontWeight::SemiBold,
        PaletteKey::Text,
    );

    // No held piece yet — outline + label is the entire output.
    let Some(kind) = vp.snapshot().hold else {
        return;
    };

    let piece_cell = box_size / theme.hud.hold_box_cells;
    for_each_piece_cell(
        kind,
        (hold_x_phys, hold_y_phys),
        piece_cell,
        theme.hud.hold_box_cells,
        |rect| {
            canvas.draw_cell_sprite(rect, Cell::Block(kind));
        },
    );
}
