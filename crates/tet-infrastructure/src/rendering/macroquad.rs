//! Macroquad implementation of the `Renderer` port.
//!
//! Drawing happens in screen-pixel coordinates with `y` down (macroquad's
//! convention). The domain is `y` up. The renderer flips `y` once per cell
//! via [`screen_y_offset_cells`] so the application layer never touches the
//! flip.
//!
//! ## No math at draw time
//!
//! Every pixel position comes from [`crate::rendering::layout`]. The layout
//! planner computes integer physical-pixel coordinates for every rect and
//! every line up front. The renderer just iterates the plan and calls
//! `draw_rectangle` with `width=1` or `height=1` for lines — no subpixel
//! math, no half-pixel offsets, no anti-aliasing of edges.

// All integer/float casts in this file are bounded by domain invariants
// (board dims ≤ 128, dpi ≤ 4, cell sizes ≤ 128 logical) — well within
// both `i32` and `f32` mantissa range. Suppress the pedantic warnings.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless
)]

use macroquad::prelude::*;
use tet_application::{Frame, GridStyle, PlayerView, Renderer};
use tet_domain::{Cell, MinoType, Orientation, Ruleset};

use super::layout::{PixelRect, plan_board, plan_box};

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

/// Convert a domain Y coordinate (0 = bottom, growing up) to a y-offset,
/// in cells, from the TOP of a region of the given height.
fn screen_y_offset_cells(domain_y: i8, region_height_cells: i8) -> f32 {
    (region_height_cells - 1 - domain_y) as f32
}

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
}

impl MacroquadRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            dpi_scale: screen_dpi_scale(),
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
            render_view(view, &frame.ruleset, self.dpi_scale);
        }
    }
}

fn render_view(view: &PlayerView, ruleset: &Ruleset, dpi: f32) {
    render_board(view, ruleset, dpi);
    render_active_piece(view, ruleset, dpi);
    if let Some(ghost) = view.ghost {
        render_ghost(view, ruleset, ghost, dpi);
    }
    if view.layout.show_hold {
        render_hold(view, dpi);
    }
    if view.layout.show_queue {
        render_queue(view, ruleset, dpi);
    }
    render_stats(view, ruleset, dpi);
    render_label(view, ruleset, dpi);
}

fn render_board(view: &PlayerView, ruleset: &Ruleset, dpi: f32) {
    let plan = plan_board(view, ruleset, dpi);

    // Draw order matters:
    //   1. Faint grid lines (cover the full board, including the rect edges)
    //   2. Brighter "cross" segments at each grid intersection
    //   3. Outer outline LAST (so the border overwrites the faint grid
    //      color at the rect edges with the brighter STRUCTURE_COLOR)
    let draw_h = matches!(
        view.layout.grid_style,
        GridStyle::Horizontal | GridStyle::Full
    );
    let draw_v = matches!(
        view.layout.grid_style,
        GridStyle::Vertical | GridStyle::Full
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

    // Pass 2: brighter "cross" segments at each grid intersection. To
    // create a visible fade from the dim base line to a brighter corner,
    // we draw multiple overlapping layers per arm. Layers are painted
    // outermost-first (faint, long) so each inner layer (closer to the
    // corner, higher alpha) overwrites the area it covers with a
    // brighter pixel. The result is a gradient: brightest at the corner
    // pixel, fading smoothly outward.
    //
    // At rect-edge corners each arm is clipped to the rect bounds so it
    // never extends past the border.
    if draw_h && draw_v {
        let cell_phys = (view.layout.cell_px * dpi).round() as i32;
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

    // Pass 3: outer outline, drawn last so the border overwrites the
    // faint grid line color at the rect edges with the brighter color.
    draw_outline(plan.outer, STRUCTURE_COLOR);

    // Locked cells. `Board::rows()` yields bottom-to-top; macroquad y is
    // top-to-bottom. Skip buffer rows (y >= visible_rows).
    for (row, board_row) in view
        .snapshot
        .board
        .rows()
        .enumerate()
        .take(plan.outer.h as usize / 24)
    {
        for (col, cell) in board_row.iter().enumerate() {
            let x_phys = plan.outer.x + col as i32 * (plan.outer.w / ruleset.num_cols as i32);
            #[allow(clippy::cast_possible_truncation)]
            let y_offset_cells = screen_y_offset_cells(row as i8, ruleset.num_visible_rows as i8);
            let y_phys = plan.outer.y
                + (y_offset_cells as i32) * (plan.outer.h / ruleset.num_visible_rows as i32);
            let cw = plan.outer.w / ruleset.num_cols as i32;

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
}

fn render_active_piece(view: &PlayerView, ruleset: &Ruleset, dpi: f32) {
    let cell_px_phys = (view.layout.cell_px * dpi).round() as i32;
    let color = piece_color(view.snapshot.current.kind);
    let (ox, oy) = (view.layout.origin.0, view.layout.origin.1);

    for cell_pos in view.snapshot.current.cells() {
        let col = cell_pos.x;
        let row_in_visible = ruleset.num_visible_rows as i8 - 1 - cell_pos.y;
        if row_in_visible < 0 || row_in_visible >= ruleset.num_visible_rows as i8 {
            continue;
        }
        let x_phys = (ox + (col) as f32 * view.layout.cell_px) * dpi;
        let y_phys = (oy + (row_in_visible) as f32 * view.layout.cell_px) * dpi;
        draw_rectangle(
            x_phys.round() / dpi,
            y_phys.round() / dpi,
            cell_px_phys as f32,
            cell_px_phys as f32,
            color,
        );
    }
}

fn render_ghost(view: &PlayerView, ruleset: &Ruleset, ghost: tet_domain::Vec2, dpi: f32) {
    let (ox, oy) = (view.layout.origin.0, view.layout.origin.1);
    let cell_px = view.layout.cell_px;
    let cell_px_phys = (cell_px * dpi).round() as i32;
    let color = piece_color(view.snapshot.current.kind);

    let cur_pos = view.snapshot.current.pos;
    let pos_offset_x = ghost.x - cur_pos.x;
    let pos_offset_y = ghost.y - cur_pos.y;

    for cell_offset in view.snapshot.current.cells() {
        let cell_col = cell_offset.x + pos_offset_x;
        let cell_row_in_visible =
            ruleset.num_visible_rows as i8 - 1 - (cell_offset.y + pos_offset_y);
        if cell_row_in_visible < 0 || cell_row_in_visible >= ruleset.num_visible_rows as i8 {
            continue;
        }
        let pos = (
            ox + (cell_col) as f32 * cell_px,
            oy + (cell_row_in_visible) as f32 * cell_px,
        );
        let plan = plan_box(pos, cell_px_phys, dpi);
        draw_rectangle(
            plan.rect.x as f32,
            plan.rect.y as f32,
            plan.rect.w as f32,
            plan.rect.h as f32,
            GHOST_COLOR,
        );
        draw_outline(plan.rect, color);
    }
}

fn render_hold(view: &PlayerView, dpi: f32) {
    let layout = &view.layout;
    let box_size_phys = ((layout.cell_px * dpi).round() as i32) * HOLD_BOX_CELLS;
    let hold_x = layout.origin.0 - (box_size_phys + 4) as f32;
    let hold_y = layout.origin.1;
    let plan = plan_box((hold_x, hold_y), box_size_phys, dpi);
    draw_outline(plan.rect, STRUCTURE_COLOR);
    draw_text("HOLD", hold_x, hold_y - 4.0, HUD_FONT_SIZE, TEXT_COLOR);

    let Some(kind) = view.snapshot.hold else {
        return;
    };

    let color = piece_color(kind);
    let piece_cell = box_size_phys / HOLD_BOX_CELLS;
    for cell in piece_box_cells(kind, HOLD_BOX_CELLS) {
        let px = hold_x + (cell.0) as f32 * (piece_cell) as f32;
        let py = hold_y + (cell.1) as f32 * (piece_cell) as f32;
        let cell_plan = plan_box((px, py), piece_cell, dpi);
        draw_rectangle(
            cell_plan.rect.x as f32,
            cell_plan.rect.y as f32,
            cell_plan.rect.w as f32,
            cell_plan.rect.h as f32,
            color,
        );
    }
}

fn render_queue(view: &PlayerView, ruleset: &Ruleset, dpi: f32) {
    let layout = &view.layout;
    let cell_phys = (QUEUE_PREVIEW_CELL_PX * dpi).round() as i32;
    let box_phys = cell_phys * HOLD_BOX_CELLS;
    let bw = ((layout.cell_px * dpi).round() as i32) * ruleset.num_cols as i32;
    let bx = (layout.origin.0 * dpi).round() as i32;
    let by = (layout.origin.1 * dpi).round() as i32;
    let qx = bx + bw + (QUEUE_PREVIEW_SPACING * dpi).round() as i32;
    let qy = by;

    draw_text(
        "NEXT",
        layout.origin.0 + layout.cell_px * ruleset.num_cols as f32,
        layout.origin.1 - 4.0,
        HUD_FONT_SIZE,
        TEXT_COLOR,
    );

    let preview_count = ruleset.num_preview.min(view.snapshot.queue.len());
    for i in 0..preview_count {
        let kind = view.snapshot.queue[i];
        let qy_box = qy + i as i32 * (box_phys + (QUEUE_PREVIEW_SPACING * dpi).round() as i32);
        let plan = plan_box((qx as f32, qy_box as f32), box_phys, dpi);
        draw_outline(plan.rect, STRUCTURE_COLOR);
        let color = piece_color(kind);
        let piece_cell = cell_phys;
        for cell in piece_box_cells(kind, HOLD_BOX_CELLS) {
            let px = qx as f32 + (cell.0) as f32 * (piece_cell) as f32;
            let py = qy_box as f32 + (cell.1) as f32 * (piece_cell) as f32;
            let cell_plan = plan_box((px, py), piece_cell, dpi);
            draw_rectangle(
                cell_plan.rect.x as f32,
                cell_plan.rect.y as f32,
                cell_plan.rect.w as f32,
                cell_plan.rect.h as f32,
                color,
            );
        }
    }
}

fn render_stats(view: &PlayerView, ruleset: &Ruleset, _dpi: f32) {
    let layout = &view.layout;
    let (ox, oy) = (layout.origin.0, layout.origin.1);
    let stats_y = oy + layout.cell_px * ruleset.num_visible_rows as f32 + HUD_FONT_SIZE + 4.0;
    let mut line: i32 = 0;
    let mut text = |s: &str| {
        let y_offset = (line) as f32 * HUD_FONT_SIZE;
        draw_text(s, ox, stats_y + y_offset, HUD_FONT_SIZE, TEXT_COLOR);
        line += 1;
    };
    text(&format!("combo: {}", view.snapshot.combo));
    text(&format!("b2b: {}", view.snapshot.b2b));
}

fn render_label(view: &PlayerView, ruleset: &Ruleset, _dpi: f32) {
    if view.label.is_empty() {
        return;
    }
    let layout = &view.layout;
    let (ox, oy) = (layout.origin.0, layout.origin.1);
    let queue_x = ox + layout.cell_px * ruleset.num_cols as f32 + QUEUE_PREVIEW_SPACING;
    let label_y = oy + layout.cell_px * ruleset.num_visible_rows as f32 - 4.0;
    draw_text(view.label, queue_x, label_y, HUD_FONT_SIZE, TEXT_COLOR);
}

/// Cell positions inside an N×N box, top-left origin (matches macroquad
/// rendering). Returns the local (col, row) coordinates for each of the
/// piece's 4 cells, with the Y axis flipped so the piece's TOP (high
/// domain y) appears at the TOP of the box (low screen y).
fn piece_box_cells(kind: MinoType, box_cells: i32) -> [(i32, i32); 4] {
    let box_height = box_cells;
    let coords = kind.coords(Orientation::North);
    [
        (
            coords[0].x as i32,
            screen_y_offset_cells(coords[0].y, box_height as i8) as i32,
        ),
        (
            coords[1].x as i32,
            screen_y_offset_cells(coords[1].y, box_height as i8) as i32,
        ),
        (
            coords[2].x as i32,
            screen_y_offset_cells(coords[2].y, box_height as i8) as i32,
        ),
        (
            coords[3].x as i32,
            screen_y_offset_cells(coords[3].y, box_height as i8) as i32,
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

    // ---- screen_y_offset_cells ----
    //
    // Maps domain Y (0 = bottom, growing up) to a y-offset in cells from
    // the TOP of the region. Inverted so a piece's bottom (domain_y=0)
    // appears at the BOTTOM of the rendered box.

    #[rstest]
    #[case(0_i8, 4_i8, 3.0)] // bottom of domain → bottom of screen (offset 3 in a 4-cell region)
    #[case(1_i8, 4_i8, 2.0)]
    #[case(2_i8, 4_i8, 1.0)]
    #[case(3_i8, 4_i8, 0.0)] // top of domain → top of screen
    fn screen_offset_inverts_domain_y_for_4_cell_region(
        #[case] domain_y: i8,
        #[case] region_cells: i8,
        #[case] expected_offset: f32,
    ) {
        assert_eq!(
            screen_y_offset_cells(domain_y, region_cells),
            expected_offset
        );
    }

    #[rstest]
    #[case(0_i8, 20_i8, 19.0)] // 20 visible rows: bottom = offset 19
    #[case(19_i8, 20_i8, 0.0)] // top row of visible = offset 0
    fn screen_offset_scales_with_region_height(
        #[case] domain_y: i8,
        #[case] region_cells: i8,
        #[case] expected_offset: f32,
    ) {
        assert_eq!(
            screen_y_offset_cells(domain_y, region_cells),
            expected_offset
        );
    }

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
    //   J: [(0,1), (1,1), (2,1), (0,2)] — base at y=1, bump at y=2 x=0
    //   S: [(1,1), (2,1), (0,2), (1,2)]
    //   Z: [(0,1), (1,1), (1,2), (2,2)]
    //
    // With BOX_HEIGHT = 4, screen_y_offset_cells(y, 4) = 3 - y. So:
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
