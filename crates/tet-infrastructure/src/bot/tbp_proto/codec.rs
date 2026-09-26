pub use tbp::bot_msg::{Error, Info, Ready, Suggestion};
pub use tbp::data::{ErrorCause, Move, Orientation, Piece, PieceLocation, Spin};
pub use tbp::frontend_msg::{NewPiece, Play, Quit, Rules, Start, Stop, Suggest};
pub use tbp::{BotMessage, Feature, FrontendMessage, MaybeUnknown};

#[cfg(test)]
mod test {
    //! Smoke test: verifies the re-exports compile and a wire message
    //! round-trips through serde. Catches future regressions where a
    //! re-export gets dropped or renamed.

    use super::*;
    use rstest::rstest;

    #[rstest]
    fn codec_round_trips_a_suggest_message() {
        let original = FrontendMessage::Suggest(Suggest::default());
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: FrontendMessage = serde_json::from_str(&json).expect("deserialize");
        assert!(matches!(parsed, FrontendMessage::Suggest(_)));
    }

    #[rstest]
    fn codec_round_trips_an_info_message_with_features() {
        // `Info` has a required `features` field; if our re-export drops
        // something, this test catches it.
        let original = BotMessage::Info(Info::new(
            "test_bot".into(),
            "1.0.0".into(),
            "tet-rs".into(),
            vec![],
        ));
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: BotMessage = serde_json::from_str(&json).expect("deserialize");
        match parsed {
            BotMessage::Info(i) => {
                assert_eq!(i.name, "test_bot");
                assert_eq!(i.version, "1.0.0");
                assert_eq!(i.author, "tet-rs");
            }
            other => panic!("expected Info, got {other:?}"),
        }
    }

    #[rstest]
    fn codec_maybe_unknown_round_trips_through_known_and_unknown() {
        // Both Known and Unknown variants must survive serialize+deserialize.
        let known = MaybeUnknown::Known(Piece::T);
        let json = serde_json::to_string(&known).expect("serialize Known");
        let back: MaybeUnknown<Piece> = serde_json::from_str(&json).expect("deserialize Known");
        assert!(matches!(back, MaybeUnknown::Known(Piece::T)));

        let unknown = MaybeUnknown::<Piece>::Unknown(serde_json::json!("future-piece"));
        let json = serde_json::to_string(&unknown).expect("serialize Unknown");
        let back: MaybeUnknown<Piece> = serde_json::from_str(&json).expect("deserialize Unknown");
        assert!(matches!(back, MaybeUnknown::Unknown(_)));
    }
}
