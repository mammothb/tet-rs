//! Player label above the queue column (e.g. "human", "bot", or
//! a TBP-supplied name).

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::font::Fonts;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;
use tet_application::PlayerView;

/// **Stub** — Phase 14 returns early if `view.label.is_empty()`,
/// else draws `view.label` at the queue column x-offset and the
/// board's bottom-row y via `canvas.text`.
pub fn draw(
    _view: &PlayerView,
    _vp: &Viewport,
    _theme: &Theme,
    _fonts: &Fonts,
    _canvas: &mut dyn Canvas,
) {
    // No-op until Phase 14 fills the body in.
}
