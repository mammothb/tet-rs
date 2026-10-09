//! Macroquad implementation of the `Renderer` port.
//!
//! Drawing happens in screen-pixel coordinates with `y` down (macroquad's
//! convention). The domain is `y` up. The renderer flips `y` once per cell
//! via [`crate::rendering::viewport::domain_to_screen_row`] so the
//! application layer never touches the flip.
//!
//! ## No math at draw time
//!
//! Every pixel position comes from either the layout planner
//! ([`crate::rendering::layout`]) or the [`Viewport`] helpers. The
//! renderer just iterates the plan and calls `draw_rectangle` with
//! `width=1` or `height=1` for lines — no subpixel math, no half-pixel
//! offsets, no anti-aliasing of edges.
//!
//! Per-view data (`PlayerSnapshot`, `Layout`, `Ruleset`, `dpi`) is
//! bundled once at the top of `render_view` into a `Viewport`; helper
//! functions read pixel positions from `&Viewport` so the `(x * dpi)
//! round() as i32` duplication disappears.
//!
//! ## Fonts
//!
//! Fonts are loaded once at startup via [`Fonts`]
//! (embedded via `include_bytes!`). HUD text uses Regular; prominent
//! labels (`HOLD`, `NEXT`, player names) use `SemiBold`.

// All integer/float casts in this file are bounded by domain invariants
// (board dims ≤ 128, dpi ≤ 4, cell sizes ≤ 128 logical) — well within
// both `i32` and `f32` mantissa range.
//
// `clippy::similar_names` is also allowed at file scope: every
// feature module pairs `x` and `y` for rect coordinates, and finding
// non-overlapping name pairs (without renaming the whole concept)
// produces more obfuscation than clarity.
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::similar_names
)]

use macroquad::prelude::*;
use tet_application::{Frame, PlayerView, Renderer};
use tet_domain::{Cell, MinoType, Orientation, Ruleset};

use crate::rendering::font::Fonts;
use crate::rendering::layout::{PixelRect, plan_board};
use crate::rendering::viewport::{Viewport, domain_to_screen_row};

/// Color for locked garbage cells and the board outline. Bright enough
/// to be clearly visible on the black background.
const STRUCTURE_COLOR: Color = Color::new(80.0 / 255.0, 80.0 / 255.0, 80.0 / 255.0, 1.0);

/// Base grid line color. Dim against the black background — the "background"
/// brightness of the grid (visible in the middle of each cell edge).
const GRID_BASE_COLOR: Color = Color::new(20.0 / 255.0, 20.0 / 255.0, 20.0 / 255.0, 1.0);

/// Brighter color (with alpha) for the "cross" segments at each grid
/// intersection. Alpha < 1 lets the segments blend with the base line
/// underneath, so the transition from dim to bright is gradual rather
/// than a hard edge. Two perpendicular arms overlap at the corner, which
/// gives an even brighter pixel there. The actual alpha is overridden
/// per-layer (see the `layers` array in `render_board`).
const GRID: Color = Color::new(60.0 / 255.0, 60.0 / 255.0, 60.0 / 255.0, 1.0);

/// Faint white for the ghost piece outline. Reduces alpha so the
/// locked cells underneath remain visible.
const GHOST_COLOR: Color = Color::new(1.0, 1.0, 1.0, 0.25);

/// White text for HUD elements.
const TEXT_COLOR: Color = WHITE;

const HUD_FONT_SIZE: f32 = 18.0;
const HOLD_BOX_CELLS: i32 = 4;
const QUEUE_PREVIEW_CELL_PX: f32 = 16.0;
const QUEUE_PREVIEW_SPACING: f32 = 4.0;
/// Vertical offset from the nearest edge for labels (HOLD, NEXT, stats,
/// player name). The text baseline sits at this offset above the edge,
/// so the text ascends into the available space above.
///
/// **Must stay small** — the cap height at font size 18 extends ~14px
/// above the baseline. The window title bar clips the top of the canvas;
/// at `LABEL_PADDING = 8` the cap height went above the title bar and
/// got cut off. `0.0` keeps it below.
const LABEL_PADDING: f32 = 0.0;

/// Draw a 1-pixel-tall horizontal line as a filled rectangle covering
/// pixel row `y_phys` from column `x_phys` to `x_phys + width_phys - 1`.
fn draw_h_line(x_phys: i32, y_phys: i32, width_phys: i32, color: Color) {
    draw_rectangle(x_phys as f32, y_phys as f32, width_phys as f32, 1.0, color);
}

/// Draw a 1-pixel-wide vertical line as a filled rectangle covering
/// pixel column `x_phys` from row `y_phys` to `y_phys + height_phys - 1`.
fn draw_v_line(x_phys: i32, y_phys: i32, height_phys: i32, color: Color) {
    draw_rectangle(x_phys as f32, y_phys as f32, 1.0, height_phys as f32, color);
}

/// Draw text at the given logical position with the given font. Wraps
/// `draw_text_ex` so the call sites don't need to construct
/// `TextParams`. Positions are in **logical pixels** — the existing
/// render label code paths pass logical-pixel floats and macroquad
/// accepts that scale; we preserve that here.
fn draw_label(text: &str, x: f32, y: f32, font_size: u16, font: &Font, color: Color) {
    draw_text_ex(
        text,
        x,
        y,
        TextParams {
            font: Some(font),
            font_size,
            font_scale: 1.0,
            font_scale_aspect: 1.0,
            color,
            rotation: 0.0,
        },
    );
}

/// Draw a 4-edge outline of a `PixelRect`. Each edge is a 1-pixel-thick
/// filled rectangle at integer physical-pixel positions.
fn draw_outline(rect: PixelRect, color: Color) {
    draw_h_line(rect.x, rect.y, rect.w, color);
    draw_h_line(rect.x, rect.bottom_pixel(), rect.w, color);
    draw_v_line(rect.x, rect.y, rect.h, color);
    draw_v_line(rect.right_pixel(), rect.y, rect.h, color);
}

pub struct MacroquadRenderer {
    dpi_scale: f32,
    fonts: Fonts,
}

impl MacroquadRenderer {
    /// Load fonts from the embedded bytes.
    ///
    /// # Panics
    ///
    /// Panics if the embedded TTF bytes are malformed. The shipped bytes
    /// are well-formed so this should not happen in practice.
    #[must_use]
    pub fn new() -> Self {
        let fonts = Fonts::load().expect("Fira Code fonts are embedded in the binary");
        Self {
            dpi_scale: screen_dpi_scale(),
            fonts,
        }
    }
}

impl Default for MacroquadRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for MacroquadRenderer {
    fn render(&mut self, frame: &Frame) {
        clear_background(BLACK);
        for view in &frame.views {
            render_view(view, &frame.ruleset, self.dpi_scale, &self.fonts);
        }
    }
}

/// Build a [`Viewport`] from a `PlayerView` and dispatch to per-feature
/// renderers. The dispatch order matters: outline + grid behind locked
/// cells, hold/queue around the board, HUD on top.
fn render_view(view: &PlayerView, ruleset: &Ruleset, dpi: f32, fonts: &Fonts) {
    let vp = Viewport::new(&view.snapshot, &view.layout, ruleset, dpi);

    render_board(view, &vp);
    render_active_piece(&vp);
    if let Some(ghost) = view.ghost {
        render_ghost(&vp, ghost);
    }
    if vp.layout().show_hold {
        render_hold(&vp, fonts);
    }
    if vp.layout().show_queue {
        render_queue(&vp, fonts);
    }
    render_stats(&vp, fonts);
    render_label(view, &vp, fonts);
}

fn render_board(view: &PlayerView, vp: &Viewport) {
    let plan = plan_board(view, vp.ruleset(), vp.dpi());

    // Draw order matters:
    //   1. Faint grid lines (cover the full board, including the rect edges)
    //   2. Brighter "cross" segments at each grid intersection
    //   3. Locked cells
    //   4. Outer outline LAST (overwrites the faint grid color at the
    //      rect edges with the brighter STRUCTURE_COLOR).
    let draw_h = matches!(
        vp.layout().grid_style,
        tet_application::GridStyle::Horizontal | tet_application::GridStyle::Full
    );
    let draw_v = matches!(
        vp.layout().grid_style,
        tet_application::GridStyle::Vertical | tet_application::GridStyle::Full
    );

    // Pass 1: faint base lines spanning the whole board.
    if draw_h {
        for &row_phys in &plan.h_line_rows {
            draw_h_line(
                plan.outer.x,
                row_phys,
                plan.grid_line_length_phys,
                GRID_BASE_COLOR,
            );
        }
    }
    if draw_v {
        for &col_phys in &plan.v_line_cols {
            draw_v_line(col_phys, plan.outer.y, plan.outer.h, GRID_BASE_COLOR);
        }
    }

    // Pass 2: brighter "cross" segments at each grid intersection.
    //
    // To create a visible fade from the dim base line to a brighter
    // corner, we draw multiple overlapping layers per arm. Layers are
    // painted outermost-first (faint, long) so each inner layer
    // (closer to the corner, higher alpha) overwrites the area it
    // covers with a brighter pixel. The result is a gradient:
    // brightest at the corner pixel, fading smoothly outward.
    //
    // At rect-edge corners each arm is clipped to the rect bounds so
    // it never extends past the border.
    if draw_h && draw_v {
        let cell_phys = vp.cell_px_phys();
        let x_lo = plan.outer.x;
        let x_hi = plan.outer.right_pixel() + 1; // exclusive
        let y_lo = plan.outer.y;
        let y_hi = plan.outer.bottom_pixel() + 1; // exclusive
        // Each entry: (reach_in_phys, fading_alpha). Outermost first.
        // The innermost (shortest reach, highest alpha) wins at the corner.
        let layers: &[(i32, f32)] = &[
            (cell_phys / 3, 0.20),
            (cell_phys / 6, 0.40),
            (cell_phys / 12, 0.65),
            (cell_phys / 24, 0.95),
        ];

        for &y_phys in &plan.h_line_rows {
            for &x_phys in &plan.v_line_cols {
                // Horizontal arm
                for &(reach, alpha) in layers {
                    let h_start = (x_phys - reach).max(x_lo);
                    let h_end = (x_phys + reach).min(x_hi);
                    if h_end > h_start {
                        let c = Color {
                            r: GRID.r,
                            g: GRID.g,
                            b: GRID.b,
                            a: alpha,
                        };
                        draw_h_line(h_start, y_phys, h_end - h_start, c);
                    }
                }
                // Vertical arm
                for &(reach, alpha) in layers {
                    let v_start = (y_phys - reach).max(y_lo);
                    let v_end = (y_phys + reach).min(y_hi);
                    if v_end > v_start {
                        let c = Color {
                            r: GRID.r,
                            g: GRID.g,
                            b: GRID.b,
                            a: alpha,
                        };
                        draw_v_line(x_phys, v_start, v_end - v_start, c);
                    }
                }
            }
        }
    }

    // Pass 3: locked cells. `Board::rows()` yields bottom-to-top;
    // `Viewport::cell_pixel` flips the Y axis. Skip buffer rows above
    // the visible window (`num_visible_rows`).
    let cw = vp.cell_px_phys();
    let visible_rows = vp.ruleset().num_visible_rows;
    for (row, board_row) in vp.snapshot().board.rows().enumerate().take(visible_rows) {
        for (col, cell) in board_row.iter().enumerate() {
            let (x_phys, y_phys) = vp.cell_pixel(col as i32, row as i32);
            match cell {
                Cell::Empty => {}
                Cell::Block(t) => {
                    draw_rectangle(
                        x_phys as f32,
                        y_phys as f32,
                        cw as f32,
                        cw as f32,
                        piece_color(*t),
                    );
                }
                Cell::Garbage => {
                    draw_rectangle(
                        x_phys as f32,
                        y_phys as f32,
                        cw as f32,
                        cw as f32,
                        STRUCTURE_COLOR,
                    );
                }
            }
        }
    }

    // Pass 4: outer outline, drawn last so the border overwrites the
    // faint grid line color at the rect edges with the brighter color.
    draw_outline(plan.outer, STRUCTURE_COLOR);
}

fn render_active_piece(vp: &Viewport) {
    let color = piece_color(vp.snapshot().current.kind);
    let cw = vp.cell_px_phys();
    let visible_rows = vp.ruleset().num_visible_rows as i32;

    for cell_pos in vp.snapshot().current.cells() {
        let col = i32::from(cell_pos.x);
        let domain_row = i32::from(cell_pos.y);
        if domain_row < 0 || domain_row >= visible_rows {
            continue;
        }
        let (x_phys, y_phys) = vp.cell_pixel(col, domain_row);
        draw_rectangle(x_phys as f32, y_phys as f32, cw as f32, cw as f32, color);
    }
}

fn render_ghost(vp: &Viewport, ghost: tet_domain::Vec2) {
    let color = piece_color(vp.snapshot().current.kind);
    let cur_pos = vp.snapshot().current.pos;
    let pos_offset_x = i32::from(ghost.x) - i32::from(cur_pos.x);
    let pos_offset_y = i32::from(ghost.y) - i32::from(cur_pos.y);
    let visible_rows = vp.ruleset().num_visible_rows as i32;
    let cw = vp.cell_px_phys();

    for cell_offset in vp.snapshot().current.cells() {
        let cell_col = i32::from(cell_offset.x) + pos_offset_x;
        let cell_domain_row = i32::from(cell_offset.y) + pos_offset_y;
        if cell_domain_row < 0 || cell_domain_row >= visible_rows {
            continue;
        }
        let (x_phys, y_phys) = vp.cell_pixel(cell_col, cell_domain_row);
        draw_rectangle(
            x_phys as f32,
            y_phys as f32,
            cw as f32,
            cw as f32,
            GHOST_COLOR,
        );
        draw_outline(PixelRect::new(x_phys, y_phys, cw, cw), color);
    }
}

fn render_hold(vp: &Viewport, fonts: &Fonts) {
    let layout = vp.layout();
    let (ox, oy) = vp.origin_pixel();
    let cw = vp.cell_px_phys();
    let box_size = cw * HOLD_BOX_CELLS;
    let hold_x_phys = ox - (box_size + 4);
    let hold_y_phys = oy;

    // Box outline + filled piece cells use integer physical pixels.
    draw_outline(
        PixelRect::new(hold_x_phys, hold_y_phys, box_size, box_size),
        STRUCTURE_COLOR,
    );

    // Label sits above the box. Logical f32 coords (matches existing
    // label rendering — `draw_label` historically took pre-DPI floats).
    draw_label(
        "HOLD",
        layout.origin.0 - (box_size + 4) as f32,
        (hold_y_phys as f32) - LABEL_PADDING,
        HUD_FONT_SIZE as u16,
        &fonts.semibold,
        TEXT_COLOR,
    );

    let Some(kind) = vp.snapshot().hold else {
        return;
    };

    let color = piece_color(kind);
    let piece_cell = box_size / HOLD_BOX_CELLS;
    for (cell_x, cell_y) in piece_box_cells(kind, HOLD_BOX_CELLS) {
        let px_phys = hold_x_phys + cell_x * piece_cell;
        let py_phys = hold_y_phys + cell_y * piece_cell;
        draw_rectangle(
            px_phys as f32,
            py_phys as f32,
            piece_cell as f32,
            piece_cell as f32,
            color,
        );
    }
}

fn render_queue(vp: &Viewport, fonts: &Fonts) {
    let layout = vp.layout();
    let ruleset = vp.ruleset();
    let (ox, oy) = vp.origin_pixel();
    let cw = vp.cell_px_phys();
    // Queue preview cells are sized independently of the board's
    // cell_px. They're drawn in a column to the right of the board.
    let cell_phys = (QUEUE_PREVIEW_CELL_PX * vp.dpi()).round() as i32;
    let box_phys = cell_phys * HOLD_BOX_CELLS;
    let bw = cw * ruleset.num_cols as i32;
    let spacing_phys = (QUEUE_PREVIEW_SPACING * vp.dpi()).round() as i32;
    let qx_phys = ox + bw + spacing_phys;
    let qy_phys = oy;

    // NEXT label — logical f32, see comment on `draw_label`.
    draw_label(
        "NEXT",
        layout.origin.0 + layout.cell_px * ruleset.num_cols as f32,
        (qy_phys as f32) - LABEL_PADDING,
        HUD_FONT_SIZE as u16,
        &fonts.semibold,
        TEXT_COLOR,
    );

    let preview_count = ruleset.num_preview.min(vp.snapshot().queue.len());
    for i in 0..preview_count {
        let kind = vp.snapshot().queue[i];
        let qy_box_phys = qy_phys + i as i32 * (box_phys + spacing_phys);
        draw_outline(
            PixelRect::new(qx_phys, qy_box_phys, box_phys, box_phys),
            STRUCTURE_COLOR,
        );
        let color = piece_color(kind);
        let piece_cell = cell_phys;
        for (cell_x, cell_y) in piece_box_cells(kind, HOLD_BOX_CELLS) {
            let px = qx_phys + cell_x * piece_cell;
            let py = qy_box_phys + cell_y * piece_cell;
            draw_rectangle(
                px as f32,
                py as f32,
                piece_cell as f32,
                piece_cell as f32,
                color,
            );
        }
    }
}

fn render_stats(vp: &Viewport, fonts: &Fonts) {
    let layout = vp.layout();
    let ruleset = vp.ruleset();
    let (ox, oy) = (layout.origin.0, layout.origin.1);
    // Logical f32 throughout — stats text uses the same pre-DPI
    // convention as `draw_label` (see `render_hold`).
    let stats_y =
        oy + layout.cell_px * ruleset.num_visible_rows as f32 + HUD_FONT_SIZE + LABEL_PADDING;
    let mut line: i32 = 0;
    let mut text = |s: &str| {
        let y_offset = (line) as f32 * HUD_FONT_SIZE;
        draw_label(
            s,
            ox,
            stats_y + y_offset,
            HUD_FONT_SIZE as u16,
            &fonts.regular,
            TEXT_COLOR,
        );
        line += 1;
    };
    text(&format!("combo: {}", vp.snapshot().combo));
    text(&format!("b2b: {}", vp.snapshot().b2b));
}

/// `PlayerView::label` lives on `PlayerView`, not `PlayerSnapshot` —
/// not surfaced via the `Viewport` since it isn't a render-layout
/// concern. The `view` parameter here is just for that one accessor.
fn render_label(view: &PlayerView, vp: &Viewport, fonts: &Fonts) {
    if view.label.is_empty() {
        return;
    }
    let layout = view.layout;
    let ruleset = vp.ruleset();
    let ox = layout.origin.0;
    let oy = layout.origin.1;
    let queue_x = ox + layout.cell_px * ruleset.num_cols as f32 + QUEUE_PREVIEW_SPACING;
    // Logical f32 — matches existing label-rendering convention.
    let label_y = oy + layout.cell_px * ruleset.num_visible_rows as f32 - LABEL_PADDING;
    draw_label(
        view.label,
        queue_x,
        label_y,
        HUD_FONT_SIZE as u16,
        &fonts.semibold,
        TEXT_COLOR,
    );
}

/// Cell positions inside an N×N box, top-left origin (matches macroquad
/// rendering). Returns the local (col, row) coordinates for each of the
/// piece's 4 cells, with the Y axis flipped so the piece's TOP (high
/// domain y) appears at the TOP of the box (low screen y).
///
/// **Migration target:** replaced by [`crate::rendering::piece_box::PieceBox`]
/// in Phase 6 of `PLAN-renderer-refactor.md`. Kept here until then.
fn piece_box_cells(kind: MinoType, box_cells: i32) -> [(i32, i32); 4] {
    let box_height = box_cells;
    let coords = kind.coords(Orientation::North);
    [
        (
            coords[0].x as i32,
            domain_to_screen_row(coords[0].y as i32, box_height),
        ),
        (
            coords[1].x as i32,
            domain_to_screen_row(coords[1].y as i32, box_height),
        ),
        (
            coords[2].x as i32,
            domain_to_screen_row(coords[2].y as i32, box_height),
        ),
        (
            coords[3].x as i32,
            domain_to_screen_row(coords[3].y as i32, box_height),
        ),
    ]
}

fn piece_color(t: MinoType) -> Color {
    match t {
        MinoType::I => Color::new(0.0, 0.85, 0.85, 1.0), // cyan
        MinoType::O => Color::new(0.95, 0.85, 0.0, 1.0), // yellow
        MinoType::T => Color::new(0.55, 0.0, 0.55, 1.0), // purple
        MinoType::S => Color::new(0.0, 0.85, 0.0, 1.0),  // green
        MinoType::Z => Color::new(0.85, 0.0, 0.0, 1.0),  // red
        MinoType::J => Color::new(0.0, 0.0, 0.85, 1.0),  // blue
        MinoType::L => Color::new(0.95, 0.55, 0.0, 1.0), // orange
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod test {
    use super::*;
    use rstest::rstest;

    // ---- piece_box_cells ----
    //
    // Maps each piece's 4 cells (in domain space, bottom-up) into screen
    // box coordinates (top-down). Y is inverted so the piece's TOP tip
    // appears at the TOP of the rendered box.
    //
    // Domain coords for North orientation (from `MinoType::coords`):
    //   T: [(0,1), (1,1), (2,1), (1,2)]   — base at y=1, tip at y=2
    //   I: [(0,2), (1,2), (2,2), (3,2)]   — flat at y=2
    //   O: [(1,1), (2,1), (1,2), (2,2)]   — square
    //   L: [(0,1), (1,1), (2,1), (2,2)]
    //   J: [(0,1), (1,1), (2,1), (0,2)]   — base at y=1, bump at y=2 x=0
    //   S: [(1,1), (2,1), (0,2), (1,2)]
    //   Z: [(0,1), (1,1), (1,2), (2,2)]
    //
    // With BOX_HEIGHT = 4, screen y = 3 - domain_y. So:
    //   domain y=0 → screen y=3 (bottom of box)
    //   domain y=1 → screen y=2
    //   domain y=2 → screen y=1 (near top)
    //   domain y=3 → screen y=0 (top of box)

    #[rstest]
    #[case(MinoType::T, 1)] // (0,2): tip at y=2 in domain → y=1 in box (near top)
    #[case(MinoType::T, 2)] // base cells (0,1), (1,1), (2,1) all → y=2
    #[case(MinoType::I, 1)] // (1,2) — vertical middle of box
    #[case(MinoType::L, 1)] // (2,2) — corner piece's tall end
    #[case(MinoType::J, 1)] // (1,1) — J base cell → y=2
    fn piece_box_y_is_domain_y_inverted(#[case] kind: MinoType, #[case] cell_idx: usize) {
        let cells = piece_box_cells(kind, 4);
        let coords = kind.coords(Orientation::North);
        let expected_screen_y = 3_i32 - (coords[cell_idx].y as i32);
        assert_eq!(cells[cell_idx].1, expected_screen_y);
    }

    #[rstest]
    #[case(MinoType::T, 3, 1, 1)] // (1,2) → (1, 1): tip at top of box
    #[case(MinoType::I, 0, 0, 1)] // (0,2) → (0, 1): leftmost I cell
    #[case(MinoType::O, 0, 1, 2)] // (1,1) → (1, 2): O is a 2x2 square
    #[case(MinoType::O, 3, 2, 1)] // (2,2) → (2, 1): top-right of O
    #[case(MinoType::J, 3, 0, 1)] // (0,2) → (0, 1): J's top bump at the very top-left
    fn piece_box_specific_cells(
        #[case] kind: MinoType,
        #[case] cell_idx: usize,
        #[case] expected_x: i32,
        #[case] expected_y: i32,
    ) {
        let cells = piece_box_cells(kind, 4);
        assert_eq!(cells[cell_idx], (expected_x, expected_y));
    }

    #[rstest]
    fn piece_box_returns_exactly_4_cells() {
        for kind in [
            MinoType::T,
            MinoType::I,
            MinoType::O,
            MinoType::S,
            MinoType::Z,
            MinoType::J,
            MinoType::L,
        ] {
            assert_eq!(piece_box_cells(kind, 4).len(), 4);
        }
    }
}
