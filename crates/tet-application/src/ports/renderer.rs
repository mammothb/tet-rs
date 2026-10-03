use tet_domain::{Ruleset, Vec2};

use crate::PlayerSnapshot;

/// One rendered frame. The renderer interprets each view at its layout,
/// using the ruleset for board dimensions and preview count.
pub struct Frame {
    pub ruleset: Ruleset,
    pub views: Vec<PlayerView>,
}

pub struct PlayerView {
    pub snapshot: PlayerSnapshot,
    pub label: &'static str,
    /// Where to render this view on screen.
    pub layout: Layout,
    /// Projected landing position for the current piece (rendered with
    /// reduced opacity). Computed by `tick::project_ghost(&player)`.
    pub ghost: Option<Vec2>,
}

/// Where to draw a board and what to show. Coordinates are in screen pixels,
/// top-left origin, y-down (matches the renderer's coordinate space). The
/// composition root (tet-app) builds a `Layout` per board.
///
/// Board dimensions (cols × rows) come from the `Frame`'s `Ruleset`, not
/// from this struct — keeping the layout focused on pixel-level config.
#[derive(Clone, Copy)]
pub struct Layout {
    /// Top-left of the board in screen pixels.
    pub origin: (f32, f32),
    /// Cell size in pixels.
    pub cell_px: f32,
    /// Render the upcoming pieces column.
    pub show_queue: bool,
    /// Render the held piece.
    pub show_hold: bool,
    /// Render faint grid lines on empty cells.
    pub show_grid: bool,
}

impl Layout {
    /// Default pixel-level layout. Cell size 24px, all visual features enabled.
    /// Combine with `..Layout::default()` for overrides.
    #[must_use]
    pub const fn default() -> Self {
        Self {
            origin: (0.0, 0.0),
            cell_px: 24.0,
            show_queue: true,
            show_hold: true,
            show_grid: true,
        }
    }

    /// Board width in pixels for `cols` columns.
    #[must_use]
    pub fn board_w(&self, cols: usize) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let cols = cols as f32;
        self.cell_px * cols
    }

    /// Board height in pixels for `rows` rows.
    #[must_use]
    pub fn board_h(&self, rows: usize) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let rows = rows as f32;
        self.cell_px * rows
    }

    /// Width occupied by the hold box on the LEFT of the board, including
    /// the spacing gap. The composition root adds this to the board's
    /// origin.x so the hold box fits on screen without overlapping the board.
    #[must_use]
    pub fn hold_box_width(&self) -> f32 {
        // Hold box: 0.6 * cell_px per cell, 4 cells wide, plus 4px gap to the board.
        self.cell_px * 0.6 * 4.0 + 4.0
    }
}

pub trait Renderer {
    fn render(&mut self, frame: &Frame);
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod test {
    use super::*;

    #[test]
    fn default_layout_uses_24px_cells_and_all_features_enabled() {
        let layout = Layout::default();
        assert_eq!(layout.cell_px, 24.0);
        assert_eq!(layout.origin, (0.0, 0.0));
        assert!(layout.show_queue);
        assert!(layout.show_hold);
        assert!(layout.show_grid);
    }

    #[test]
    fn board_w_and_h_use_cols_and_rows_from_ruleset() {
        let layout = Layout::default();
        let ruleset = Ruleset::guideline();
        assert_eq!(layout.board_w(ruleset.num_cols), 240.0);
        assert_eq!(layout.board_h(ruleset.num_rows), 600.0);
    }

    #[test]
    fn layout_can_be_overridden_for_smaller_boards() {
        let layout = Layout {
            cell_px: 16.0,
            ..Layout::default()
        };
        assert_eq!(layout.board_w(10), 160.0);
        assert_eq!(layout.board_h(25), 400.0);
        assert!(layout.show_queue); // unchanged
    }

    #[test]
    fn hold_box_width_scales_with_cell_size() {
        // Default 24px cells → 0.6 * 24 * 4 + 4 ≈ 61.6px wide sidebar.
        // Exact comparison would fail due to f32 precision (0.6 isn't exact),
        // so check within a small tolerance.
        let layout = Layout::default();
        let expected = 0.6 * 24.0 * 4.0 + 4.0;
        assert!(
            (layout.hold_box_width() - expected).abs() < 0.001,
            "expected ≈ {}, got {}",
            expected,
            layout.hold_box_width()
        );
        // Smaller cells → proportionally smaller sidebar.
        let layout_small = Layout {
            cell_px: 16.0,
            ..Layout::default()
        };
        let expected_small = 0.6 * 16.0 * 4.0 + 4.0;
        assert!((layout_small.hold_box_width() - expected_small).abs() < 0.001);
        // And the small case is smaller than the default case.
        assert!(layout_small.hold_box_width() < layout.hold_box_width());
    }
}
