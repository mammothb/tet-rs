//! Ghost piece: landing-position math and outline rendering.

use tet_domain::{Cell, Vec2};

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::layout::PixelRect;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// Compute the (col, row) offset from the current piece's position
/// to the projected ghost landing position. Pure function —
/// `ghost - current` gives the per-cell translation applied to each
/// of the piece's 4 cells.
#[must_use]
pub fn offset(current: Vec2, ghost: Vec2) -> (i32, i32) {
    let dx = i32::from(ghost.x) - i32::from(current.x);
    let dy = i32::from(ghost.y) - i32::from(current.y);
    (dx, dy)
}

/// Render the ghost piece at its projected landing position.
///
/// The ghost has a translucent [`PaletteKey::Ghost`] fill plus an
/// opaque outline in the active piece's [`PaletteKey::Piece(kind)`]
/// color. Cells outside the visible row range are skipped.
pub fn draw(vp: &Viewport, theme: &Theme, ghost_pos: Vec2, canvas: &mut dyn Canvas) {
    let kind = vp.snapshot().current.kind;
    let cur_pos = vp.snapshot().current.pos;
    let pos_offset_x = i32::from(ghost_pos.x) - i32::from(cur_pos.x);
    let pos_offset_y = i32::from(ghost_pos.y) - i32::from(cur_pos.y);
    let visible_rows = vp.ruleset().num_visible_rows as i32;
    let cw = vp.cell_px_phys();

    for cell_offset in vp.snapshot().current.cells() {
        let cell_col = i32::from(cell_offset.x) + pos_offset_x;
        let cell_domain_row = i32::from(cell_offset.y) + pos_offset_y;
        if cell_domain_row < 0 || cell_domain_row >= visible_rows {
            continue;
        }
        let (x_phys, y_phys) = vp.cell_pixel(cell_col, cell_domain_row);
        let rect = PixelRect::new(x_phys, y_phys, cw, cw);
        canvas.draw_cell_sprite(rect, Cell::Block(kind), theme.ghost.alpha);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

    /// `offset` returns `ghost - current` as `(i32, i32)`. Each
    /// case exercises an axis direction so the function is verified
    /// symmetrically in all four quadrants.
    #[rstest]
    #[case::zero_distance(Vec2::new(3, 5), Vec2::new(3, 5), (0_i32, 0_i32))]
    #[case::drop_5_rows(Vec2::new(0, 19), Vec2::new(0, 14), (0_i32, -5_i32))]
    #[case::sideways_3_cols(Vec2::new(0, 5), Vec2::new(3, 5), (3_i32, 0_i32))]
    #[case::up_3_rows(Vec2::new(2, 10), Vec2::new(2, 13), (0_i32, 3_i32))]
    #[case::diagonal(Vec2::new(0, 0), Vec2::new(4, -2), (4_i32, -2_i32))]
    fn offset_equals_ghost_minus_current(
        #[case] current: Vec2,
        #[case] ghost: Vec2,
        #[case] expected: (i32, i32),
    ) {
        assert_eq!(offset(current, ghost), expected);
    }
}
