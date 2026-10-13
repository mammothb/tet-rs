//! HUD overlay — stats below the board, player label above the
//! queue column.

use crate::rendering::backend::canvas::Canvas;
use crate::rendering::theme::Theme;
use crate::rendering::viewport::Viewport;

pub mod label;
pub mod stats;

/// Render the stats line and player label. Both gated by
/// `Layout::show_stats` and `Layout::show_label`.
pub fn draw(vp: &Viewport, theme: &Theme, label: &str, canvas: &mut dyn Canvas) {
    if vp.layout().show_stats {
        stats::draw(vp, theme, canvas);
    }
    if vp.layout().show_label {
        label::draw(vp, theme, label, canvas);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use tet_application::{GridStyle, Layout, Phase, Piece, PlayerSnapshot};
    use tet_domain::{Board, MinoType, Ruleset};

    use crate::rendering::backend::canvas::mock::{DrawOp, RecordingCanvas};
    use crate::rendering::backend::canvas::{FontWeight, PaletteKey};

    fn test_setup() -> (PlayerSnapshot, Layout, Ruleset) {
        let ruleset = Ruleset::guideline();
        let snapshot = PlayerSnapshot {
            board: Board::new(
                u8::try_from(ruleset.num_cols).unwrap(),
                u8::try_from(ruleset.num_rows).unwrap(),
            ),
            queue: vec![],
            hold: None,
            combo: 2,
            b2b: true,
            current: Piece::spawn(MinoType::I),
            phase: Phase::Playing,
            pending_garbage: vec![],
        };
        // Matches live composition root geometry from side panel tests
        let layout = Layout {
            origin: (116.0, 16.0),
            cell_px: 24.0,
            show_queue: true,
            show_hold: true,
            show_stats: true,
            show_label: true,
            grid_style: GridStyle::None,
        };
        (snapshot, layout, ruleset)
    }

    // ---- orchestrator gating ----

    #[test]
    fn hud_skips_both_when_both_flags_off() {
        let (snapshot, mut layout, ruleset) = test_setup();
        layout.show_stats = false;
        layout.show_label = false;
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();

        draw(&vp, &Theme::DARK, "human", &mut canvas);

        assert!(
            canvas.calls.is_empty(),
            "no calls expected when both stats and label flags are off, got: {:?}",
            canvas.calls,
        );
    }

    #[test]
    fn hud_renders_only_stats_when_label_disabled() {
        let (snapshot, mut layout, ruleset) = test_setup();
        layout.show_stats = true;
        layout.show_label = false;
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();

        draw(&vp, &Theme::DARK, "human", &mut canvas);

        let text_count = canvas
            .calls
            .iter()
            .filter(|op| matches!(op, DrawOp::Text { .. }))
            .count();
        assert_eq!(text_count, 2, "only stats texts (combo and b2b)");
    }

    #[test]
    fn hud_renders_only_label_when_stats_disabled() {
        let (snapshot, mut layout, ruleset) = test_setup();
        layout.show_stats = false;
        layout.show_label = true;
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();

        draw(&vp, &Theme::DARK, "human", &mut canvas);

        let text_count = canvas
            .calls
            .iter()
            .filter(|op| matches!(op, DrawOp::Text { .. }))
            .count();
        assert_eq!(text_count, 1, "only player label text");
    }

    // ---- stats rendering ----

    #[test]
    fn stats_renders_combo_and_b2b_correctly() {
        let (snapshot, layout, ruleset) = test_setup();
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();

        stats::draw(&vp, &Theme::DARK, &mut canvas);

        let texts: Vec<_> = canvas
            .calls
            .iter()
            .filter_map(|op| match op {
                DrawOp::Text {
                    text, weight, fill, ..
                } => Some((text.clone(), *weight, *fill)),
                _ => None,
            })
            .collect();

        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0].0, "combo: 2");
        assert_eq!(texts[0].1, FontWeight::Regular);
        assert_eq!(texts[0].2, PaletteKey::Text);

        assert_eq!(texts[1].0, "b2b: true");
        assert_eq!(texts[1].1, FontWeight::Regular);
        assert_eq!(texts[1].2, PaletteKey::Text);
    }

    // ---- label rendering ----

    #[test]
    fn label_returns_early_when_empty() {
        let (snapshot, layout, ruleset) = test_setup();
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();

        label::draw(&vp, &Theme::DARK, "", &mut canvas);

        assert!(
            canvas.calls.is_empty(),
            "no draw calls expected for empty label"
        );
    }

    #[test]
    fn label_renders_with_correct_weight_and_content() {
        let (snapshot, layout, ruleset) = test_setup();
        let vp = Viewport::new(&snapshot, &layout, &ruleset, 1.0);
        let mut canvas = RecordingCanvas::default();

        label::draw(&vp, &Theme::DARK, "bot", &mut canvas);

        let texts: Vec<_> = canvas
            .calls
            .iter()
            .filter_map(|op| match op {
                DrawOp::Text {
                    text, weight, fill, ..
                } => Some((text.clone(), *weight, *fill)),
                _ => None,
            })
            .collect();

        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0].0, "bot");
        assert_eq!(texts[0].1, FontWeight::SemiBold);
        assert_eq!(texts[0].2, PaletteKey::Text);
    }
}
