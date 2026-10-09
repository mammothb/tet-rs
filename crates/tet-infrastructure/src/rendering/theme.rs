//! Visual theme configuration for the renderer.
//!
//! No macroquad dependency: this module is pure data. The renderer (when
//! it consumes a `Theme`) maps `Palette` to a backend-specific color
//! type at draw time.
//!
//! ## Why a separate type from macroquad's `Color`
//!
//! `Theme` is the **single source of truth** for visual configuration
//! (palette, HUD metrics). Having a renderer-agnostic type means:
//!
//! - Tests verify theme values directly (no GL context required).
//! - Alternative renderers (CLI, headless recorder) consume the same
//!   `Theme` via their own `Palette → color` mapping.
//! - Switching to colorblind / light themes is one const swap.

use tet_domain::MinoType;

/// 8-bit RGBA color. Alpha 0 = transparent, 255 = opaque.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Palette {
    pub rgb: (u8, u8, u8),
    pub alpha: u8,
}

impl Palette {
    /// Opaque RGB (alpha = 255).
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            rgb: (r, g, b),
            alpha: 255,
        }
    }

    /// RGBA with explicit alpha (0..=255).
    #[must_use]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            rgb: (r, g, b),
            alpha: a,
        }
    }
}

/// HUD-level visual metrics — sizes and spacings used by side panels
/// and the HUD overlay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudMetrics {
    pub font_size_px: f32,
    pub label_padding_px: f32,
    pub hold_box_cells: i32,
    pub queue_preview_cell_px: f32,
    pub queue_preview_spacing_px: f32,
}

/// Color per tetromino kind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PiecePalette {
    pub i: Palette,
    pub o: Palette,
    pub t: Palette,
    pub s: Palette,
    pub z: Palette,
    pub j: Palette,
    pub l: Palette,
}

impl PiecePalette {
    /// Look up the palette for a single tetromino kind.
    #[must_use]
    pub const fn get(&self, kind: MinoType) -> Palette {
        match kind {
            MinoType::I => self.i,
            MinoType::O => self.o,
            MinoType::T => self.t,
            MinoType::S => self.s,
            MinoType::Z => self.z,
            MinoType::J => self.j,
            MinoType::L => self.l,
        }
    }
}

/// A complete visual theme. `Theme::DARK` is the default for this game.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// Window background. Drawn at the start of every frame.
    pub background: Palette,
    /// Board outline + locked garbage cells.
    pub structure: Palette,
    /// Faint base grid lines (drawn across the full board).
    pub grid_base: Palette,
    /// Brighter grid color used at corner intersections. Alpha is
    /// overlaid per-layer at draw time; the stored value is the max.
    pub grid_cross: Palette,
    /// Ghost piece fill (semi-transparent).
    pub ghost: Palette,
    /// HUD text color.
    pub text: Palette,
    pub pieces: PiecePalette,
    pub hud: HudMetrics,
}

impl Theme {
    /// Default theme. Values transcribed from the original module-level
    /// constants in `macroquad.rs` (which this theme is intended to
    /// replace). Each field's source is documented inline so the byte
    /// mapping is traceable.
    pub const DARK: Self = Self {
        // clear_background(BLACK) → Color::new(0.0, 0.0, 0.0, 1.0).
        background: Palette::rgb(0, 0, 0),
        // STRUCTURE_COLOR was Color::new(80.0/255.0, 80.0/255.0, 80.0/255.0, 1.0).
        structure: Palette::rgb(80, 80, 80),
        // GRID_BASE_COLOR was Color::new(20.0/255.0, ..., 1.0).
        grid_base: Palette::rgb(20, 20, 20),
        // GRID was Color::new(60.0/255.0, ..., 1.0). Alpha is overridden
        // per-layer in render_board.
        grid_cross: Palette::rgb(60, 60, 60),
        // GHOST_COLOR was Color::new(1.0, 1.0, 1.0, 0.25).
        // 0.25 * 255 = 63.75 → rounds to 64.
        ghost: Palette::rgba(255, 255, 255, 64),
        // TEXT_COLOR was `WHITE` = Color::new(1.0, 1.0, 1.0, 1.0).
        text: Palette::rgb(255, 255, 255),
        // piece_color() values, rounded to nearest u8 from the original
        // f32 literals. Rounding matches what `palette_from_f32` does in
        // the test module — the bytes are byte-for-byte identical to the
        // rendered framebuffer bytes produced by the macroquad source.
        pieces: PiecePalette {
            // I: Color::new(0.0, 0.85, 0.85, 1.0) → (0, 217, 217, 255).
            i: Palette::rgb(0, 217, 217),
            // O: Color::new(0.95, 0.85, 0.0, 1.0) → (242, 217, 0, 255).
            o: Palette::rgb(242, 217, 0),
            // T: Color::new(0.55, 0.0, 0.55, 1.0) → (140, 0, 140, 255).
            t: Palette::rgb(140, 0, 140),
            // S: Color::new(0.0, 0.85, 0.0, 1.0) → (0, 217, 0, 255).
            s: Palette::rgb(0, 217, 0),
            // Z: Color::new(0.85, 0.0, 0.0, 1.0) → (217, 0, 0, 255).
            z: Palette::rgb(217, 0, 0),
            // J: Color::new(0.0, 0.0, 0.85, 1.0) → (0, 0, 217, 255).
            j: Palette::rgb(0, 0, 217),
            // L: Color::new(0.95, 0.55, 0.0, 1.0) → (242, 140, 0, 255).
            l: Palette::rgb(242, 140, 0),
        },
        hud: HudMetrics {
            // HUD_FONT_SIZE = 18.0
            font_size_px: 18.0,
            // LABEL_PADDING = 0.0
            label_padding_px: 0.0,
            // HOLD_BOX_CELLS = 4
            hold_box_cells: 4,
            // QUEUE_PREVIEW_CELL_PX = 16.0
            queue_preview_cell_px: 16.0,
            // QUEUE_PREVIEW_SPACING = 4.0
            queue_preview_spacing_px: 4.0,
        },
    };
}

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp
)]
mod test {
    use super::*;

    /// Convert an f32 RGBA tuple (each ∈ [0, 1]) into a `Palette` using
    /// the same rounding (`round() as u8`) macroquad's `Color::new` would
    /// round-trip to display bytes. Lets theme tests assert against the
    /// documented source macroquad constants without taking a macroquad
    /// dependency in this module.
    const fn palette_from_f32(r: f32, g: f32, b: f32, a: f32) -> Palette {
        Palette {
            rgb: (
                (r * 255.0).round() as u8,
                (g * 255.0).round() as u8,
                (b * 255.0).round() as u8,
            ),
            alpha: (a * 255.0).round() as u8,
        }
    }

    // ---- background / structure / grid / ghost / text ----
    //
    // Each test names the source macroquad literal in the docstring so
    // the byte mapping stays traceable.

    #[test]
    fn dark_background_is_black() {
        let expected = palette_from_f32(0.0, 0.0, 0.0, 1.0);
        assert_eq!(Theme::DARK.background, expected);
    }

    #[test]
    fn dark_structure_matches_original_structure_color() {
        // Source: `STRUCTURE_COLOR = Color::new(80.0/255.0, ..., 1.0)`.
        let expected = palette_from_f32(80.0 / 255.0, 80.0 / 255.0, 80.0 / 255.0, 1.0);
        assert_eq!(Theme::DARK.structure, expected);
    }

    #[test]
    fn dark_grid_base_matches_original_grid_base_color() {
        // Source: `GRID_BASE_COLOR = Color::new(20.0/255.0, ..., 1.0)`.
        let expected = palette_from_f32(20.0 / 255.0, 20.0 / 255.0, 20.0 / 255.0, 1.0);
        assert_eq!(Theme::DARK.grid_base, expected);
    }

    #[test]
    fn dark_grid_cross_matches_original_grid_color() {
        // Source: `GRID = Color::new(60.0/255.0, ..., 1.0)`. Alpha is
        // overridden per-layer at draw time; we store opaque here.
        let expected = palette_from_f32(60.0 / 255.0, 60.0 / 255.0, 60.0 / 255.0, 1.0);
        assert_eq!(Theme::DARK.grid_cross.rgb, expected.rgb);
        assert_eq!(Theme::DARK.grid_cross.alpha, 255);
    }

    #[test]
    fn dark_ghost_matches_original_ghost_color() {
        // Source: `GHOST_COLOR = Color::new(1.0, 1.0, 1.0, 0.25)`.
        let expected = palette_from_f32(1.0, 1.0, 1.0, 0.25);
        assert_eq!(Theme::DARK.ghost, expected);
        // Sanity: alpha 0.25 rounds to 64.
        assert_eq!(Theme::DARK.ghost.alpha, 64);
    }

    #[test]
    fn dark_text_matches_original_text_color() {
        // Source: `TEXT_COLOR = WHITE = Color::new(1.0, 1.0, 1.0, 1.0)`.
        let expected = palette_from_f32(1.0, 1.0, 1.0, 1.0);
        assert_eq!(Theme::DARK.text, expected);
    }

    // ---- piece colors ----

    /// Each `PiecePalette` entry matches its source `piece_color()` value.
    #[test]
    fn dark_piece_palette_matches_original_piece_color_per_mino() {
        let rgb = |r: f32, g: f32, b: f32| -> Palette { palette_from_f32(r, g, b, 1.0) };
        assert_eq!(Theme::DARK.pieces.i, rgb(0.0, 0.85, 0.85));
        assert_eq!(Theme::DARK.pieces.o, rgb(0.95, 0.85, 0.0));
        assert_eq!(Theme::DARK.pieces.t, rgb(0.55, 0.0, 0.55));
        assert_eq!(Theme::DARK.pieces.s, rgb(0.0, 0.85, 0.0));
        assert_eq!(Theme::DARK.pieces.z, rgb(0.85, 0.0, 0.0));
        assert_eq!(Theme::DARK.pieces.j, rgb(0.0, 0.0, 0.85));
        assert_eq!(Theme::DARK.pieces.l, rgb(0.95, 0.55, 0.0));
    }

    /// `PiecePalette::get` dispatches to the correct named field.
    #[test]
    fn piece_palette_get_returns_correct_entry_per_mino() {
        assert_eq!(Theme::DARK.pieces.get(MinoType::I), Theme::DARK.pieces.i);
        assert_eq!(Theme::DARK.pieces.get(MinoType::O), Theme::DARK.pieces.o);
        assert_eq!(Theme::DARK.pieces.get(MinoType::T), Theme::DARK.pieces.t);
        assert_eq!(Theme::DARK.pieces.get(MinoType::S), Theme::DARK.pieces.s);
        assert_eq!(Theme::DARK.pieces.get(MinoType::Z), Theme::DARK.pieces.z);
        assert_eq!(Theme::DARK.pieces.get(MinoType::J), Theme::DARK.pieces.j);
        assert_eq!(Theme::DARK.pieces.get(MinoType::L), Theme::DARK.pieces.l);
    }

    /// All seven tetromino colors are distinct. Catches a transcription
    /// bug where two pieces accidentally get the same palette.
    #[test]
    fn dark_pieces_are_all_distinct() {
        let pals = [
            ("I", Theme::DARK.pieces.i),
            ("O", Theme::DARK.pieces.o),
            ("T", Theme::DARK.pieces.t),
            ("S", Theme::DARK.pieces.s),
            ("Z", Theme::DARK.pieces.z),
            ("J", Theme::DARK.pieces.j),
            ("L", Theme::DARK.pieces.l),
        ];
        for i in 0..pals.len() {
            for j in (i + 1)..pals.len() {
                assert_ne!(
                    pals[i].1, pals[j].1,
                    "piece palettes {} and {} are identical",
                    pals[i].0, pals[j].0
                );
            }
        }
    }

    // ---- HUD metrics ----

    #[test]
    fn dark_hud_matches_original_constants() {
        // HUD_FONT_SIZE = 18.0, LABEL_PADDING = 0.0, HOLD_BOX_CELLS = 4,
        // QUEUE_PREVIEW_CELL_PX = 16.0, QUEUE_PREVIEW_SPACING = 4.0.
        assert_eq!(Theme::DARK.hud.font_size_px, 18.0);
        assert_eq!(Theme::DARK.hud.label_padding_px, 0.0);
        assert_eq!(Theme::DARK.hud.hold_box_cells, 4);
        assert_eq!(Theme::DARK.hud.queue_preview_cell_px, 16.0);
        assert_eq!(Theme::DARK.hud.queue_preview_spacing_px, 4.0);
    }

    // ---- Palette constructors ----

    #[test]
    fn palette_rgb_defaults_alpha_to_opaque() {
        let p = Palette::rgb(10, 20, 30);
        assert_eq!(p.rgb, (10, 20, 30));
        assert_eq!(p.alpha, 255);
    }

    #[test]
    fn palette_rgba_preserves_alpha() {
        let p = Palette::rgba(10, 20, 30, 64);
        assert_eq!(p.rgb, (10, 20, 30));
        assert_eq!(p.alpha, 64);
    }
}
