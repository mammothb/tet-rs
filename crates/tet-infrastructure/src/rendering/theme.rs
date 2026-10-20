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
    pub ghost_alpha: u8,
    /// HUD text color.
    pub text: Palette,
    pub pieces: PiecePalette,
    pub hud: HudMetrics,
}

impl Theme {
    /// Default theme.
    pub const DARK: Self = Self {
        background: Palette::rgb(0, 0, 0),
        structure: Palette::rgb(80, 80, 80),
        grid_base: Palette::rgb(20, 20, 20),
        grid_cross: Palette::rgb(60, 60, 60),
        ghost_alpha: 128,
        text: Palette::rgb(255, 255, 255),
        pieces: PiecePalette {
            i: Palette::rgb(0, 217, 217),
            o: Palette::rgb(242, 217, 0),
            t: Palette::rgb(140, 0, 140),
            s: Palette::rgb(0, 217, 0),
            z: Palette::rgb(217, 0, 0),
            j: Palette::rgb(0, 0, 217),
            l: Palette::rgb(242, 140, 0),
        },
        hud: HudMetrics {
            font_size_px: 18.0,
            label_padding_px: 0.0,
            hold_box_cells: 4,
            queue_preview_cell_px: 16.0,
            queue_preview_spacing_px: 4.0,
        },
    };

    #[must_use]
    pub const fn dark(ghost_alpha: u8) -> Self {
        Self {
            ghost_alpha,
            ..Self::DARK
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp
)]
mod test {
    use super::*;
    use rstest::rstest;

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

    // ---- palette colors ----

    #[rstest]
    #[case::structure(Theme::DARK.structure, 80.0 / 255.0, 80.0 / 255.0, 80.0 / 255.0, 1.0)]
    #[case::grid_base(Theme::DARK.grid_base, 20.0 / 255.0, 20.0 / 255.0, 20.0 / 255.0, 1.0)]
    #[case::grid_cross(Theme::DARK.grid_cross, 60.0 / 255.0, 60.0 / 255.0, 60.0 / 255.0, 1.0)]
    #[case::text(Theme::DARK.text, 1.0,1.0, 1.0, 1.0)]
    #[case::background(Theme::DARK.background, 0.0, 0.0, 0.0, 1.0)]
    fn dark_palette_field_matches_source_macroquad_color(
        #[case] actual: Palette,
        #[case] r: f32,
        #[case] g: f32,
        #[case] b: f32,
        #[case] a: f32,
    ) {
        let expected = palette_from_f32(r, g, b, a);
        assert_eq!(actual, expected);
    }

    // ---- piece colors ----
    //
    // Each `PiecePalette` entry matches its source `piece_color()` literal.

    #[rstest]
    #[case::i(MinoType::I, 0.0, 0.85, 0.85)]
    #[case::o(MinoType::O, 0.95, 0.85, 0.0)]
    #[case::t(MinoType::T, 0.55, 0.0, 0.55)]
    #[case::s(MinoType::S, 0.0, 0.85, 0.0)]
    #[case::z(MinoType::Z, 0.85, 0.0, 0.0)]
    #[case::j(MinoType::J, 0.0, 0.0, 0.85)]
    #[case::l(MinoType::L, 0.95, 0.55, 0.0)]
    fn dark_piece_palette_matches_source_piece_color(
        #[case] kind: MinoType,
        #[case] r: f32,
        #[case] g: f32,
        #[case] b: f32,
    ) {
        let expected = palette_from_f32(r, g, b, 1.0);
        assert_eq!(Theme::DARK.pieces.get(kind), expected);
    }

    // ---- `PiecePalette::get` dispatch ----
    //
    // `get(MinoType)` must dispatch to the matching named field. This
    // catches a `match` typo where e.g. `MinoType::I` returns `self.j`
    // instead of `self.i`.

    #[rstest]
    #[case::i(MinoType::I, Theme::DARK.pieces.i)]
    #[case::o(MinoType::O, Theme::DARK.pieces.o)]
    #[case::t(MinoType::T, Theme::DARK.pieces.t)]
    #[case::s(MinoType::S, Theme::DARK.pieces.s)]
    #[case::z(MinoType::Z, Theme::DARK.pieces.z)]
    #[case::j(MinoType::J, Theme::DARK.pieces.j)]
    #[case::l(MinoType::L, Theme::DARK.pieces.l)]
    fn piece_palette_get_dispatches_to_named_field(
        #[case] kind: MinoType,
        #[case] expected: Palette,
    ) {
        assert_eq!(Theme::DARK.pieces.get(kind), expected);
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
