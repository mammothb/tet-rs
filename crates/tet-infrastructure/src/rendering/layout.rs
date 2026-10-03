//! Layout planning for the renderer.
//!
//! Every pixel position the renderer draws is computed here, in **integer
//! physical pixels**, before any draw call. The renderer just iterates the
//! planned rectangles and lines — it does no math of its own.
//!
//! ## Why a separate plan
//!
//! Trying to compute pixel positions inside the draw loop led to a long
//! string of subpixel-coordinate bugs (half-pixel offsets, fractional
//! endpoints, mismatched integer/float math). Computing the plan once,
//! up front, in integer pixels, eliminates the entire category.
//!
//! ## What gets planned
//!
//! For each rendered element, the plan returns:
//!
//! - **Outer outline**: 4 line segments (top, bottom, left, right). The
//!   outer rect's width and height are `cols * cell_px` and
//!   `visible_rows * cell_px` — never derived from "where the last grid
//!   line happens to land".
//! - **Horizontal grid lines**: one per *row boundary*. With
//!   `visible_rows` rows, there are `visible_rows + 1` row boundaries
//!   (top of row 0, between rows 0/1, ..., bottom of last row). Every
//!   one is a horizontal line at integer pixel row `y_phys`.
//! - **Vertical grid lines**: one per *column boundary*. With `cols`
//!   columns, there are `cols + 1` column boundaries (left of col 0,
//!   between cols 0/1, ..., right of last col). Every one is a vertical
//!   line at integer pixel column `x_phys`.
//!
//! The grid lines include the BORDERS (not just internal divisions). The
//! leftmost grid line coincides with the outer rect's left edge, the
//! rightmost with the right edge, etc. — so the grid and the outline
//! line up by construction, not by accident.

// Casts bounded by domain invariants (cols/rows/cell_px ≤ 128, dpi ≤ 4).
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]

use tet_application::{Layout, PlayerView};
use tet_domain::Ruleset;

/// Integer-pixel rectangle. All four are physical pixel positions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl PixelRect {
    #[must_use]
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    /// Inclusive bottom edge pixel row of the rect. The last pixel INSIDE
    /// the rect is at `y + h - 1`.
    #[must_use]
    pub const fn bottom_pixel(&self) -> i32 {
        self.y + self.h - 1
    }

    /// Inclusive right edge pixel column. Last pixel INSIDE is at
    /// `x + w - 1`.
    #[must_use]
    pub const fn right_pixel(&self) -> i32 {
        self.x + self.w - 1
    }
}

/// Plan for a board (the main playfield + grid).
#[derive(Clone, Debug)]
pub struct BoardLayout {
    /// The outer rectangle. All four edges are integer pixel positions.
    pub outer: PixelRect,
    /// One horizontal grid line per row boundary. Length is
    /// `visible_rows + 1`. Each entry is the pixel ROW the line sits on.
    /// The outer rect's top edge is at `outer.y` and bottom edge at
    /// `outer.bottom_pixel()`; both must appear in this list (positions 0
    /// and last).
    pub h_line_rows: Vec<i32>,
    /// One vertical grid line per column boundary. Length is `cols + 1`.
    /// Each entry is the pixel COLUMN the line sits on. Includes the
    /// leftmost and rightmost columns of the outer rect.
    pub v_line_cols: Vec<i32>,
    /// Width of each grid line segment along the rect's length.
    pub grid_line_length_phys: i32,
}

/// Plan for a 4-sided outlined box (hold, queue preview, ghost cell).
#[derive(Clone, Copy, Debug)]
pub struct BoxLayout {
    pub rect: PixelRect,
}

/// Plan for a filled cell (locked piece on the board).
#[derive(Clone, Copy, Debug)]
pub struct FilledCell {
    pub rect: PixelRect,
}

/// Plan a board's outline + grid lines for one view.
///
/// All positions are integer physical pixels. `cell_px * dpi` is rounded
/// to the nearest integer pixel; `origin.x * dpi` and `origin.y * dpi` are
/// rounded similarly. The outer rect's width is `cols * cell_px_phys`
/// (NOT "where the last grid line lands"), so the outline and the grid
/// line up by construction.
#[must_use]
pub fn plan_board(view: &PlayerView, ruleset: &Ruleset, dpi: f32) -> BoardLayout {
    let cell_px_phys = (view.layout.cell_px * dpi).round() as i32;
    let ox = (view.layout.origin.0 * dpi).round() as i32;
    let oy = (view.layout.origin.1 * dpi).round() as i32;
    #[allow(clippy::cast_possible_truncation)] // num_cols ≤ 128 by invariant
    let cols = ruleset.num_cols as i32;
    #[allow(clippy::cast_possible_truncation)] // num_visible_rows ≤ 128 by invariant
    let visible_rows = ruleset.num_visible_rows as i32;

    let outer = PixelRect::new(ox, oy, cell_px_phys * cols, cell_px_phys * visible_rows);

    // Horizontal grid lines: one per ROW BOUNDARY. visible_rows rows have
    // visible_rows + 1 boundaries (top of row 0, top of row 1, ...,
    // top of row 19, BOTTOM of row 19). The first is at `oy` (rect top edge);
    // the last is at `oy + h - 1` (rect bottom edge). The intermediate ones
    // are at `oy + row * cell_px_phys` for row in 0..visible_rows.
    let mut h_line_rows: Vec<i32> = (0..visible_rows)
        .map(|row| oy + row * cell_px_phys)
        .collect();
    h_line_rows.push(oy + cell_px_phys * visible_rows - 1);

    // Vertical grid lines: one per COLUMN BOUNDARY. Same idea — last entry
    // is at the rect's right edge (inclusive), not one past it.
    let mut v_line_cols: Vec<i32> = (0..cols).map(|col| ox + col * cell_px_phys).collect();
    v_line_cols.push(ox + cell_px_phys * cols - 1);

    BoardLayout {
        outer,
        h_line_rows,
        v_line_cols,
        grid_line_length_phys: outer.w,
    }
}

/// Plan a 4-sided outlined box at `pos` with `size_phys` × `size_phys`
/// pixels. Used for hold box, queue preview boxes, ghost cells.
#[must_use]
pub fn plan_box(pos: (f32, f32), size_phys: i32, dpi: f32) -> BoxLayout {
    let x = (pos.0 * dpi).round() as i32;
    let y = (pos.1 * dpi).round() as i32;
    BoxLayout {
        rect: PixelRect::new(x, y, size_phys, size_phys),
    }
}

/// Plan a filled cell at `pos` with the given cell size in physical pixels.
#[must_use]
pub fn plan_filled_cell(pos: (f32, f32), size_phys: i32, dpi: f32) -> FilledCell {
    let x = (pos.0 * dpi).round() as i32;
    let y = (pos.1 * dpi).round() as i32;
    FilledCell {
        rect: PixelRect::new(x, y, size_phys, size_phys),
    }
}

/// Compute the screen pixel position (top-left of cell) for a logical
/// `(col, row)` cell, given the board's `origin` and `cell_px`.
#[must_use]
pub fn cell_origin(layout: &Layout, col: i32, row: i32, dpi: f32) -> (f32, f32) {
    let x = (layout.origin.0 + (col) as f32 * layout.cell_px) * dpi;
    let y = (layout.origin.1 + (row) as f32 * layout.cell_px) * dpi;
    (x / dpi, y / dpi)
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod test {
    use super::*;
    use rstest::{fixture, rstest};
    use tet_application::Layout;
    use tet_domain::Ruleset;

    /// Test DPI helpers. All return `f32`, so they can't be `#[fixture]`
    /// functions (multiple `f32`-returning fixtures are ambiguous in rstest).
    /// Tests call them explicitly: `dpi1()`, `dpi2()`, `dpi_1_5()`.
    fn dpi1() -> f32 {
        1.0
    }

    fn dpi2() -> f32 {
        2.0
    }

    fn dpi_1_5() -> f32 {
        1.5
    }

    /// Standard guideline ruleset: 10 cols, 25 total rows, 20 visible.
    #[fixture]
    fn ruleset() -> Ruleset {
        Ruleset::guideline()
    }

    /// Standard 24px layout with origin at (116, 16) — same as the live
    /// tet-app composition root.
    #[fixture]
    fn layout() -> Layout {
        Layout {
            origin: (116.0, 16.0),
            cell_px: 24.0,
            ..Layout::default()
        }
    }

    /// `PlayerView` with a fresh T-piece on an empty board. Combines
    /// `ruleset` and `layout` fixtures so the same instances are passed
    /// to any test that takes `view`.
    #[fixture]
    fn view(ruleset: Ruleset, layout: Layout) -> PlayerView {
        PlayerView {
            snapshot: tet_application::PlayerSnapshot {
                board: tet_domain::Board::new(
                    u8::try_from(ruleset.num_cols).unwrap(),
                    u8::try_from(ruleset.num_rows).unwrap(),
                ),
                queue: vec![],
                hold: None,
                combo: 0,
                b2b: false,
                current: tet_application::Piece::spawn(tet_domain::MinoType::T),
                phase: tet_application::Phase::Playing,
                pending_garbage: vec![],
            },
            label: "",
            layout,
            ghost: None,
        }
    }

    #[rstest]
    fn board_outer_uses_cols_times_cell_px_not_last_grid_line(view: PlayerView, ruleset: Ruleset) {
        // The bug we just fixed: width was computed from "where the last
        // grid line lands" instead of `cols * cell_px`. That left the
        // rightmost column boundary uncovered.
        let plan = plan_board(&view, &ruleset, dpi1());

        // 10 × 24 = 240. NOT 332 + 1 or any "last grid line" calculation.
        assert_eq!(plan.outer.w, 240);
        // 20 × 24 = 480
        assert_eq!(plan.outer.h, 480);
        assert_eq!(plan.outer.x, 116);
        assert_eq!(plan.outer.y, 16);
    }

    #[rstest]
    fn h_line_rows_include_outer_top_and_bottom(view: PlayerView, ruleset: Ruleset) {
        let plan = plan_board(&view, &ruleset, dpi1());

        // 20 visible rows → 21 row boundaries: top of row 0 through top of
        // row 19, plus the BOTTOM pixel row of row 19. The bottom pixel row
        // is `oy + h - 1` (the inclusive rect bottom), NOT `oy + h`.
        assert_eq!(plan.h_line_rows.len(), 21);
        // First row boundary is the outer rect's TOP edge
        assert_eq!(plan.h_line_rows[0], plan.outer.y);
        // Last row boundary is the outer rect's BOTTOM edge (inclusive pixel)
        assert_eq!(plan.h_line_rows[20], plan.outer.bottom_pixel());
        assert_eq!(plan.h_line_rows[20], 495);
        // Evenly spaced by cell_px for the first 20 entries
        for (i, expected) in (0..20).map(|i| 16 + i * 24).enumerate() {
            assert_eq!(plan.h_line_rows[i], expected);
        }
    }

    #[rstest]
    fn v_line_cols_include_outer_left_and_right(view: PlayerView, ruleset: Ruleset) {
        let plan = plan_board(&view, &ruleset, dpi1());

        // 10 cols → 11 column boundaries: left of col 0 through left of
        // col 9, plus the RIGHT pixel column of col 9. The right pixel
        // column is `ox + w - 1` (the inclusive rect right), NOT `ox + w`.
        assert_eq!(plan.v_line_cols.len(), 11);
        assert_eq!(plan.v_line_cols[0], plan.outer.x);
        assert_eq!(plan.v_line_cols[10], plan.outer.right_pixel());
        assert_eq!(plan.v_line_cols[10], 355);
        // Evenly spaced by cell_px for the first 10 entries
        for (i, expected) in (0..10).map(|i| 116 + i * 24).enumerate() {
            assert_eq!(plan.v_line_cols[i], expected);
        }
    }

    #[rstest]
    fn grid_lines_line_up_with_outer_rect(view: PlayerView, ruleset: Ruleset) {
        // The rightmost vertical grid line MUST equal the rect's right
        // pixel. Otherwise there's a gap between the grid and the
        // outline.
        let plan = plan_board(&view, &ruleset, dpi1());

        assert_eq!(plan.v_line_cols[0], plan.outer.x);
        assert_eq!(
            plan.v_line_cols[plan.v_line_cols.len() - 1],
            plan.outer.right_pixel(),
        );
        assert_eq!(plan.h_line_rows[0], plan.outer.y);
        assert_eq!(
            plan.h_line_rows[plan.h_line_rows.len() - 1],
            plan.outer.bottom_pixel(),
        );
    }

    #[rstest]
    fn dpi_2_doubles_pixel_positions(view: PlayerView, ruleset: Ruleset) {
        let plan = plan_board(&view, &ruleset, dpi2());

        // Origin 116 logical × dpi 2 = 232 physical
        assert_eq!(plan.outer.x, 232);
        assert_eq!(plan.outer.y, 32);
        // 10 × (24 × 2) = 480
        assert_eq!(plan.outer.w, 480);
        assert_eq!(plan.outer.h, 960);
        // Each row boundary is 48 physical pixels apart
        assert_eq!(plan.h_line_rows[0], 32);
        assert_eq!(plan.h_line_rows[1], 32 + 48);
    }

    #[rstest]
    fn fractional_dpi_snaps_to_nearest_pixel(mut view: PlayerView, ruleset: Ruleset) {
        // Origin 0.5 logical pixels off — must still produce integers
        view.layout.origin = (116.5, 16.5);
        let plan = plan_board(&view, &ruleset, dpi_1_5());

        // 116.5 * 1.5 = 174.75 → rounds to 175
        assert_eq!(plan.outer.x, 175);
        // 16.5 * 1.5 = 24.75 → rounds to 25
        assert_eq!(plan.outer.y, 25);
        // 24 * 1.5 = 36 → no rounding
        assert_eq!(plan.outer.w, 36 * 10);
        // First row boundary matches outer.y
        assert_eq!(plan.h_line_rows[0], plan.outer.y);
    }

    #[rstest]
    fn plan_box_uses_integer_pixel_size() {
        // Queue preview box: 4 cells × 16 logical pixels = 64 logical,
        // 64 physical at DPI 1.
        let plan = plan_box((360.0, 16.0), 64, dpi1());
        assert_eq!(plan.rect.x, 360);
        assert_eq!(plan.rect.y, 16);
        assert_eq!(plan.rect.w, 64);
        assert_eq!(plan.rect.h, 64);
        assert_eq!(plan.rect.right_pixel(), 423);
        assert_eq!(plan.rect.bottom_pixel(), 79);
    }

    #[rstest]
    fn cell_origin_returns_logical_coords() {
        // For a cell at col=2, row=3 in a 24px grid at origin (100, 200):
        let l = Layout {
            origin: (100.0, 200.0),
            cell_px: 24.0,
            ..Layout::default()
        };
        let (x, y) = cell_origin(&l, 2, 3, dpi1());
        assert_eq!(x, 100.0 + 2.0 * 24.0);
        assert_eq!(y, 200.0 + 3.0 * 24.0);
    }

    #[rstest]
    fn pixel_rect_helpers() {
        let r = PixelRect::new(10, 20, 100, 50);
        assert_eq!(r.right_pixel(), 109);
        assert_eq!(r.bottom_pixel(), 69);
    }
}
