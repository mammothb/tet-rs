use tet_domain::{MinoType, Rng, Rotation, Ruleset, v2};

use crate::tick;
use crate::{
    ATTACK_FOR_LINES, BotTransport, GARBAGE_DELAY_FRAMES, Input, Move, PendingGarbage, Phase,
    Piece, Player, PlayerSnapshot, TickResult,
};

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
        self.frame += 1;

        let mut results = Vec::with_capacity(self.players.len());
        for player in &mut self.players {
            if player.phase != Phase::Playing {
                results.push(TickResult::default());
                continue;
            }

            // 1. Decrement pending garbage delays; insert ready rows.
            let mut i = 0;
            while i < player.pending_garbage.len() {
                player.pending_garbage[i].delay_remaining -= 1;
                if player.pending_garbage[i].delay_remaining == 0 {
                    let a = player.pending_garbage.remove(i);
                    player.board.insert_garbage(a.count, a.hole);
                } else {
                    i += 1;
                }
            }

            // 2. Tick this player (delegated to tick.rs).
            let r = tick::step_player(player, &self.ruleset);

            // 3. Tick.rs has already cleared lines and cancelled matching rows
            //    from `pending_garbage`. We just collect the result here.
            results.push(r);
        }

        // 4. Distribute new attacks from line clears to all opponents.
        self.distribute_garbage(&results);
    }

    /// Drive a bot: take snapshot, ask transport for moves, apply first
    /// valid one. (For when the session is in control of the bot loop.)
    pub fn step_bot(&mut self, idx: usize, bot: &mut dyn BotTransport) {
        let snap = self.snapshot(idx);
        bot.update(&snap).ok();

        let Ok(moves) = bot.suggest() else { return };

        // Try each move in preference order; first valid one wins.
        for mv in moves {
            if apply_bot_move(&mut self.players[idx], mv, &self.ruleset).is_some() {
                return;
            }
        }
        // All suggested moves were invalid (board state changed under bot);
        // next `suggest` will see updated state.
    }

    /// Apply a discrete human input to one player.
    pub fn apply_input(&mut self, idx: usize, input: Input) -> Option<TickResult> {
        if self.players[idx].phase != Phase::Playing {
            return None;
        }

        match input {
            Input::None => None,
            Input::MoveLeft => {
                tick::apply_horizontal_input(&mut self.players[idx], -1);
                None
            }
            Input::MoveRight => {
                tick::apply_horizontal_input(&mut self.players[idx], 1);
                None
            }
            Input::RotateCW => {
                tick::apply_rotation_input(&mut self.players[idx], &self.ruleset, Rotation::CW);
                None
            }
            Input::RotateCCW => {
                tick::apply_rotation_input(&mut self.players[idx], &self.ruleset, Rotation::CCW);
                None
            }
            Input::SoftDrop => {
                tick::soft_drop(&mut self.players[idx]);
                None
            }
            Input::Hold => {
                tick::try_hold(&mut self.players[idx]);
                None
            }
            Input::HardDrop => {
                let result = tick::hard_drop(&mut self.players[idx]);
                tick::post_lock(&mut self.players[idx], &result, &self.ruleset);
                Some(result)
            }
        }
    }

    /// Take a `Clone`-able snapshot for bot consumption.
    #[must_use]
    pub fn snapshot(&self, idx: usize) -> PlayerSnapshot {
        let p = &self.players[idx];
        PlayerSnapshot {
            board: p.board.clone(),
            queue: p.queue.peek().to_vec(),
            hold: p.hold,
            combo: p.combo,
            b2b: p.b2b,
            current: p.current,
            phase: p.phase,
            pending_garbage: p.pending_garbage.clone(),
        }
    }

    /// True when every player is `GameOver`.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.players.iter().all(|p| p.phase == Phase::GameOver)
    }

    fn distribute_garbage(&mut self, results: &[TickResult]) {
        let num_cols = u32::try_from(self.ruleset.num_cols).expect("board width fits in u32");
        let n = self.players.len();
        let mut pending: Vec<(usize, PendingGarbage)> = Vec::new();
        for (idx, r) in results.iter().enumerate() {
            let count = ATTACK_FOR_LINES[r.lines_cleared as usize]; // 0, 1, 2, 4
            if count == 0 {
                continue;
            }
            let hole = u8::try_from(self.players[idx].attack_rng.next_u32() % num_cols)
                .expect("hole column fits in u8");
            let atk = PendingGarbage {
                count,
                hole,
                delay_remaining: GARBAGE_DELAY_FRAMES,
            };
            for opp in 0..n {
                if opp != idx {
                    pending.push((opp, atk));
                }
            }
        }
        for (opp, atk) in pending {
            self.players[opp].pending_garbage.push(atk);
        }
    }
}

fn apply_bot_move<R: Rng>(player: &mut Player<R>, mv: Move, ruleset: &Ruleset) -> Option<Piece> {
    // 1. Convert TBP true-rotation-center → bbox-anchor position.
    //    `MinoType::tbp_center_for` is a static lookup in the domain.
    let center = MinoType::rotation_center_offset(mv.location.kind, mv.location.orientation);
    let pos = v2![mv.location.x, mv.location.y] - center;

    let target = mv.location.orientation;
    let curr_orient = player.current.orientation;
    let orig_orient = curr_orient;
    let orig_pos = player.current.pos;

    // 2. No rotation: just check the position.
    if target == curr_orient {
        player.current.pos = pos;
        if !player
            .board
            .collides(&player.current.cells().collect::<Vec<_>>())
        {
            return Some(player.current);
        }
        player.current.orientation = orig_orient;
        player.current.pos = orig_pos;
        return None;
    }

    // 3. Determine rotation direction (CW or CCW) from current → target.
    let rot = if curr_orient.rotate(Rotation::CW) == target {
        Rotation::CW
    } else if curr_orient.rotate(Rotation::CCW) == target {
        Rotation::CCW
    } else {
        // 180° rotation isn't valid in SRS.
        return None;
    };

    // 4. Try each SRS kick offset.
    let kicks = ruleset.kicks(mv.location.kind, target, rot);
    for kick in kicks {
        player.current.orientation = target;
        player.current.pos = pos + *kick;
        if !player
            .board
            .collides(&player.current.cells().collect::<Vec<_>>())
        {
            return Some(player.current);
        }
    }

    // 5. All kicks failed — revert and return None.
    player.current.orientation = orig_orient;
    player.current.pos = orig_pos;
    None
}
