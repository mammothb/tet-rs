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
use tet_domain::Ruleset;

use crate::rendering::backend::canvas::{Canvas, FontWeight, PaletteKey};
use crate::rendering::font::Fonts;
use crate::rendering::layout::PixelRect;
use crate::rendering::piece;
use crate::rendering::side_panel;
use crate::rendering::theme::{Palette, Theme};
use crate::rendering::view::{Frame, PlayerView};
use crate::rendering::viewport::Viewport;
use crate::rendering::{board, hud};

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

pub struct MacroquadRenderer {
    dpi_scale: f32,
    canvas: MacroquadCanvas,
}

impl MacroquadRenderer {
    /// Load fonts from the embedded bytes and seed the canvas with
    /// the default theme.
    ///
    /// # Panics
    ///
    /// Panics if the embedded TTF bytes are malformed. The shipped bytes
    /// are well-formed so this should not happen in practice.
    #[must_use]
    pub fn new() -> Self {
        let fonts = Fonts::load().expect("Fira Code fonts are embedded in the binary");
        let canvas = MacroquadCanvas::new(Theme::DARK, fonts);
        Self {
            dpi_scale: screen_dpi_scale(),
            canvas,
        }
    }

    pub fn render(&mut self, frame: &Frame) {
        // Clear once per frame so background color matches the theme.
        self.canvas.clear(PaletteKey::Background);
        for view in &frame.views {
            render_view(view, &frame.ruleset, self.dpi_scale, &mut self.canvas);
        }
    }
}

impl Default for MacroquadRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a [`Viewport`] from a `PlayerView` and dispatch to per-feature
/// renderers.
fn render_view(view: &PlayerView, ruleset: &Ruleset, dpi: f32, canvas: &mut MacroquadCanvas) {
    let vp = Viewport::new(&view.snapshot, &view.layout, ruleset, dpi);
    // Copy the theme so we can hand `&mut *canvas` to the draw call;
    // `Theme: Copy` makes the duplication free.
    let theme = *canvas.theme();

    board::draw(&vp, &theme, &mut *canvas);
    piece::draw(&vp, &theme, view.ghost, &mut *canvas);
    side_panel::draw(&vp, &theme, &mut *canvas);
    hud::draw(&vp, &theme, view.label, &mut *canvas);
}

// ---- `Canvas` impl for macroquad ----
//
// The port impl above (helpers + per-feature `render_*` functions)
// stays put — the `Canvas` impl below is a **backend-internal**
// adapter that the per-feature modules (`board/`, `piece/`,
// `side_panel/`, `hud/`) consume. Co-locating here keeps the
// `draw_h_line` / `draw_v_line` / `draw_label` helpers and their
// `Canvas`-method counterparts in one file.

/// Convert an 8-bit [`Palette`] to a `0.0..=1.0` float `Color`
/// matching macroquad's `Color::new(r, g, b, a)` representation.
/// Bit-identical to the original `Color::new(r_u8 as f32 / 255.0, …)`
/// idiom (verified by the `theme` module's
/// `dark_*_matches_original_*` tests).
fn palette_to_color(p: Palette) -> Color {
    Color::new(
        f32::from(p.rgb.0) / 255.0,
        f32::from(p.rgb.1) / 255.0,
        f32::from(p.rgb.2) / 255.0,
        f32::from(p.alpha) / 255.0,
    )
}

/// Macroquad implementation of the [`Canvas`] trait — a thin wrapper
/// over `macroquad::prelude` draw primitives. Holds a `Theme` so
/// `PaletteKey`s resolve to concrete colors, and `Fonts` so
/// `FontWeight`s resolve to the right TTF bytes.
pub struct MacroquadCanvas {
    theme: Theme,
    fonts: Fonts,
}

impl MacroquadCanvas {
    /// Construct a canvas backed by the given theme and pre-loaded
    /// fonts.
    #[must_use]
    pub fn new(theme: Theme, fonts: Fonts) -> Self {
        Self { theme, fonts }
    }

    /// Resolve a [`PaletteKey`] to a concrete [`Color`] using the
    /// stored theme. `GridCross(alpha)` overrides the alpha from the
    /// theme's `grid_cross` palette entry.
    fn color(&self, key: PaletteKey) -> Color {
        match key {
            PaletteKey::Background => palette_to_color(self.theme.background),
            PaletteKey::Structure => palette_to_color(self.theme.structure),
            PaletteKey::GridBase => palette_to_color(self.theme.grid_base),
            PaletteKey::GridCross(alpha) => {
                let p = self.theme.grid_cross;
                Color::new(
                    f32::from(p.rgb.0) / 255.0,
                    f32::from(p.rgb.1) / 255.0,
                    f32::from(p.rgb.2) / 255.0,
                    f32::from(alpha) / 255.0,
                )
            }
            PaletteKey::Ghost => palette_to_color(self.theme.ghost),
            PaletteKey::Text => palette_to_color(self.theme.text),
            PaletteKey::Piece(t) => palette_to_color(self.theme.pieces.get(t)),
        }
    }

    /// Resolve a [`FontWeight`] to the corresponding [`Font`] handle.
    fn font(&self, weight: FontWeight) -> &Font {
        match weight {
            FontWeight::Regular => &self.fonts.regular,
            FontWeight::SemiBold => &self.fonts.semibold,
        }
    }

    /// Borrow the stored [`Theme`]. Module-level draws that need the
    /// theme separately (`board::draw`, `piece::draw`, etc.) get it
    /// through this accessor while the canvas-mutating calls happen
    /// through [`Canvas`] trait methods.
    #[must_use]
    pub const fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Borrow the stored [`Fonts`]. Side panels and HUD modules
    /// need the actual `Font` handles for `Canvas::text` calls.
    #[must_use]
    pub const fn fonts(&self) -> &Fonts {
        &self.fonts
    }
}

impl Canvas for MacroquadCanvas {
    fn clear(&mut self, fill: PaletteKey) {
        clear_background(self.color(fill));
    }

    fn fill_rect(&mut self, rect: PixelRect, fill: PaletteKey) {
        let c = self.color(fill);
        draw_rectangle(
            rect.x as f32,
            rect.y as f32,
            rect.w as f32,
            rect.h as f32,
            c,
        );
    }

    fn stroke_rect(&mut self, rect: PixelRect, stroke: PaletteKey) {
        // Reuse the 4-edge draw path of the legacy `draw_outline` so
        // outlining behavior stays byte-identical to the previous
        // board-outline / box-outline draws.
        let c = self.color(stroke);
        draw_h_line(rect.x, rect.y, rect.w, c);
        draw_h_line(rect.x, rect.bottom_pixel(), rect.w, c);
        draw_v_line(rect.x, rect.y, rect.h, c);
        draw_v_line(rect.right_pixel(), rect.y, rect.h, c);
    }

    fn stroke_h_line(&mut self, x: i32, y: i32, length: i32, stroke: PaletteKey) {
        let c = self.color(stroke);
        draw_rectangle(x as f32, y as f32, length as f32, 1.0, c);
    }

    fn stroke_v_line(&mut self, x: i32, y: i32, length: i32, stroke: PaletteKey) {
        let c = self.color(stroke);
        draw_rectangle(x as f32, y as f32, 1.0, length as f32, c);
    }

    fn text(&mut self, text: &str, x: f32, y: f32, weight: FontWeight, fill: PaletteKey) {
        let c = self.color(fill);
        let font = self.font(weight);
        let font_size = self.theme.hud.font_size_px.round() as u16;
        draw_text_ex(
            text,
            x,
            y,
            TextParams {
                font: Some(font),
                font_size,
                font_scale: 1.0,
                font_scale_aspect: 1.0,
                color: c,
                rotation: 0.0,
            },
        );
    }
}
