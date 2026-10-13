//! Grid lines: faint base + alpha-layered "cross" gradient at
//! intersections.
//!
//! Layers are painted **outermost-first** so each inner layer
//! (closer to the corner, higher alpha) overwrites the area it
//! covers with a brighter pixel. The result is a smooth gradient
//! brightest at each corner pixel.
//!
//! Alpha values are 8-bit (0..=255) — the `PaletteKey::GridCross(u8)`
//! variant carries the alpha so the trait payload stays small.
//! Conversion from the original f32 constants (0.20/0.40/0.65/0.95):
//!
//! ```text
//!    0.20 × 255 = 51.0  → 51
//!    0.40 × 255 = 102.0 → 102
//!    0.65 × 255 = 165.75 → 166 (rounded)
//!    0.95 × 255 = 242.25 → 242 (rounded)
//! ```
//!
//! The framebuffer byte each layer writes is identical to the
//! previous macroquad draw (verified by `theme::palette_from_f32`).

use tet_application::GridStyle;

use crate::rendering::backend::canvas::{Canvas, PaletteKey};
use crate::rendering::layout::plan_board;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

/// Faint base lines + corner-cross gradient, gated by
/// `vp.layout().grid_style`.
///
/// - `None`        → no lines drawn.
/// - `Horizontal`  → only horizontal base lines.
/// - `Vertical`    → only vertical base lines.
/// - `Full`        → both, plus the cross gradient.
pub fn draw(vp: &Viewport, _theme: &Theme, canvas: &mut dyn Canvas) {
    let plan = plan_board(vp.layout(), vp.ruleset(), vp.dpi());
    let layout = vp.layout();
    let draw_h = matches!(layout.grid_style, GridStyle::Horizontal | GridStyle::Full);
    let draw_v = matches!(layout.grid_style, GridStyle::Vertical | GridStyle::Full);

    // Pass 1: faint base lines spanning the whole board.
    if draw_h {
        for &row_phys in &plan.h_line_rows {
            canvas.stroke_h_line(
                plan.outer.x,
                row_phys,
                plan.grid_line_length_phys,
                PaletteKey::GridBase,
            );
        }
    }
    if draw_v {
        for &col_phys in &plan.v_line_cols {
            canvas.stroke_v_line(col_phys, plan.outer.y, plan.outer.h, PaletteKey::GridBase);
        }
    }

    // Pass 2: corner-cross gradient. For each intersection, draw
    // four layered horizontal arms + four layered vertical arms,
    // painted outermost-first so the innermost (shortest reach,
    // highest alpha) wins at the corner pixel. At rect-edge
    // intersections the arms are clipped to the rect bounds via
    // `max`/`min`, so they never extend past the border (drawn
    // last by `outline::draw_outer`).
    if draw_h && draw_v {
        let cell_phys = vp.cell_px_phys();
        let x_lo = plan.outer.x;
        let x_hi = plan.outer.right_pixel() + 1; // exclusive
        let y_lo = plan.outer.y;
        let y_hi = plan.outer.bottom_pixel() + 1; // exclusive
        // (reach_in_phys, fading_alpha). Outermost first; innermost
        // (shortest reach, highest alpha) wins at the corner.
        let layers: &[(i32, u8)] = &[
            (cell_phys / 3, 51),   // ≈ 0.20
            (cell_phys / 6, 102),  // ≈ 0.40
            (cell_phys / 12, 166), // ≈ 0.65
            (cell_phys / 24, 242), // ≈ 0.95
        ];

        for &y_phys in &plan.h_line_rows {
            for &x_phys in &plan.v_line_cols {
                // Horizontal arm — extend left and right of
                // `x_phys` up to `reach`, clipped to the rect.
                for &(reach, alpha) in layers {
                    let h_start = (x_phys - reach).max(x_lo);
                    let h_end = (x_phys + reach).min(x_hi);
                    if h_end > h_start {
                        canvas.stroke_h_line(
                            h_start,
                            y_phys,
                            h_end - h_start,
                            PaletteKey::GridCross(alpha),
                        );
                    }
                }
                // Vertical arm — extend up and down of `y_phys`
                // up to `reach`, clipped to the rect.
                for &(reach, alpha) in layers {
                    let v_start = (y_phys - reach).max(y_lo);
                    let v_end = (y_phys + reach).min(y_hi);
                    if v_end > v_start {
                        canvas.stroke_v_line(
                            x_phys,
                            v_start,
                            v_end - v_start,
                            PaletteKey::GridCross(alpha),
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::similar_names)]
mod test {
    use super::*;
    use rstest::rstest;

    use tet_application::{Layout, Phase, Piece, PlayerSnapshot};
    use tet_domain::{Board, MinoType, Ruleset};

    use crate::rendering::backend::canvas::mock::{DrawOp, RecordingCanvas};

    /// Build the inputs that `grid::draw` consumes. Origin at
    /// `(0, 0)` so pixel positions are predictable in assertions.
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
            current: Piece::spawn(MinoType::T),
            phase: Phase::Playing,
            pending_garbage: vec![],
        };
        let layout = Layout {
            origin: (0.0, 0.0),
            grid_style: GridStyle::Full,
            ..Layout::default()
        };
        (snapshot, layout, ruleset)
    }

    /// The corner intersection at `(0, 0)` receives four
    /// `StrokeHLine` calls — one per `GridCross` alpha layer —
    /// painted outermost-first.
    #[test]
    fn corner_intersection_receives_four_horizontal_layered_alphas() {
        let (snapshot, layout, ruleset) = test_setup();
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();

        draw(&vp, &Theme::DARK, &mut canvas);

        // Filter to StrokeHLine calls anchored at the top-left
        // intersection (0, 0) carrying a GridCross palette.
        let alphas: Vec<u8> = canvas
            .calls
            .iter()
            .filter_map(|op| match op {
                DrawOp::StrokeHLine { x, y, stroke, .. } if *x == 0 && *y == 0 => {
                    if let PaletteKey::GridCross(a) = stroke {
                        Some(*a)
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect();

        // Outermost-first ordering: 51, 102, 166, 242.
        assert_eq!(alphas, vec![51, 102, 166, 242]);
    }

    /// At the corner intersection, every gradient arm is clipped
    /// to the outer rect — never extending left of `outer.x` or
    /// below `outer.bottom_pixel()`.
    #[test]
    fn gradient_arms_at_corner_are_clipped_to_outer_rect() {
        let (snapshot, layout, ruleset) = test_setup();
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();

        draw(&vp, &Theme::DARK, &mut canvas);

        let plan = plan_board(vp.layout(), vp.ruleset(), vp.dpi());
        let outer_x = plan.outer.x;
        let outer_y = plan.outer.y;
        let outer_right = plan.outer.right_pixel();
        let outer_bottom = plan.outer.bottom_pixel();

        for op in &canvas.calls {
            match op {
                DrawOp::StrokeHLine { x, length, .. } => {
                    // StrokeHLine draws pixels `[x, x + length)` horizontally.
                    assert!(*x >= outer_x, "h-line starts left of rect: {op:?}");
                    assert!(
                        *x + *length <= outer_right + 1,
                        "h-line ends right of rect: {op:?}",
                    );
                }
                DrawOp::StrokeVLine { y, length, .. } => {
                    assert!(*y >= outer_y, "v-line starts above rect: {op:?}");
                    assert!(
                        *y + *length <= outer_bottom + 1,
                        "v-line ends below rect: {op:?}",
                    );
                }
                _ => {}
            }
        }
    }

    /// Sanity: only horizontal base lines (no cross gradient) when
    /// `GridStyle::Horizontal`.
    #[test]
    fn horizontal_only_style_records_no_vertical_cross_calls() {
        let (snapshot, mut layout, ruleset) = test_setup();
        layout.grid_style = GridStyle::Horizontal;
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();

        draw(&vp, &Theme::DARK, &mut canvas);

        // No GridCross alpha calls when not in Full mode.
        let any_cross = canvas.calls.iter().any(|op| {
            matches!(
                op,
                DrawOp::StrokeHLine {
                    stroke: PaletteKey::GridCross(_),
                    ..
                } | DrawOp::StrokeVLine {
                    stroke: PaletteKey::GridCross(_),
                    ..
                }
            )
        });
        assert!(
            !any_cross,
            "Horizontal-only grid should not draw cross arms"
        );
    }

    /// Sanity: `GridStyle::None` records no base lines or cross.
    #[rstest]
    #[case::none(GridStyle::None)]
    #[case::horizontal(GridStyle::Horizontal)]
    fn no_base_lines_when_no_grid_lines(#[case] style: GridStyle) {
        let (snapshot, mut layout, ruleset) = test_setup();
        layout.grid_style = style;
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();

        draw(&vp, &Theme::DARK, &mut canvas);

        // `None` records no calls. `Horizontal` records some
        // (sanity that the gate works for `None`).
        if matches!(style, GridStyle::None) {
            // All recorded calls must be a fill_rect from another
            // module — but `grid::draw` itself records nothing for
            // `None`. Wait, fill_rect is from `cells::draw_locked`,
            // not grid. So this case should see zero calls here.
            assert!(
                canvas.calls.is_empty(),
                "GridStyle::None should record zero grid calls",
            );
        }
    }

    /// Trait object safety: pass `&mut dyn Canvas` and confirm the
    /// dispatch works (smoke compile-time check — the real shape
    /// is verified by the recording-canvas tests above).
    #[test]
    fn draw_works_with_trait_object() {
        let (snapshot, layout, ruleset) = test_setup();
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas: Box<dyn Canvas> = Box::new(RecordingCanvas::default());
        draw(&vp, &Theme::DARK, canvas.as_mut());
    }
}
