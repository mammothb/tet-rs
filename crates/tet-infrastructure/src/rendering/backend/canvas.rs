//! Backend-agnostic drawing surface.
//!
//! The renderer draws via the [`Canvas`] trait so that:
//!
//! - Per-feature rendering modules don't depend on macroquad.
//! - Tests can substitute a [`mock::RecordingCanvas`] (or any other
//!   mock impl) to assert on draw-call sequences without spinning
//!   up a GL context.
//!
//! Concrete backends (macroquad, headless, recorder, ...) live under
//! `rendering::backend::*`. They translate palette keys + pixel
//! coordinates into the backend's primitive.
//!
//! ## Palette keys
//!
//! Every draw call carries a [`PaletteKey`] rather than a concrete
//! color. The macroquad backend maps keys to `Theme::DARK` values
//! (or any other `Theme`); the recording mock preserves keys
//! verbatim. This keeps the canvas trait payload-tiny and lets the
//! same backend palette be swapped at runtime.
//!
//! [`PaletteKey::GridCross`] carries its alpha because the board's
//! gradient effect draws the same color at multiple alphas (see
//! `render_board`'s 4-layer cross). Packaging alpha in the key
//! avoids adding an `alpha: u8` parameter on every line-draw call.

use tet_domain::{Cell, MinoType};

use crate::rendering::layout::PixelRect;

/// Backend-agnostic drawing surface.
///
/// All pixel coordinates are physical pixels (integer). Text uses
/// **logical** pixels to match the existing `draw_label` convention.
pub trait Canvas {
    /// Clear the whole framebuffer with the given palette.
    fn clear(&mut self, fill: PaletteKey);

    /// Draw a piece cell sprite at `rect`. `cell` selects the source tile from
    /// the loaded sprite sheet. `Cell::Empty` is unreachable, callers skips
    /// empty cells before drawing. Implementations without a sprite sheet fall
    /// back to `fill_rect` with the appropriate palette color.
    fn draw_cell_sprite(&mut self, rect: PixelRect, cell: Cell);

    /// Fill an opaque rectangle.
    fn fill_rect(&mut self, rect: PixelRect, fill: PaletteKey);

    /// Stroke the 4 edges of a rectangle (1-pixel thick).
    fn stroke_rect(&mut self, rect: PixelRect, stroke: PaletteKey);

    /// Stroke a 1-pixel-tall horizontal line at `(x, y)` extending
    /// `length` pixels right.
    fn stroke_h_line(&mut self, x: i32, y: i32, length: i32, stroke: PaletteKey);

    /// Stroke a 1-pixel-wide vertical line at `(x, y)` extending
    /// `length` pixels down.
    fn stroke_v_line(&mut self, x: i32, y: i32, length: i32, stroke: PaletteKey);

    /// Draw text at the given logical-pixel position with the given
    /// font weight.
    fn text(&mut self, text: &str, x: f32, y: f32, weight: FontWeight, fill: PaletteKey);
}

/// Visual palette references for canvas draws. The macroquad backend
/// resolves keys to concrete colors via [`crate::rendering::theme::Theme`];
/// the recording mock keeps keys verbatim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PaletteKey {
    /// Whole-frame clear color.
    Background,
    /// Board outline + locked garbage cells.
    Structure,
    /// Faint grid base line.
    GridBase,
    /// Brighter grid corner cross. The inner `u8` is the alpha
    /// (`0..=255`); the underlying RGB is fixed by the backend.
    GridCross(u8),
    /// Ghost piece fill.
    Ghost,
    /// HUD text color.
    Text,
    /// Tetromino-specific palette.
    Piece(MinoType),
}

/// Font weight for [`Canvas::text`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontWeight {
    Regular,
    SemiBold,
}

// ---- test-only mock + recording ----

/// Test-only mock [`Canvas`] impl that records every draw call into a
/// `Vec<DrawOp>`.
///
/// Compiled only under `cfg(test)`; production builds never see these
/// items.
#[cfg(test)]
pub mod mock {
    use super::*;

    /// A single recorded draw call.
    #[derive(Clone, Debug, PartialEq)]
    pub enum DrawOp {
        Clear(PaletteKey),
        CellSprite {
            rect: PixelRect,
            cell: Cell,
        },
        FillRect {
            rect: PixelRect,
            fill: PaletteKey,
        },
        StrokeRect {
            rect: PixelRect,
            stroke: PaletteKey,
        },
        StrokeHLine {
            x: i32,
            y: i32,
            length: i32,
            stroke: PaletteKey,
        },
        StrokeVLine {
            x: i32,
            y: i32,
            length: i32,
            stroke: PaletteKey,
        },
        Text {
            text: String,
            x: f32,
            y: f32,
            weight: FontWeight,
            fill: PaletteKey,
        },
    }

    /// [`Canvas`] impl that records every draw call into
    /// `self.calls` for assertions in feature-module tests.
    #[derive(Default)]
    pub struct RecordingCanvas {
        pub calls: Vec<DrawOp>,
    }

    impl Canvas for RecordingCanvas {
        fn clear(&mut self, fill: PaletteKey) {
            self.calls.push(DrawOp::Clear(fill));
        }

        fn draw_cell_sprite(&mut self, rect: PixelRect, cell: Cell) {
            self.calls.push(DrawOp::CellSprite { rect, cell });
        }

        fn fill_rect(&mut self, rect: PixelRect, fill: PaletteKey) {
            self.calls.push(DrawOp::FillRect { rect, fill });
        }

        fn stroke_rect(&mut self, rect: PixelRect, stroke: PaletteKey) {
            self.calls.push(DrawOp::StrokeRect { rect, stroke });
        }

        fn stroke_h_line(&mut self, x: i32, y: i32, length: i32, stroke: PaletteKey) {
            self.calls.push(DrawOp::StrokeHLine {
                x,
                y,
                length,
                stroke,
            });
        }

        fn stroke_v_line(&mut self, x: i32, y: i32, length: i32, stroke: PaletteKey) {
            self.calls.push(DrawOp::StrokeVLine {
                x,
                y,
                length,
                stroke,
            });
        }

        fn text(&mut self, text: &str, x: f32, y: f32, weight: FontWeight, fill: PaletteKey) {
            self.calls.push(DrawOp::Text {
                text: text.to_string(),
                x,
                y,
                weight,
                fill,
            });
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod test {
    use super::mock::{DrawOp, RecordingCanvas};
    use super::*;
    use rstest::rstest;

    #[test]
    fn recording_canvas_clear_records_clear_call() {
        let mut canvas = RecordingCanvas::default();
        canvas.clear(PaletteKey::Background);
        assert_eq!(canvas.calls, vec![DrawOp::Clear(PaletteKey::Background)]);
    }

    #[test]
    fn recording_canvas_fill_rect_records_fill() {
        let mut canvas = RecordingCanvas::default();
        let rect = PixelRect::new(10, 20, 30, 40);
        canvas.fill_rect(rect, PaletteKey::Piece(MinoType::T));
        assert_eq!(
            canvas.calls,
            vec![DrawOp::FillRect {
                rect,
                fill: PaletteKey::Piece(MinoType::T),
            }],
        );
    }

    #[test]
    fn recording_canvas_stroke_rect_records_outline() {
        let mut canvas = RecordingCanvas::default();
        let rect = PixelRect::new(0, 0, 100, 50);
        canvas.stroke_rect(rect, PaletteKey::Structure);
        assert_eq!(
            canvas.calls,
            vec![DrawOp::StrokeRect {
                rect,
                stroke: PaletteKey::Structure,
            }],
        );
    }

    #[test]
    fn recording_canvas_stroke_h_line_records_with_coords() {
        let mut canvas = RecordingCanvas::default();
        canvas.stroke_h_line(5, 10, 200, PaletteKey::GridBase);
        assert_eq!(
            canvas.calls,
            vec![DrawOp::StrokeHLine {
                x: 5,
                y: 10,
                length: 200,
                stroke: PaletteKey::GridBase,
            }],
        );
    }

    #[test]
    fn recording_canvas_stroke_v_line_records_with_coords() {
        let mut canvas = RecordingCanvas::default();
        canvas.stroke_v_line(15, 25, 80, PaletteKey::GridCross(64));
        assert_eq!(
            canvas.calls,
            vec![DrawOp::StrokeVLine {
                x: 15,
                y: 25,
                length: 80,
                stroke: PaletteKey::GridCross(64),
            }],
        );
    }

    #[test]
    fn recording_canvas_text_records_with_cloned_text() {
        let mut canvas = RecordingCanvas::default();
        canvas.text("HOLD", 16.0, 32.0, FontWeight::SemiBold, PaletteKey::Text);
        assert_eq!(
            canvas.calls,
            vec![DrawOp::Text {
                text: "HOLD".to_string(),
                x: 16.0,
                y: 32.0,
                weight: FontWeight::SemiBold,
                fill: PaletteKey::Text,
            }],
        );
    }

    #[test]
    fn recording_canvas_preserves_call_order() {
        let mut canvas = RecordingCanvas::default();
        canvas.clear(PaletteKey::Background);
        canvas.fill_rect(PixelRect::new(0, 0, 10, 10), PaletteKey::Structure);
        canvas.stroke_h_line(0, 0, 5, PaletteKey::GridBase);

        assert_eq!(canvas.calls.len(), 3);
        // Order matters: clear before fill before stroke.
        assert!(matches!(canvas.calls[0], DrawOp::Clear(_)));
        assert!(matches!(canvas.calls[1], DrawOp::FillRect { .. }));
        assert!(matches!(canvas.calls[2], DrawOp::StrokeHLine { .. }));
    }

    #[rstest]
    #[case::piece_i(PaletteKey::Piece(MinoType::I))]
    #[case::piece_t(PaletteKey::Piece(MinoType::T))]
    #[case::grid_cross_alpha_0(PaletteKey::GridCross(0))]
    #[case::grid_cross_alpha_max(PaletteKey::GridCross(255))]
    fn palette_keys_round_trip_through_draw_call(#[case] key: PaletteKey) {
        // Equality through the public surface guards against a
        // phantom-type mistake: if `PaletteKey` had any field that
        // compared unequal after going through the trait method,
        // this would catch it.
        let mut canvas = RecordingCanvas::default();
        canvas.fill_rect(PixelRect::new(0, 0, 1, 1), key);
        assert_eq!(canvas.calls.len(), 1);
        match &canvas.calls[0] {
            DrawOp::FillRect { fill, .. } => assert_eq!(*fill, key),
            other => panic!("expected FillRect, got {other:?}"),
        }
    }
}
