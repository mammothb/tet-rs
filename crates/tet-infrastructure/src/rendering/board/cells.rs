//! Locked cells on the playfield.

// Casts bounded by domain invariants (cols/visible_rows ≤ 128).
#![allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]

use tet_domain::Cell;

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::layout::PixelRect;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// Iterate `vp.snapshot().board.rows()` (bottom-to-top, Y-flipped
/// via `viewport.cell_pixel`) and fill each non-empty cell with the
/// appropriate palette. Buffer rows above the visible window are
/// skipped via `.take(...)`.
pub fn draw_locked(vp: &Viewport, _theme: &Theme, canvas: &mut dyn Canvas) {
    let cw = vp.cell_px_phys();
    let visible_rows = vp.ruleset().num_visible_rows;
    for (row, board_row) in vp.snapshot().board.rows().enumerate().take(visible_rows) {
        for (col, cell) in board_row.iter().enumerate() {
            let (x_phys, y_phys) = vp.cell_pixel(col as i32, row as i32);
            let rect = PixelRect::new(x_phys, y_phys, cw, cw);
            match cell {
                Cell::Empty => {}
                _ => canvas.draw_cell_sprite(rect, *cell, 255),
            }
        }
    }
}
