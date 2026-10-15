//! Active piece + ghost rendering.

// Casts bounded by domain invariants (cell coords ∈ i8, casts to i32
// are lossless within piece-cell ranges).
#![allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]

pub mod ghost;

use tet_domain::{Cell, Vec2};

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::layout::PixelRect;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// Render the active tetromino. If `ghost` is `Some`, also render
/// the ghost piece at the projected landing position
/// (translucent fill + opaque outline, both via [`Canvas`]).
pub fn draw(vp: &Viewport, theme: &Theme, ghost: Option<Vec2>, canvas: &mut dyn Canvas) {
    draw_active(vp, canvas);
    if let Some(ghost_pos) = ghost {
        ghost::draw(vp, theme, ghost_pos, canvas);
    }
}

/// Draw the current piece's 4 cells, skipping those outside the
/// visible row range.
fn draw_active(vp: &Viewport, canvas: &mut dyn Canvas) {
    let kind = vp.snapshot().current.kind;
    let cw = vp.cell_px_phys();
    let visible_rows = vp.ruleset().num_visible_rows as i32;

    for cell_pos in vp.snapshot().current.cells() {
        let col = i32::from(cell_pos.x);
        let domain_row = i32::from(cell_pos.y);
        if domain_row < 0 || domain_row >= visible_rows {
            continue;
        }
        let (x_phys, y_phys) = vp.cell_pixel(col, domain_row);
        canvas.draw_cell_sprite(
            PixelRect::new(x_phys, y_phys, cw, cw),
            Cell::Block(kind),
            255,
        );
    }
}

#[cfg(test)]
#[allow(clippy::similar_names)]
mod test {
    use super::*;
    use rstest::rstest;

    use tet_application::{Phase, Piece, PlayerSnapshot};
    use tet_domain::{Board, MinoType, Ruleset};

    use crate::rendering::backend::canvas::mock::{DrawOp, RecordingCanvas};
    use crate::rendering::view::{GridStyle, Layout};
    use crate::rendering::viewport::Viewport;

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
            // I-spawn keeps all 4 cells inside the visible window
            // (T-spawn has its tip at y=20, above visible_rows=20).
            current: Piece::spawn(MinoType::I),
            phase: Phase::Playing,
            pending_garbage: vec![],
        };
        let layout = Layout {
            origin: (0.0, 0.0),
            cell_px: 24.0,
            show_queue: false,
            show_hold: false,
            show_stats: false,
            show_label: false,
            grid_style: GridStyle::None,
        };
        (snapshot, layout, ruleset)
    }

    /// Without a ghost, the active piece emits one `CellSprite` per
    /// visible cell — 4 cells for the I-spawn. All carry
    /// [`Cell::Block(MinoType::I)`].
    #[test]
    fn active_piece_records_four_sprites() {
        let (snapshot, layout, ruleset) = test_setup();
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();
        super::draw(&vp, &Theme::DARK, None, &mut canvas);

        let sprites: Vec<_> = canvas
            .calls
            .iter()
            .filter_map(|op| match op {
                DrawOp::CellSprite { rect, cell, alpha } => Some((*rect, *cell, *alpha)),
                _ => None,
            })
            .collect();
        assert_eq!(
            sprites.len(),
            4,
            "expected 4 CellSprite calls, got {sprites:?}"
        );
        for (_, sprite, _) in &sprites {
            assert!(matches!(sprite, Cell::Block(MinoType::I)));
        }
    }

    /// With a ghost at the floor (pos.y=0), `pos_offset_y` = -17 for
    /// the I-spawn at pos=(3,17). All 4 ghost cells land inside
    /// the visible window. Active piece emits 4 `CellSprite`; ghost emits
    /// 4 fills + 4 outline strokes.
    #[test]
    fn piece_with_ghost_records_transluscent_ghost_sprites() {
        let (snapshot, layout, ruleset) = test_setup();
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();
        let current_pos = vp.snapshot().current.pos;
        // Ghost origin lands at the bottom row: ghost_pos.y = 0.
        let ghost_pos = Vec2::new(current_pos.x, 0);
        super::draw(&vp, &Theme::DARK, Some(ghost_pos), &mut canvas);

        let (opaque, translucent): (Vec<_>, Vec<_>) = canvas
            .calls
            .iter()
            .filter_map(|op| match op {
                DrawOp::CellSprite { rect, cell, alpha } => Some((*rect, *cell, *alpha)),
                _ => None,
            })
            .partition(|(_, _, a)| *a == 255);

        assert_eq!(opaque.len(), 4, "4 active CellSprite calls");
        assert_eq!(translucent.len(), 4, "4 ghost CellSprite calls");
        for (_, _, a) in &translucent {
            assert_eq!(*a, Theme::DARK.ghost_alpha, "ghost alpha matches theme");
        }
    }

    /// Without ghost, no `StrokeRect` calls happen.
    #[test]
    fn piece_without_ghost_records_no_strokes() {
        let (snapshot, layout, ruleset) = test_setup();
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();
        super::draw(&vp, &Theme::DARK, None, &mut canvas);

        let stroke_count = canvas
            .calls
            .iter()
            .filter(|op| matches!(op, DrawOp::StrokeRect { .. }))
            .count();
        assert_eq!(stroke_count, 0);
    }

    /// Sanity that `piece::draw` works through a trait object
    /// (`&mut dyn Canvas`), not just the concrete type.
    #[rstest]
    fn piece_draw_trait_object_compiles() {
        let (snapshot, layout, ruleset) = test_setup();
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas: Box<dyn Canvas> = Box::new(RecordingCanvas::default());
        super::draw(&vp, &Theme::DARK, None, canvas.as_mut());
    }
}
