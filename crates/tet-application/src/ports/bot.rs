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

pub enum BotError {
    Io,
    Protocol,
    Exited,
}

pub trait BotTransport {
    /// Called once at game start. Bot must respond with `Ready`.
    fn start(&mut self, rules: &Ruleset) -> Result<(), BotError>;
    /// Push the latest player state to the bot.
    fn update(&mut self, snapshot: &PlayerSnapshot) -> Result<(), BotError>;
    /// Ask for the bot's preferred moves, ordered best-first.
    fn suggest(&mut self) -> Result<Vec<Move>, BotError>;
    /// Tell the bot to stop calculating. Called on game end or disconnect.
    fn stop(&mut self);
}
