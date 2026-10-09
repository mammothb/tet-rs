//! HUD overlay — stats below the board, player label above the
//! queue column.

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::font::Fonts;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;
use tet_application::PlayerView;

pub mod label;
pub mod stats;

/// Render the stats line and player label. Both gated by
/// `Layout::show_stats` and `Layout::show_label` (added in
/// Phase 14).
///
/// **Stub** — Phase 14 delegates to `stats::draw` and `label::draw`.
pub fn draw(
    _view: &PlayerView,
    _vp: &Viewport,
    _theme: &Theme,
    _fonts: &Fonts,
    _canvas: &mut dyn Canvas,
) {
    // No-op until Phase 14 fills the body in.
}
