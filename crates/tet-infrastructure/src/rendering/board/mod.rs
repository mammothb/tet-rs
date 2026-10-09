//! Board rendering — outline, locked cells, grid lines.
//!
//! Splits the previous inline `render_board` body into three
//! sub-passes plus a thin orchestrator that draws them in the
//! proper order. The layering rationale (drawn last so the outline
//! overwrites the faint grid color at the rect edges) lives in
//! `outline::draw_outer`'s doc comment.

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

pub mod cells;
pub mod grid;
pub mod outline;

/// Render the playfield: outer outline, locked cells, grid lines,
/// in the order documented by `outline::draw_outer`.
///
/// **Stub** — Phase 11 wires the three sub-passes up.
pub fn draw(_vp: &Viewport, _theme: &Theme, _canvas: &mut dyn Canvas) {
    // No-op until Phase 11 fills the body in.
}
