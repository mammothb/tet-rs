use tet_domain::{Rng, Ruleset};

use crate::{BotTransport, Input, PlayerSnapshot, player::Player};

pub struct GameSession<R: Rng> {
    pub players: Vec<Player<R>>,
    pub ruleset: Ruleset,
    pub frame: u64,
}

impl<R: Rng> GameSession<R> {
    #[must_use]
    pub fn new(ruleset: Ruleset) -> Self {
        Self {
            players: Vec::new(),
            ruleset,
            frame: 0,
        }
    }

    /// Add a player. Returns the player index used for `apply_*` and `snapshot`.
    pub fn add_player(&mut self, player: Player<R>) -> usize {
        self.players.push(player);
        self.players.len() - 1
    }

    /// Advance the entire game by one frame.
    pub fn step_frame(&mut self) {
        todo!()
    }

    /// Drive a bot: take snapshot, ask transport for moves, apply first
    /// valid one. (For when the session is in control of the bot loop.)
    pub fn step_bot(&mut self, _idx: usize, _bot: &mut dyn BotTransport) {
        todo!()
    }

    /// Apply a discrete human input to one player.
    pub fn apply_input(&mut self, _idx: usize, _input: Input) {
        todo!()
    }

    /// Take a `Clone`-able snapshot for bot consumption.
    #[must_use]
    pub fn snapshot(&self, _idx: usize) -> PlayerSnapshot {
        todo!()
    }

    /// True when every player is `GameOver`.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        todo!()
    }
}
