//! Side panels: hold (left) and queue (right).
//!
//! Gated by `vp.layout().show_hold` / `vp.layout().show_queue`.

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::font::Fonts;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

pub mod hold;
pub mod queue;

/// Render whichever side panels are enabled for this view.
///
/// **Stub** — Phase 13 wires `hold::draw` and `queue::draw` behind
/// `show_hold` / `show_queue` feature flags.
pub fn draw(_vp: &Viewport, _theme: &Theme, _fonts: &Fonts, _canvas: &mut dyn Canvas) {
    // No-op until Phase 13 fills the body in.
}
