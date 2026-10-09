//! Layout helper for rendering a single tetromino inside an N×N box.
//!
//! `PieceBox` computes the four physical-pixel rectangles that make up a
//! piece inside an N×N box (4×4 for hold / queue preview today). Pure
//! construction — no rendering, no macroquad dependency.
//!
//! ## Why a struct (not just a function)
//!
//! The previous helper returned only the local (col, row) coordinates
//! of the four cells; every caller then repeated the same
//! pixel-arithmetic (`origin + col * cell_size`, etc.) inline. The
//! struct carries `origin_phys` and `cell_size_phys`, so callers get
//! **absolute** [`PixelRect`]s via [`PieceBox::rects`] and a single
//! `outer()` for the container.
//!
//! ## Shape encoding
//!
//! The per-cell (col, row) layout within the box is taken from
//! `MinoType::coords(North)` and Y-flipped via
//! [`domain_to_screen_row`] (domain Y up → screen row down). The
//! output is identical, cell for cell, to the previous free helper
//! `piece_box_cells` in `macroquad.rs` — see the test module.

// Casts bounded by domain invariants (cell coords ∈ [0, 4)).
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use tet_domain::{MinoType, Orientation};

use crate::rendering::layout::PixelRect;
use crate::rendering::viewport::domain_to_screen_row;

/// Four-cell layout for one tetromino inside an N×N box (top-left
/// origin, screen-space coordinates).
#[derive(Clone, Copy, Debug)]
pub struct PieceBox {
    /// Physical-pixel top-left of the box.
    origin_phys: (i32, i32),
    /// Physical-pixel size of one cell within the box.
    cell_size_phys: i32,
    /// Number of cells per side of the box (4 for the live renderer).
    box_cells: i32,
    /// (col, `screen_row_within_box`) for each of the 4 cells, with the
    /// Y axis flipped so the piece's top tip appears at the top of
    /// the rendered box.
    cells: [(i32, i32); 4],
}

impl PieceBox {
    /// Build a `PieceBox` for `kind`.
    ///
    /// - `origin_phys` is the physical-pixel top-left of the box
    ///   (screen coordinates).
    /// - `cell_size_phys` is the physical-pixel size of one cell.
    /// - `box_cells` is the number of cells per side of the box
    ///   (always `4` for the current hold / queue preview rendering).
    #[must_use]
    pub fn new(
        kind: MinoType,
        origin_phys: (i32, i32),
        cell_size_phys: i32,
        box_cells: i32,
    ) -> Self {
        let coords = kind.coords(Orientation::North);
        let cells = [
            (
                i32::from(coords[0].x),
                domain_to_screen_row(i32::from(coords[0].y), box_cells),
            ),
            (
                i32::from(coords[1].x),
                domain_to_screen_row(i32::from(coords[1].y), box_cells),
            ),
            (
                i32::from(coords[2].x),
                domain_to_screen_row(i32::from(coords[2].y), box_cells),
            ),
            (
                i32::from(coords[3].x),
                domain_to_screen_row(i32::from(coords[3].y), box_cells),
            ),
        ];
        Self {
            origin_phys,
            cell_size_phys,
            box_cells,
            cells,
        }
    }

    /// The N×N box's outer rectangle in physical pixels.
    #[must_use]
    pub const fn outer(&self) -> PixelRect {
        let cs = self.cell_size_phys;
        let n = self.box_cells;
        PixelRect::new(self.origin_phys.0, self.origin_phys.1, cs * n, cs * n)
    }

    /// The four cell-level physical-pixel rectangles.
    ///
    /// Each `PixelRect` is positioned absolutely within the box — no
    /// further arithmetic is needed at the call site.
    #[must_use]
    pub fn rects(&self) -> [PixelRect; 4] {
        let mut out = [PixelRect::new(0, 0, 0, 0); 4];
        let ox = self.origin_phys.0;
        let oy = self.origin_phys.1;
        let cs = self.cell_size_phys;
        for (i, &(col, row)) in self.cells.iter().enumerate() {
            out[i] = PixelRect::new(ox + col * cs, oy + row * cs, cs, cs);
        }
        out
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

    // ---- `outer()` ----

    #[test]
    fn outer_is_full_box_at_origin() {
        let pb = PieceBox::new(MinoType::T, (100, 200), 24, 4);
        // 4 × 24 = 96×96 box at (100, 200).
        assert_eq!(pb.outer(), PixelRect::new(100, 200, 96, 96));
    }

    #[test]
    fn outer_scales_with_cell_size() {
        // 4 × 16 = 64×64.
        let pb = PieceBox::new(MinoType::T, (0, 0), 16, 4);
        assert_eq!(pb.outer(), PixelRect::new(0, 0, 64, 64));
    }

    // ---- `rects()` matches the per-mino shape ----
    //
    // These mirror the assertion table in `macroquad.rs`'s
    // `piece_box_specific_cells` test (which exercises the free
    // helper `piece_box_cells` directly). With `cell_size = 1` and
    // `origin = (0, 0)`, a local (col, row) maps to
    // `PixelRect(col, row, 1, 1)`. Byte-identical coverage.

    #[rstest]
    #[case::t_tip(MinoType::T, 3, 1, 1)] // T's (1, 2) → screen (1, 1)
    #[case::i_leftmost(MinoType::I, 0, 0, 1)] // I's (0, 2) → screen (0, 1)
    #[case::o_bottom_left(MinoType::O, 0, 1, 2)] // O's (1, 1) → screen (1, 2)
    #[case::o_top_right(MinoType::O, 3, 2, 1)] // O's (2, 2) → screen (2, 1)
    #[case::j_top_bump(MinoType::J, 3, 0, 1)] // J's (0, 2) → screen (0, 1)
    fn rects_match_legacy_piece_box_cells_per_mino(
        #[case] kind: MinoType,
        #[case] cell_idx: usize,
        #[case] expected_col: i32,
        #[case] expected_screen_row: i32,
    ) {
        let pb = PieceBox::new(kind, (0, 0), 1, 4);
        assert_eq!(
            pb.rects()[cell_idx],
            PixelRect::new(expected_col, expected_screen_row, 1, 1),
        );
    }

    // ---- `rects()` produces absolute pixel rects ----

    #[test]
    fn rects_are_offset_by_origin_phys() {
        // T's tip at local (1, 1). With origin (0, 0) the rect is at
        // (1, 1). With origin (100, 200) it's offset to (101, 201).
        let pb_at_zero = PieceBox::new(MinoType::T, (0, 0), 1, 4);
        let pb_offset = PieceBox::new(MinoType::T, (100, 200), 1, 4);
        assert_eq!(pb_at_zero.rects()[3].x, 1);
        assert_eq!(pb_offset.rects()[3].x, 101);
        assert_eq!(pb_offset.rects()[3].y, 201);
    }

    #[test]
    fn rects_scale_with_cell_size_phys() {
        // T's tip at local (1, 1), cell_size 24, origin (0, 0):
        //   rect = (1 × 24, 1 × 24, 24, 24)
        let pb = PieceBox::new(MinoType::T, (0, 0), 24, 4);
        assert_eq!(pb.rects()[3], PixelRect::new(24, 24, 24, 24));
    }

    #[test]
    fn rects_returns_exactly_4_rects() {
        for kind in [
            MinoType::T,
            MinoType::I,
            MinoType::O,
            MinoType::S,
            MinoType::Z,
            MinoType::J,
            MinoType::L,
        ] {
            let pb = PieceBox::new(kind, (0, 0), 1, 4);
            assert_eq!(pb.rects().len(), 4);
        }
    }

    // ---- shape sanity: no duplicates ----

    #[test]
    fn rects_have_four_distinct_cells_for_every_piece() {
        // Different MinoType shapes may have cells at the same local
        // (col, row) by coincidence — this test only guards against a
        // shape producing duplicate cells for a single piece.
        for kind in [
            MinoType::T,
            MinoType::I,
            MinoType::O,
            MinoType::S,
            MinoType::Z,
            MinoType::J,
            MinoType::L,
        ] {
            let pb = PieceBox::new(kind, (0, 0), 1, 4);
            let rects = pb.rects();
            for i in 0..4 {
                for j in (i + 1)..4 {
                    assert_ne!(
                        (rects[i].x, rects[i].y),
                        (rects[j].x, rects[j].y),
                        "{kind:?} cells {i} and {j} overlap",
                    );
                }
            }
        }
    }
}
