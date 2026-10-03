//! Macroquad implementation of the `Renderer` port.
//!
//! Drawing happens in screen-pixel coordinates with `y` down (macroquad's
//! convention). The domain is `y` up. The renderer flips `y` once per cell
//! via [`screen_y_offset_cells`] so the application layer never touches the flip.

use macroquad::prelude::*;
use tet_application::{Frame, PlayerView, Renderer};
use tet_domain::{Cell, MinoType, Orientation, Ruleset};

/// Color for locked garbage cells, grid lines, and the board outline.
const STRUCTURE_COLOR: Color = GRAY;

/// Faint white for the ghost piece outline. Reduces alpha so the
/// locked cells underneath remain visible.
const GHOST_COLOR: Color = Color::new(1.0, 1.0, 1.0, 0.25);

/// White text for HUD elements.
const TEXT_COLOR: Color = WHITE;

const HUD_FONT_SIZE: f32 = 18.0;
const HOLD_BOX_CELLS: f32 = 4.0;
const QUEUE_PREVIEW_CELL_PX: f32 = 16.0;
const QUEUE_PREVIEW_SPACING: f32 = 4.0;

/// Convert a domain Y coordinate (0 = bottom, growing up) to a y-offset,
/// in cells, from the TOP of a region of the given height.
///
/// `region_height_cells` is the total cell-count of the region being drawn
/// (e.g. `ruleset.num_rows` for the board, 4 for a 4×4 preview box).
/// Returns 0 for the topmost domain row, growing downward.
fn screen_y_offset_cells(domain_y: i8, region_height_cells: i8) -> f32 {
    f32::from(region_height_cells - 1 - domain_y)
}

pub struct MacroquadRenderer;

impl Renderer for MacroquadRenderer {
    fn render(&mut self, frame: &Frame) {
        clear_background(BLACK);
        for view in &frame.views {
            render_view(view, &frame.ruleset);
        }
    }
}

fn render_view(view: &PlayerView, ruleset: &Ruleset) {
    render_board(view, ruleset);
    render_active_piece(view, ruleset);
    if let Some(ghost) = view.ghost {
        render_ghost(view, ruleset, ghost);
    }
    if view.layout.show_hold {
        render_hold(view);
    }
    if view.layout.show_queue {
        render_queue(view, ruleset);
    }
    render_stats(view, ruleset);
    render_label(view, ruleset);
}

fn render_board(view: &PlayerView, ruleset: &Ruleset) {
    let layout = &view.layout;
    let (x0, y0) = layout.origin;
    let cell_px = layout.cell_px;
    let cols = ruleset.num_cols;
    let rows = ruleset.num_rows;
    let board_w = layout.board_w(cols);
    let board_h = layout.board_h(rows);

    // Outline.
    draw_rectangle_lines(x0, y0, board_w, board_h, 1.0, STRUCTURE_COLOR);

    // Locked cells. `Board::rows()` yields bottom-to-top; macroquad y is
    // top-to-bottom. Flip via `screen_y_offset_cells`.
    for (y, row) in view.snapshot.board.rows().enumerate() {
        for (x, cell) in row.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let screen_x = x0 + x as f32 * cell_px;
            // Board dimensions are bounded (< 128), so the usize → i8 cast is safe.
            #[allow(clippy::cast_possible_truncation)]
            let y_offset = screen_y_offset_cells(y as i8, rows as i8);
            let screen_y = y0 + y_offset * cell_px;

            match cell {
                Cell::Empty => {
                    if layout.show_grid {
                        draw_rectangle_lines(
                            screen_x,
                            screen_y,
                            cell_px,
                            cell_px,
                            0.5,
                            STRUCTURE_COLOR,
                        );
                    }
                }
                Cell::Block(t) => {
                    draw_rectangle(screen_x, screen_y, cell_px, cell_px, piece_color(*t));
                }
                Cell::Garbage => {
                    draw_rectangle(screen_x, screen_y, cell_px, cell_px, STRUCTURE_COLOR);
                }
            }
        }
    }
}

fn render_active_piece(view: &PlayerView, ruleset: &Ruleset) {
    let layout = &view.layout;
    let (x0, y0) = layout.origin;
    let cell_px = layout.cell_px;
    let color = piece_color(view.snapshot.current.kind);

    for cell_pos in view.snapshot.current.cells() {
        #[allow(clippy::cast_precision_loss)]
        let screen_x = x0 + f32::from(cell_pos.x) * cell_px;
        // Board dimensions bounded; safe to truncate.
        #[allow(clippy::cast_possible_truncation)]
        let y_offset = screen_y_offset_cells(cell_pos.y, ruleset.num_rows as i8);
        let screen_y = y0 + y_offset * cell_px;
        draw_rectangle(screen_x, screen_y, cell_px, cell_px, color);
    }
}

fn render_ghost(view: &PlayerView, ruleset: &Ruleset, ghost: tet_domain::Vec2) {
    let layout = &view.layout;
    let (x0, y0) = layout.origin;
    let cell_px = layout.cell_px;
    let color = piece_color(view.snapshot.current.kind);

    for cell_offset in view.snapshot.current.cells() {
        // `cells()` returns positions relative to `Piece::pos`. Ghost pos
        // replaces `pos` but the cell offsets stay the same.
        let cell_pos = tet_domain::Vec2::new(
            ghost.x + (cell_offset.x - view.snapshot.current.pos.x),
            ghost.y + (cell_offset.y - view.snapshot.current.pos.y),
        );
        let screen_x = x0 + f32::from(cell_pos.x) * cell_px;
        // Board dimensions bounded; safe to truncate.
        #[allow(clippy::cast_possible_truncation)]
        let y_offset = screen_y_offset_cells(cell_pos.y, ruleset.num_rows as i8);
        let screen_y = y0 + y_offset * cell_px;
        draw_rectangle(screen_x, screen_y, cell_px, cell_px, GHOST_COLOR);
        // Trace the piece color on the edges so the ghost is recognizable.
        draw_rectangle_lines(screen_x, screen_y, cell_px, cell_px, 1.0, color);
    }
}

fn render_hold(view: &PlayerView) {
    let layout = &view.layout;
    let (x0, y0) = layout.origin;
    let box_size = cell_px_for_box(layout.cell_px) * HOLD_BOX_CELLS;
    let hold_x = x0 - box_size - QUEUE_PREVIEW_SPACING;
    let hold_y = y0;

    // Box.
    draw_rectangle_lines(hold_x, hold_y, box_size, box_size, 1.0, STRUCTURE_COLOR);
    draw_text("HOLD", hold_x, hold_y - 4.0, HUD_FONT_SIZE, TEXT_COLOR);

    let Some(kind) = view.snapshot.hold else {
        return;
    };

    // Center the piece in the hold box. Cell offsets relative to top-left
    // of the 4-cell box.
    let color = piece_color(kind);
    let piece_cell = box_size / HOLD_BOX_CELLS;
    for cell in piece_box_cells(kind) {
        let px = hold_x + cell.0 * piece_cell;
        let py = hold_y + cell.1 * piece_cell;
        draw_rectangle(px, py, piece_cell, piece_cell, color);
    }
}

fn render_queue(view: &PlayerView, ruleset: &tet_domain::Ruleset) {
    let layout = &view.layout;
    let (x0, y0) = layout.origin;
    let cell = QUEUE_PREVIEW_CELL_PX;
    let queue_x = x0 + layout.board_w(ruleset.num_cols) + QUEUE_PREVIEW_SPACING;
    let queue_y = y0;

    draw_text("NEXT", queue_x, queue_y - 4.0, HUD_FONT_SIZE, TEXT_COLOR);

    let preview = view.snapshot.queue.iter().take(ruleset.num_preview);
    for (i, kind) in preview.enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let y = queue_y + i as f32 * (cell * 4.0 + QUEUE_PREVIEW_SPACING);
        draw_rectangle_lines(queue_x, y, cell * 4.0, cell * 4.0, 1.0, STRUCTURE_COLOR);
        let color = piece_color(*kind);
        for cell_pos in piece_box_cells(*kind) {
            let px = queue_x + cell_pos.0 * cell;
            let py = y + cell_pos.1 * cell;
            draw_rectangle(px, py, cell, cell, color);
        }
    }
}

fn render_stats(view: &PlayerView, ruleset: &Ruleset) {
    let layout = &view.layout;
    let (x0, y0) = layout.origin;
    let stats_y = y0 + layout.board_h(ruleset.num_rows) + HUD_FONT_SIZE + 4.0;
    let mut line: i32 = 0;
    let mut text = |s: &str| {
        #[allow(clippy::cast_precision_loss)]
        let y_offset = line as f32 * HUD_FONT_SIZE;
        draw_text(s, x0, stats_y + y_offset, HUD_FONT_SIZE, TEXT_COLOR);
        line += 1;
    };
    text(&format!("combo: {}", view.snapshot.combo));
    text(&format!("b2b: {}", view.snapshot.b2b));
}

fn render_label(view: &PlayerView, ruleset: &Ruleset) {
    if view.label.is_empty() {
        return;
    }
    let layout = &view.layout;
    let (x0, y0) = layout.origin;
    let queue_x = x0 + layout.board_w(ruleset.num_cols) + QUEUE_PREVIEW_SPACING;
    let label_y = y0 + layout.board_h(ruleset.num_rows) - 4.0;
    draw_text(view.label, queue_x, label_y, HUD_FONT_SIZE, TEXT_COLOR);
}

fn cell_px_for_box(board_cell_px: f32) -> f32 {
    // Hold box uses 60% of board cell size so pieces look proportional.
    board_cell_px * 0.6
}

/// Cell positions inside a 4×4 preview/hold box, top-left origin (matches
/// macroquad rendering). Returns the local (col, row) coordinates for each
/// of the piece's 4 cells, with the Y axis flipped (via
/// `screen_y_offset_cells`) so the piece's TOP (high domain y) appears at
/// the TOP of the box (low screen y).
fn piece_box_cells(kind: MinoType) -> [(f32, f32); 4] {
    const BOX_HEIGHT: i8 = 4;
    let coords = kind.coords(Orientation::North);
    [
        (
            f32::from(coords[0].x),
            screen_y_offset_cells(coords[0].y, BOX_HEIGHT),
        ),
        (
            f32::from(coords[1].x),
            screen_y_offset_cells(coords[1].y, BOX_HEIGHT),
        ),
        (
            f32::from(coords[2].x),
            screen_y_offset_cells(coords[2].y, BOX_HEIGHT),
        ),
        (
            f32::from(coords[3].x),
            screen_y_offset_cells(coords[3].y, BOX_HEIGHT),
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
