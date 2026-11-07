pub mod blockfish;

use tet_application::BotTransport;

use crate::bot::blockfish::BlockfishBot;

/// A bot implementation compiled into the game.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BotType {
    Blockfish,
}

impl BotType {
    /// All bot types understood by the configuration parser.
    pub const ALL: &'static [Self] = &[Self::Blockfish];

    /// Parse the canonical configuration token for a bot type.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "blockfish" => Some(Self::Blockfish),
            _ => None,
        }
    }

    /// Canonical token used in configuration files.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blockfish => "blockfish",
        }
    }

    /// Default display name for this bot type.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Blockfish => "Blockfish",
        }
    }
}

/// Create the in-process implementation for a bot type.
#[must_use]
pub fn create_bot(bot_type: BotType) -> Box<dyn BotTransport + Send> {
    match bot_type {
        BotType::Blockfish => Box::new(BlockfishBot::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rstest::rstest;

    #[test]
    fn parses_known_bot_types() {
        assert_eq!(BotType::parse("blockfish"), Some(BotType::Blockfish));
    }

    #[rstest]
    #[case("unknown")]
    #[case("Blockfish")]
    fn rejects_unknown_bot_types(#[case] value: &str) {
        assert_eq!(BotType::parse(value), None);
    }

    #[test]
    fn exposes_canonical_names() {
        assert_eq!(BotType::Blockfish.as_str(), "blockfish");
        assert_eq!(BotType::Blockfish.display_name(), "Blockfish");
    }

    #[test]
    fn creates_blockfish_implementation() {
        let _bot = create_bot(BotType::Blockfish);
    }
}
