//! Per-player render viewport.
//!
//! A `Viewport` bundles everything the renderer needs to translate one
//! player's logical layout into physical pixel coordinates:
//!
//! - The player's game state (`PlayerSnapshot`).
//! - The `Layout` they were assigned on screen (origin, cell size, etc).
//! - The `Ruleset` (board dimensions).
//! - The renderer's DPI scale (rounded to integer pixels).
//!
//! All pixel computations live here. Feature modules (`board/`, `piece/`,
//! etc.) read pixel positions from `Viewport` and never recompute
//! `(x * dpi).round() as i32` themselves.
//!
//! ## Coordinate convention
//!
//! Domain coordinates: `x` right, `y` up, `(0, 0)` = bottom-left of the
//! playfield.
//! Screen coordinates: `x` right, `y` down, `(0, 0)` = top-left of the
//! canvas. This matches macroquad's convention.
//!
//! [`domain_to_screen_row`] flips the Y axis once per cell. The
//! renderer calls it (or [`Viewport::cell_pixel`]) instead of doing
//! the `region_height - 1 - y` arithmetic inline.

// Casts bounded by domain invariants (rows/cell_px/dpi are small).
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]

use tet_application::PlayerSnapshot;
use tet_domain::Ruleset;

use crate::rendering::view::Layout;

/// Domain Y (0 = bottom, growing up) → screen row offset from the top
/// of a region of the given height, in cells.
///
/// The flipped value is the **row offset**, not a pixel position. The
/// caller is responsible for multiplying by the physical cell size.
#[must_use]
pub const fn domain_to_screen_row(domain_y: i32, region_height: i32) -> i32 {
    region_height - 1 - domain_y
}

/// One rendered player's worth of layout + draw context.
///
/// Holds references rather than values so the renderer operates on the
/// `Frame`'s data in place without copying per draw call.
pub struct Viewport<'a> {
    snapshot: &'a PlayerSnapshot,
    layout: &'a Layout,
    ruleset: &'a Ruleset,
    dpi: f32,
}

impl<'a> Viewport<'a> {
    #[must_use]
    pub fn new(
        snapshot: &'a PlayerSnapshot,
        layout: &'a Layout,
        ruleset: &'a Ruleset,
        dpi: f32,
    ) -> Self {
        Self {
            snapshot,
            layout,
            ruleset,
            dpi,
        }
    }

    /// Physical pixel size of one logical cell (`cell_px * dpi`, rounded).
    #[must_use]
    pub const fn cell_px_phys(&self) -> i32 {
        (self.layout.cell_px * self.dpi).round() as i32
    }

    /// Board's top-left corner in physical pixels. `(Layout::origin.0,
    /// Layout::origin.1)` rounded through `dpi`.
    #[must_use]
    pub fn origin_pixel(&self) -> (i32, i32) {
        (
            (self.layout.origin.0 * self.dpi).round() as i32,
            (self.layout.origin.1 * self.dpi).round() as i32,
        )
    }

    /// Logical cell position → physical pixel top-left of the cell.
    ///
    /// `domain_row` is the cell's row in domain coordinates (0 =
    /// bottom). The Y axis is flipped via [`domain_to_screen_row`].
    /// Cells outside the visible row range still return a position —
    /// renderers should clip via the planned [`crate::layout::PixelRect`]
    /// bounds.
    #[must_use]
    pub fn cell_pixel(&self, col: i32, domain_row: i32) -> (i32, i32) {
        let cw = self.cell_px_phys();
        let (ox, oy) = self.origin_pixel();
        let visible_rows = self.ruleset.num_visible_rows as i32;
        let screen_row = domain_to_screen_row(domain_row, visible_rows);
        (ox + col * cw, oy + screen_row * cw)
    }

    /// The player's game state snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &'a PlayerSnapshot {
        self.snapshot
    }

    /// The player's screen-space layout.
    #[must_use]
    pub const fn layout(&self) -> &'a Layout {
        self.layout
    }

    /// The ruleset for board dimensions.
    #[must_use]
    pub const fn ruleset(&self) -> &'a Ruleset {
        self.ruleset
    }

    /// The DPI scale used by the renderer.
    #[must_use]
    pub const fn dpi(&self) -> f32 {
        self.dpi
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod test {
    use super::*;
    use rstest::{fixture, rstest};

    use tet_application::{Phase, Piece, PlayerSnapshot};
    use tet_domain::{Board, MinoType, Ruleset};

    use crate::rendering::view::Layout;

    // ---- fixtures ----
    //
    // Snapshot & layout fixtures take `Ruleset` so the same values feed
    // both Viewport tests and downstream plan-board tests.

    /// Standard guideline ruleset: 10 cols, 25 total rows, 20 visible.
    #[fixture]
    fn ruleset() -> Ruleset {
        Ruleset::guideline()
    }

    /// Test layout matching the live `tet-app` composition root:
    /// origin `(116, 16)`, 24 px cells, all visual features enabled.
    #[fixture]
    fn layout() -> Layout {
        Layout {
            origin: (116.0, 16.0),
            cell_px: 24.0,
            ..Layout::default()
        }
    }

    /// Minimal `PlayerSnapshot` with a fresh T-piece on an empty board.
    /// The renderer's coordinate helpers in this phase don't read the
    /// snapshot's contents; this exists only so the `Viewport<'a>`
    /// holds a valid reference.
    #[fixture]
    fn snapshot(ruleset: Ruleset) -> PlayerSnapshot {
        PlayerSnapshot {
            board: Board::new(
                u8::try_from(ruleset.num_cols).unwrap(),
                u8::try_from(ruleset.num_rows).unwrap(),
            ),
            queue: vec![],
            hold: None,
            combo: 0,
            b2b: false,
            current: Piece::spawn(MinoType::T),
            phase: Phase::Playing,
            pending_garbage: vec![],
        }
    }

    // ---- DPI helpers (return f32, so they can't be `#[fixture]` —
    // multiple `f32`-returning fixtures are ambiguous in rstest).
    // Tests call them explicitly. ----

    fn dpi1() -> f32 {
        1.0
    }

    fn dpi2() -> f32 {
        2.0
    }

    fn dpi_1_5() -> f32 {
        1.5
    }

    // ---- `domain_to_screen_row` ----
    //
    // Mirrors the existing `screen_y_offset_cells` tests but typed
    // `i32`. Bottom of domain → bottom of region; top → top.

    #[rstest]
    #[case(0, 4, 3)] // bottom of domain → bottom of region
    #[case(1, 4, 2)]
    #[case(2, 4, 1)]
    #[case(3, 4, 0)] // top of domain → top of region
    fn domain_to_screen_row_inverts_y_in_4_cell_region(
        #[case] domain_y: i32,
        #[case] region_height: i32,
        #[case] expected: i32,
    ) {
        assert_eq!(domain_to_screen_row(domain_y, region_height), expected);
    }

    #[rstest]
    #[case(0, 20, 19)] // 20 visible rows: bottom row → offset 19
    #[case(19, 20, 0)] // top row → offset 0
    fn domain_to_screen_row_scales_with_region_height(
        #[case] domain_y: i32,
        #[case] region_height: i32,
        #[case] expected: i32,
    ) {
        assert_eq!(domain_to_screen_row(domain_y, region_height), expected);
    }

    // ---- `Viewport::cell_px_phys` ----
    //
    // `cell_px * dpi` rounded to the nearest integer physical pixel.

    #[rstest]
    fn cell_px_phys_at_dpi_1_equals_logical_cell_px(
        snapshot: PlayerSnapshot,
        layout: Layout,
        ruleset: Ruleset,
    ) {
        let vp = Viewport::new(&snapshot, &layout, &ruleset, dpi1());
        // cell_px = 24.0, dpi = 1.0 → 24
        assert_eq!(vp.cell_px_phys(), 24);
    }

    #[rstest]
    fn cell_px_phys_scales_linearly_with_dpi(
        snapshot: PlayerSnapshot,
        layout: Layout,
        ruleset: Ruleset,
    ) {
        let vp = Viewport::new(&snapshot, &layout, &ruleset, dpi2());
        // cell_px = 24.0, dpi = 2.0 → 48
        assert_eq!(vp.cell_px_phys(), 48);
    }

    #[rstest]
    fn cell_px_phys_rounds_fractional_dpi_to_nearest_pixel(
        snapshot: PlayerSnapshot,
        layout: Layout,
        ruleset: Ruleset,
    ) {
        let vp = Viewport::new(&snapshot, &layout, &ruleset, dpi_1_5());
        // cell_px = 24.0 × 1.5 = 36 (no rounding needed)
        assert_eq!(vp.cell_px_phys(), 36);
    }

    // ---- `Viewport::origin_pixel` ----
    //
    // `(origin.x, origin.y)` rounded through `dpi`.

    #[rstest]
    fn origin_pixel_at_dpi_1_equals_logical_origin(
        snapshot: PlayerSnapshot,
        layout: Layout,
        ruleset: Ruleset,
    ) {
        let vp = Viewport::new(&snapshot, &layout, &ruleset, dpi1());
        // origin = (116, 16), dpi = 1.0 → (116, 16)
        assert_eq!(vp.origin_pixel(), (116, 16));
    }

    #[rstest]
    fn origin_pixel_scales_with_dpi(snapshot: PlayerSnapshot, layout: Layout, ruleset: Ruleset) {
        let vp = Viewport::new(&snapshot, &layout, &ruleset, dpi2());
        // origin = (116, 16) × 2 → (232, 32)
        assert_eq!(vp.origin_pixel(), (232, 32));
    }

    #[rstest]
    fn origin_pixel_rounds_fractional_dpi_to_nearest_pixel(
        snapshot: PlayerSnapshot,
        layout: Layout,
        ruleset: Ruleset,
    ) {
        // Layout fixture has origin (116.0, 16.0); mutate to (116.5,
        // 16.5) and verify rounding to (175, 25) at dpi 1.5:
        //   116.5 × 1.5 = 174.75 → 175
        //   16.5 × 1.5 = 24.75 → 25
        let mut l = layout;
        l.origin = (116.5, 16.5);
        let vp = Viewport::new(&snapshot, &l, &ruleset, dpi_1_5());
        assert_eq!(vp.origin_pixel(), (175, 25));
    }

    // ---- `Viewport::cell_pixel` ----
    //
    // Logical `(col, domain_row)` → physical pixel top-left of the
    // cell, including the Y-flip. With origin (116, 16), cell_px 24,
    // 20 visible rows:
    //
    //   screen_row(y) = 19 - y
    //   cell_pixel(c, y) = (116 + c*24, 16 + (19 - y)*24)

    #[rstest]
    fn cell_pixel_bottom_left_cell(snapshot: PlayerSnapshot, layout: Layout, ruleset: Ruleset) {
        let vp = Viewport::new(&snapshot, &layout, &ruleset, dpi1());
        // col=0, row=0 → screen_row=19 → x=116, y=16+19*24=472
        assert_eq!(vp.cell_pixel(0, 0), (116, 472));
    }

    #[rstest]
    fn cell_pixel_top_left_cell(snapshot: PlayerSnapshot, layout: Layout, ruleset: Ruleset) {
        let vp = Viewport::new(&snapshot, &layout, &ruleset, dpi1());
        // col=0, row=19 → screen_row=0 → (116, 16)
        assert_eq!(vp.cell_pixel(0, 19), (116, 16));
    }

    #[rstest]
    fn cell_pixel_middle_cell(snapshot: PlayerSnapshot, layout: Layout, ruleset: Ruleset) {
        let vp = Viewport::new(&snapshot, &layout, &ruleset, dpi1());
        // col=2, row=10 → screen_row=9 → x=116+48=164, y=16+9*24=232
        assert_eq!(vp.cell_pixel(2, 10), (164, 232));
    }

    #[rstest]
    fn cell_pixel_bottom_right_cell(snapshot: PlayerSnapshot, layout: Layout, ruleset: Ruleset) {
        let vp = Viewport::new(&snapshot, &layout, &ruleset, dpi1());
        // col=9, row=0 → screen_row=19 → x=116+9*24=332, y=472
        assert_eq!(vp.cell_pixel(9, 0), (332, 472));
    }

    // ---- accessors ----

    #[rstest]
    fn accessors_return_inner_references(
        snapshot: PlayerSnapshot,
        layout: Layout,
        ruleset: Ruleset,
    ) {
        let vp = Viewport::new(&snapshot, &layout, &ruleset, dpi1());
        assert!(std::ptr::eq(vp.snapshot(), &raw const snapshot));
        assert!(std::ptr::eq(vp.layout(), &raw const layout));
        assert!(std::ptr::eq(vp.ruleset(), &raw const ruleset));
        assert_eq!(vp.dpi(), 1.0);
    }
}
