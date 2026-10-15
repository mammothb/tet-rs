//! Side panels: hold (left) and queue (right).
//!
//! Gated by `vp.layout().show_hold` / `vp.layout().show_queue`.

// Casts bounded by domain invariants (board dims ≤ 128, dpi ≤ 4).
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::cast_sign_loss,
    clippy::similar_names
)]

pub mod hold;
pub mod queue;

use tet_domain::MinoType;

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::layout::PixelRect;
use crate::rendering::piece_box::PieceBox;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// Render whichever side panels are enabled for this view.
pub fn draw(vp: &Viewport, theme: &Theme, canvas: &mut dyn Canvas) {
    if vp.layout().show_hold {
        hold::draw(vp, theme, canvas);
    }
    if vp.layout().show_queue {
        queue::draw(vp, theme, canvas);
    }
}

/// Iterate the 4 cells of a `PieceBox` and call `f` for each. Shared
/// by `hold::draw` and `queue::draw` so the iteration loop lives in
/// exactly one place.
///
/// The closure receives a `PixelRect` for each of the box's cells,
/// already offset by `origin_phys` and sized by `piece_cell`. The
/// typical body is `canvas.fill_rect(rect, fill)` with a palette key
/// matching the active mino.
pub(crate) fn for_each_piece_cell<F>(
    kind: MinoType,
    origin_phys: (i32, i32),
    piece_cell: i32,
    box_cells: i32,
    mut f: F,
) where
    F: FnMut(PixelRect),
{
    for rect in PieceBox::new(kind, origin_phys, piece_cell, box_cells).rects() {
        f(rect);
    }
}

#[cfg(test)]
#[allow(clippy::similar_names)]
mod test {
    use super::*;

    use tet_application::{Phase, Piece, PlayerSnapshot};
    use tet_domain::{Board, Cell, MinoType, Ruleset};

    use crate::rendering::backend::canvas::mock::{DrawOp, RecordingCanvas};
    use crate::rendering::view::{GridStyle, Layout};

    fn test_setup() -> (PlayerSnapshot, Layout, Ruleset) {
        let ruleset = Ruleset::guideline();
        let snapshot = PlayerSnapshot {
            board: Board::new(
                u8::try_from(ruleset.num_cols).unwrap(),
                u8::try_from(ruleset.num_rows).unwrap(),
            ),
            queue: vec![],
            hold: None,
            combo: 0,
            b2b: false,
            current: Piece::spawn(MinoType::I),
            phase: Phase::Playing,
            pending_garbage: vec![],
        };
        // Origin (116, 16), 24 px cells — matches the live
        // `tet-app` composition root, so numeric assertions below
        // match the production geometry.
        let layout = Layout {
            origin: (116.0, 16.0),
            cell_px: 24.0,
            show_queue: true,
            show_hold: true,
            show_stats: false,
            show_label: false,
            grid_style: GridStyle::None,
        };
        (snapshot, layout, ruleset)
    }

    // ---- orchestrator gating ----

    #[test]
    fn side_panel_skips_both_when_both_flags_off() {
        let (snapshot, mut layout, ruleset) = test_setup();
        layout.show_hold = false;
        layout.show_queue = false;
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();

        draw(&vp, &Theme::DARK, &mut canvas);

        assert!(
            canvas.calls.is_empty(),
            "no calls expected when both flags are off, got: {:?}",
            canvas.calls,
        );
    }

    #[test]
    fn side_panel_renders_only_hold_when_queue_disabled() {
        let (snapshot, mut layout, ruleset) = test_setup();
        layout.show_hold = true;
        layout.show_queue = false;
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();

        draw(&vp, &Theme::DARK, &mut canvas);

        let stroke_count = canvas
            .calls
            .iter()
            .filter(|op| matches!(op, DrawOp::StrokeRect { .. }))
            .count();
        assert_eq!(stroke_count, 1, "only the hold box outline");
    }

    #[test]
    fn side_panel_renders_only_queue_when_hold_disabled() {
        let (mut snapshot, mut layout, ruleset) = test_setup();
        snapshot.queue = vec![MinoType::T, MinoType::S];
        layout.show_hold = false;
        layout.show_queue = true;
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();

        draw(&vp, &Theme::DARK, &mut canvas);

        let stroke_count = canvas
            .calls
            .iter()
            .filter(|op| matches!(op, DrawOp::StrokeRect { .. }))
            .count();
        assert_eq!(stroke_count, 2, "two queue box outlines");
    }

    // ---- hold geometry ----

    #[test]
    fn hold_box_outline_is_left_of_board_origin() {
        let (snapshot, layout, ruleset) = test_setup();
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();

        hold::draw(&vp, &Theme::DARK, &mut canvas);

        let outline = canvas
            .calls
            .iter()
            .find_map(|op| match op {
                DrawOp::StrokeRect { rect, .. } => Some(*rect),
                _ => None,
            })
            .expect("hold should emit StrokeRect for the box outline");

        // ox = 116, cw = 24, hold_box_cells = 4 → box_size = 96.
        // hold_x_phys = ox − (box_size + 4) = 16.
        assert_eq!(outline, PixelRect::new(16, 16, 96, 96));
    }

    // ---- queue geometry ----

    #[test]
    fn queue_outlines_stack_vertically_right_of_board() {
        let (mut snapshot, layout, ruleset) = test_setup();
        snapshot.queue = vec![MinoType::T, MinoType::S, MinoType::Z];
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();

        queue::draw(&vp, &Theme::DARK, &mut canvas);

        // Filter to StrokeRects at the queue column x position.
        // ox = 116, bw = cw * num_cols = 24 * 10 = 240, spacing = 4
        // → qx = 116 + 240 + 4 = 360.
        // queue cell_px = 16 → box_phys = 64.
        // First box: y = 16, second: y = 16 + 64 + 4 = 84,
        // third: y = 16 + 2 * (64 + 4) = 152.
        let queue_outlines: Vec<PixelRect> = canvas
            .calls
            .iter()
            .filter_map(|op| match op {
                DrawOp::StrokeRect { rect, .. } if rect.x == 360 => Some(*rect),
                _ => None,
            })
            .collect();

        assert_eq!(queue_outlines.len(), 3);
        assert_eq!(queue_outlines[0], PixelRect::new(360, 16, 64, 64));
        assert_eq!(queue_outlines[1].y, 84);
        assert_eq!(queue_outlines[2].y, 152);
    }

    // ---- PieceBox construction ----
    //
    // When the player holds a piece, `hold::draw` constructs a
    // `PieceBox` with origin (hold_x_phys, hold_y_phys) and size
    // cw. Verifies via the *position* of the resulting fill rects.
    //
    // I-piece local cells (0..3, 2) → screen row
    // `box_cells - 1 - local_y` = `4 - 1 - 2` = 1 →
    // rect.y = hold_y_phys + screen_row * cw = 16 + 24 = 40.

    #[test]
    fn hold_piece_cells_use_hold_box_origin_and_cell_size() {
        let (mut snapshot, layout, ruleset) = test_setup();
        snapshot.hold = Some(MinoType::I);
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();

        hold::draw(&vp, &Theme::DARK, &mut canvas);

        let sprites: Vec<_> = canvas
            .calls
            .iter()
            .filter_map(|op| match op {
                DrawOp::CellSprite { rect, cell, alpha } => Some((*rect, *cell, *alpha)),
                _ => None,
            })
            .collect();

        // Outline + label = no sprites. 4 piece sprites from PieceBox.
        assert_eq!(sprites.len(), 4);
        for (rect, sprite, _) in &sprites {
            assert_eq!(
                rect.w, 24,
                "piece cell width = cw (= box_size / hold_box_cells)"
            );
            assert_eq!(rect.h, 24, "piece cell height");
            assert!(matches!(sprite, Cell::Block(MinoType::I)));
            // I-piece local y=2, screen row = box_cells - 1 - 2 = 1,
            // so rect.y = 16 + 1 * 24 = 40.
            assert_eq!(rect.y, 40, "I-piece screen row 1 in the hold box");
        }
    }
}
