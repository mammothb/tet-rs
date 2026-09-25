use tet_domain::{MinoType, Orientation, Ruleset};

use crate::PlayerSnapshot;

/// A move a bot wants to play, in TBP wire-format coordinates:
/// (x, y) is the **true-rotation center** of the piece, not the bbox origin.
#[derive(Clone, Copy)]
pub struct PieceLocation {
    pub kind: MinoType,
    pub orientation: Orientation,
    pub x: i8,
    pub y: i8,
}

#[derive(Clone, Copy)]
pub enum Spin {
    None,
    Mini,
    Full,
}

#[derive(Clone, Copy)]
pub struct Move {
    pub location: PieceLocation,
    pub spin: Spin,
}

#[derive(Debug, thiserror::Error)]
pub enum BotError {
    #[error("bot I/O failed")]
    Io,
    #[error("bot protocol violated")]
    Protocol,
    #[error("bot process exited")]
    Exited,
}

pub trait BotTransport {
    /// Called once at game start. Bot must respond with `Ready`.
    ///
    /// # Errors
    ///
    /// - `Io`: subprocess I/O failed (broken pipe, write error)
    /// - `Protocol`: bot response wasn't valid JSON or didn't match the expected `ready` message
    /// - `Exited`: bot process exited before responding
    fn start(&mut self, rules: &Ruleset) -> Result<(), BotError>;
    /// Push the latest player state to the bot.
    ///
    /// # Errors
    ///
    /// - `Io`: subprocess I/O failed
    /// - `Protocol`: bot response wasn't valid JSON
    /// - `Exited`: bot process exited
    fn update(&mut self, snapshot: &PlayerSnapshot) -> Result<(), BotError>;
    /// Ask for the bot's preferred moves, ordered best-first.
    ///
    /// # Errors
    ///
    /// - `Io`: subprocess I/O failed
    /// - `Protocol`: bot response wasn't a valid `suggestion` (missing moves, bad fields)
    /// - `Exited`: bot process exited before responding
    fn suggest(&mut self) -> Result<Vec<Move>, BotError>;
    /// Tell the bot to stop calculating. Called on game end or disconnect.
    fn stop(&mut self);
}
