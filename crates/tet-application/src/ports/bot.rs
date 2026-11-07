use async_trait::async_trait;
use tet_domain::{MinoType, Orientation, Ruleset};

use crate::PlayerSnapshot;

/// A move a bot wants to play.
///
/// `(x, y)` is the piece's **rotation center** — the pivot the piece rotates
/// around, using the game's rotation-center convention, not the bbox origin.
/// The domain's
/// `MinoType::rotation_center_offset` implements this convention, so the bot
/// that produces a move and the host that consumes it agree on the pivot.
#[derive(Clone, Copy, Debug)]
pub struct BotPieceLocation {
    pub kind: MinoType,
    pub orientation: Orientation,
    pub x: i8,
    pub y: i8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BotSpin {
    None,
    Mini,
    Full,
}

#[derive(Clone, Copy, Debug)]
pub struct BotMove {
    pub location: BotPieceLocation,
    pub spin: BotSpin,
    pub hold: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum BotError {
    #[error("bot I/O failed")]
    Io,
    #[error("bot protocol violated: {0}")]
    Protocol(String),
    #[error("bot process exited")]
    Exited,
}

#[async_trait]
pub trait BotTransport {
    /// Called once at game start, before any `update`.
    ///
    /// # Errors
    ///
    /// - `Io`: the bot could not be started
    /// - `Protocol`: the bot rejected the rules
    fn start(&mut self, rules: &Ruleset) -> Result<(), BotError>;
    /// Push the latest player state to the bot.
    ///
    /// # Errors
    ///
    /// - `Io`: the bot could not be reached
    /// - `Protocol`: the bot could not accept the state
    async fn update(&mut self, snapshot: &PlayerSnapshot) -> Result<(), BotError>;
    /// Ask for the bot's preferred moves, ordered best-first.
    ///
    /// # Errors
    ///
    /// - `Io`: the bot could not be reached
    /// - `Protocol`: the bot returned a move the host could not interpret
    async fn suggest(&mut self) -> Result<Vec<BotMove>, BotError>;
    /// Tell the bot to stop calculating. Called on game end or disconnect.
    async fn stop(&mut self);
}
