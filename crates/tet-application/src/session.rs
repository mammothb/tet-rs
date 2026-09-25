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

#[cfg(test)]
mod test {
    use super::*;

    use rstest::rstest;

    use crate::TSpinStatus;
    use tet_domain::{Board, Cell, MinoType, Orientation, Queue};

    use crate::PendingGarbage;
    use crate::player::{Controller, Player};
    use crate::ports::bot::{BotError, Move, PieceLocation, Spin};

    /// Deterministic RNG that cycles through 1..=1000. Same as bag.rs / tick.rs.
    struct StubRng {
        values: Vec<u32>,
        idx: usize,
    }

    impl StubRng {
        fn counter() -> Self {
            Self {
                values: (1..=1000).collect(),
                idx: 0,
            }
        }
    }

    impl Rng for StubRng {
        fn next_u32(&mut self) -> u32 {
            let v = self.values[self.idx % self.values.len()];
            self.idx += 1;
            v
        }
    }

    /// Stub bot that returns preconfigured moves and counts calls.
    struct StubBot {
        moves: Vec<Move>,
        updates: usize,
        suggests: usize,
        stops: usize,
        fail_suggest: bool,
    }

    impl StubBot {
        fn new(moves: Vec<Move>) -> Self {
            Self {
                moves,
                updates: 0,
                suggests: 0,
                stops: 0,
                fail_suggest: false,
            }
        }
        fn empty() -> Self {
            Self::new(Vec::new())
        }
        fn fail() -> Self {
            Self {
                moves: Vec::new(),
                updates: 0,
                suggests: 0,
                stops: 0,
                fail_suggest: true,
            }
        }
    }

    impl BotTransport for StubBot {
        fn start(&mut self, _ruleset: &Ruleset) -> Result<(), BotError> {
            Ok(())
        }
        fn update(&mut self, _snapshot: &PlayerSnapshot) -> Result<(), BotError> {
            self.updates += 1;
            Ok(())
        }
        fn suggest(&mut self) -> Result<Vec<Move>, BotError> {
            self.suggests += 1;
            if self.fail_suggest {
                Err(BotError::Exited)
            } else {
                Ok(std::mem::take(&mut self.moves))
            }
        }
        fn stop(&mut self) {
            self.stops += 1;
        }
    }

    fn empty_player() -> Player<StubRng> {
        Player {
            board: Board::new(10, 25),
            current: Piece::spawn(MinoType::T),
            queue: Queue::new(StubRng::counter(), 5),
            hold: None,
            hold_used: false,
            score: 0,
            lines: 0,
            combo: 0,
            b2b: false,
            lock_delay: 0,
            phase: Phase::Playing,
            controller: Controller::Bot(Box::new(StubBot::empty())),
            pending_garbage: Vec::new(),
            attack_rng: StubRng::counter(),
        }
    }

    fn t_session() -> GameSession<StubRng> {
        let mut session = GameSession::new(Ruleset::guideline());
        session.add_player(empty_player());
        session
    }

    // -------- new / add_player / frame counter --------

    #[rstest]
    fn new_session_has_no_players_and_zero_frame() {
        let session: GameSession<StubRng> = GameSession::new(Ruleset::guideline());
        assert_eq!(session.players.len(), 0);
        assert_eq!(session.frame, 0);
    }

    #[rstest]
    fn add_player_returns_sequential_indices() {
        let mut session: GameSession<StubRng> = GameSession::new(Ruleset::guideline());
        assert_eq!(session.add_player(empty_player()), 0);
        assert_eq!(session.add_player(empty_player()), 1);
        assert_eq!(session.add_player(empty_player()), 2);
        assert_eq!(session.players.len(), 3);
    }

    #[rstest]
    fn step_frame_increments_frame_counter() {
        let mut session = t_session();
        assert_eq!(session.frame, 0);
        session.step_frame();
        assert_eq!(session.frame, 1);
        session.step_frame();
        assert_eq!(session.frame, 2);
    }

    // -------- step_frame: pending garbage --------

    #[rstest]
    fn step_frame_decrements_pending_garbage_delay() {
        let mut session = t_session();
        session.players[0].pending_garbage.push(PendingGarbage {
            count: 2,
            hole: 5,
            delay_remaining: 5,
        });
        session.step_frame();
        assert_eq!(session.players[0].pending_garbage[0].delay_remaining, 4);
        assert_eq!(session.players[0].pending_garbage.len(), 1);
    }

    #[rstest]
    fn step_frame_inserts_garbage_when_delay_hits_zero() {
        let mut session = t_session();
        session.players[0].pending_garbage.push(PendingGarbage {
            count: 1,
            hole: 0,
            delay_remaining: 1,
        });
        session.step_frame();
        // Pending drained
        assert!(session.players[0].pending_garbage.is_empty());
        // Board has garbage with hole at col 0
        assert_eq!(session.players[0].board.get(v2![0, 0]), Cell::Empty);
        assert_eq!(session.players[0].board.get(v2![1, 0]), Cell::Garbage);
    }

    #[rstest]
    fn step_frame_advances_lock_delay_when_piece_cant_move_down() {
        let mut session = t_session();
        // T-piece bbox at pos.y = -1 places cells at world y=0,0,0,1.
        // Stepping down would put them at y=-1 (OOB).
        session.players[0].current.pos = v2![3, -1];
        session.step_frame();
        assert_eq!(session.players[0].lock_delay, 1);
    }

    #[rstest]
    fn step_frame_skips_game_over_players() {
        let mut session = t_session();
        session.players[0].phase = Phase::GameOver;
        let initial_frame = session.frame;
        session.step_frame();
        assert_eq!(session.frame, initial_frame + 1);
        // Player's lock_delay was 0; stays 0 (skipped)
        assert_eq!(session.players[0].lock_delay, 0);
    }

    // -------- snapshot --------

    #[rstest]
    fn snapshot_captures_combo_and_b2b() {
        let mut session = t_session();
        session.players[0].combo = 3;
        session.players[0].b2b = true;
        let snap = session.snapshot(0);
        assert_eq!(snap.combo, 3);
        assert!(snap.b2b);
    }

    #[rstest]
    fn snapshot_clones_board_so_session_changes_dont_leak() {
        let mut session = t_session();
        let snap = session.snapshot(0);
        session.players[0]
            .board
            .set(v2![3, 5], Cell::Block(MinoType::T));
        assert_eq!(snap.board.get(v2![3, 5]), Cell::Empty);
    }

    #[rstest]
    fn snapshot_captures_pending_garbage() {
        let mut session = t_session();
        session.players[0].pending_garbage.push(PendingGarbage {
            count: 4,
            hole: 7,
            delay_remaining: 6,
        });
        let snap = session.snapshot(0);
        assert_eq!(snap.pending_garbage.len(), 1);
        assert_eq!(snap.pending_garbage[0].count, 4);
        assert_eq!(snap.pending_garbage[0].hole, 7);
        assert_eq!(snap.pending_garbage[0].delay_remaining, 6);
    }

    // -------- is_finished --------

    #[rstest]
    fn is_finished_false_when_any_player_still_playing() {
        let mut session = GameSession::new(Ruleset::guideline());
        session.add_player(empty_player());
        session.add_player(empty_player());
        session.players[0].phase = Phase::GameOver;
        assert!(!session.is_finished());
    }

    #[rstest]
    fn is_finished_true_when_all_players_game_over() {
        let mut session = GameSession::new(Ruleset::guideline());
        session.add_player(empty_player());
        session.add_player(empty_player());
        session.players[0].phase = Phase::GameOver;
        session.players[1].phase = Phase::GameOver;
        assert!(session.is_finished());
    }

    // -------- distribute_garbage --------

    #[rstest]
    fn distribute_garbage_no_attack_for_zero_lines() {
        let mut session = GameSession::new(Ruleset::guideline());
        session.add_player(empty_player());
        session.add_player(empty_player());
        let results = vec![
            TickResult {
                lines_cleared: 0,
                tspin: TSpinStatus::None,
                piece_locked: true,
            },
            TickResult {
                lines_cleared: 0,
                tspin: TSpinStatus::None,
                piece_locked: false,
            },
        ];
        session.distribute_garbage(&results);
        assert!(session.players[0].pending_garbage.is_empty());
        assert!(session.players[1].pending_garbage.is_empty());
    }

    #[rstest]
    fn distribute_garbage_attack_count_per_guideline_table() {
        // 2 lines → 1 garbage, 3 lines → 2, 4 lines → 4.
        for &(lines, expected_rows) in &[(1, 0), (2, 1), (3, 2), (4, 4)] {
            let mut session = GameSession::new(Ruleset::guideline());
            session.add_player(empty_player());
            session.add_player(empty_player());
            let results = vec![
                TickResult {
                    lines_cleared: lines,
                    tspin: TSpinStatus::None,
                    piece_locked: true,
                },
                TickResult {
                    lines_cleared: 0,
                    tspin: TSpinStatus::None,
                    piece_locked: false,
                },
            ];
            session.distribute_garbage(&results);
            if expected_rows == 0 {
                assert!(
                    session.players[1].pending_garbage.is_empty(),
                    "lines={lines} should send 0 rows"
                );
            } else {
                assert_eq!(
                    session.players[1].pending_garbage.len(),
                    1,
                    "lines={lines} should send {expected_rows} rows"
                );
                assert_eq!(session.players[1].pending_garbage[0].count, expected_rows);
            }
        }
    }

    #[rstest]
    fn distribute_garbage_skips_attacker() {
        let mut session = GameSession::new(Ruleset::guideline());
        session.add_player(empty_player());
        session.add_player(empty_player());
        // Player 0 attacks; player 0 should NOT receive its own attack.
        let results = vec![
            TickResult {
                lines_cleared: 4,
                tspin: TSpinStatus::None,
                piece_locked: true,
            },
            TickResult {
                lines_cleared: 0,
                tspin: TSpinStatus::None,
                piece_locked: false,
            },
        ];
        session.distribute_garbage(&results);
        assert_eq!(session.players[0].pending_garbage.len(), 0); // attacker
        assert_eq!(session.players[1].pending_garbage.len(), 1); // opponent
    }

    #[rstest]
    fn distribute_garbage_3_player_each_attacks_other_two() {
        let mut session = GameSession::new(Ruleset::guideline());
        session.add_player(empty_player());
        session.add_player(empty_player());
        session.add_player(empty_player());
        let results = vec![
            TickResult {
                lines_cleared: 2,
                tspin: TSpinStatus::None,
                piece_locked: true,
            }, // p0
            TickResult {
                lines_cleared: 2,
                tspin: TSpinStatus::None,
                piece_locked: true,
            }, // p1
            TickResult {
                lines_cleared: 0,
                tspin: TSpinStatus::None,
                piece_locked: false,
            }, // p2
        ];
        session.distribute_garbage(&results);
        // p0 receives from p1
        assert_eq!(session.players[0].pending_garbage.len(), 1);
        // p1 receives from p0
        assert_eq!(session.players[1].pending_garbage.len(), 1);
        // p2 receives from both p0 and p1
        assert_eq!(session.players[2].pending_garbage.len(), 2);
    }

    #[rstest]
    fn distribute_garbage_attack_has_garbage_delay_frames() {
        let mut session = GameSession::new(Ruleset::guideline());
        session.add_player(empty_player());
        session.add_player(empty_player());
        let results = vec![
            TickResult {
                lines_cleared: 2,
                tspin: TSpinStatus::None,
                piece_locked: true,
            },
            TickResult {
                lines_cleared: 0,
                tspin: TSpinStatus::None,
                piece_locked: false,
            },
        ];
        session.distribute_garbage(&results);
        assert_eq!(
            session.players[1].pending_garbage[0].delay_remaining,
            GARBAGE_DELAY_FRAMES
        );
    }

    #[rstest]
    fn distribute_garbage_hole_uses_attacker_rng() {
        let mut p0 = empty_player();
        // Force attack_rng to return a specific value.
        p0.attack_rng = StubRng {
            values: vec![42],
            idx: 0,
        };
        let mut p1 = empty_player();
        p1.attack_rng = StubRng {
            values: vec![7],
            idx: 0,
        };
        let mut session = GameSession::new(Ruleset::guideline());
        session.add_player(p0);
        session.add_player(p1);
        let results = vec![
            TickResult {
                lines_cleared: 2,
                tspin: TSpinStatus::None,
                piece_locked: true,
            },
            TickResult {
                lines_cleared: 2,
                tspin: TSpinStatus::None,
                piece_locked: true,
            },
        ];
        session.distribute_garbage(&results);
        // p0's attack_rng returned 42 → 42 % 10 = 2 (sent to p1)
        assert_eq!(session.players[1].pending_garbage[0].hole, 2);
        // p1's attack_rng returned 7 → 7 % 10 = 7 (sent to p0)
        assert_eq!(session.players[0].pending_garbage[0].hole, 7);
    }

    // -------- apply_input --------

    #[rstest]
    fn apply_input_none_returns_none_and_no_change() {
        let mut session = t_session();
        let original_pos = session.players[0].current.pos;
        let result = session.apply_input(0, Input::None);
        assert!(result.is_none());
        assert_eq!(session.players[0].current.pos, original_pos);
    }

    #[rstest]
    fn apply_input_move_left_shifts_piece_left() {
        let mut session = t_session();
        let original_pos = session.players[0].current.pos;
        let result = session.apply_input(0, Input::MoveLeft);
        assert!(result.is_none());
        assert_eq!(session.players[0].current.pos, original_pos + v2![-1, 0]);
    }

    #[rstest]
    fn apply_input_rotate_cw_advances_orientation() {
        let mut session = t_session();
        let _ = session.apply_input(0, Input::RotateCW);
        assert_eq!(session.players[0].current.orientation, Orientation::East);
    }

    #[rstest]
    fn apply_input_move_right_shifts_piece_right() {
        let mut session = t_session();
        let original_pos = session.players[0].current.pos;
        let result = session.apply_input(0, Input::MoveRight);
        assert!(result.is_none());
        assert_eq!(session.players[0].current.pos, original_pos + v2![1, 0]);
    }

    #[rstest]
    fn apply_input_rotate_ccw_retreats_orientation() {
        let mut session = t_session();
        let _ = session.apply_input(0, Input::RotateCCW);
        assert_eq!(session.players[0].current.orientation, Orientation::West);
    }

    #[rstest]
    fn apply_input_soft_drop_moves_piece_down() {
        let mut session = t_session();
        let original_pos = session.players[0].current.pos;
        let result = session.apply_input(0, Input::SoftDrop);
        assert!(result.is_none());
        assert_eq!(session.players[0].current.pos, original_pos + v2![0, -1]);
    }

    #[rstest]
    fn apply_input_hold_with_empty_hold_pulls_from_queue() {
        let mut session = t_session();
        assert!(session.players[0].hold.is_none());
        let result = session.apply_input(0, Input::Hold);
        assert!(result.is_none());
        assert!(session.players[0].hold.is_some());
        assert!(session.players[0].hold_used);
    }

    #[rstest]
    fn apply_input_hold_fails_when_already_used() {
        let mut session = t_session();
        session.players[0].hold_used = true;
        let result = session.apply_input(0, Input::Hold);
        assert!(result.is_none());
        // hold stays empty — try_hold returns false on already-used
        assert!(session.players[0].hold.is_none());
    }

    #[rstest]
    fn apply_input_hard_drop_returns_some_with_piece_locked() {
        let mut session = t_session();
        let result = session.apply_input(0, Input::HardDrop);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.piece_locked);
    }

    #[rstest]
    fn apply_input_on_game_over_player_returns_none() {
        let mut session = t_session();
        session.players[0].phase = Phase::GameOver;
        let result = session.apply_input(0, Input::MoveLeft);
        assert!(result.is_none());
    }

    // -------- step_bot --------

    #[rstest]
    fn step_bot_calls_update_then_suggest() {
        let mut session = t_session();
        let mut bot = StubBot::empty();
        session.step_bot(0, &mut bot);
        assert_eq!(bot.updates, 1);
        assert_eq!(bot.suggests, 1);
    }

    #[rstest]
    fn step_bot_with_failing_suggest_returns_silently() {
        let mut session = t_session();
        let mut bot = StubBot::fail();
        // Should not panic; just returns.
        session.step_bot(0, &mut bot);
        assert_eq!(bot.updates, 1);
        assert_eq!(bot.suggests, 1);
    }

    #[rstest]
    fn step_bot_applies_first_valid_move() {
        let mut session = t_session();
        // Bot suggests a move that's a no-op (same orientation, valid position).
        let mv = Move {
            location: PieceLocation {
                kind: MinoType::T,
                orientation: Orientation::North,
                x: 3,
                y: 19,
            },
            spin: Spin::None,
        };
        let mut bot = StubBot::new(vec![mv]);
        session.step_bot(0, &mut bot);
        // Move was consumed (Vec was moved out)
        assert_eq!(bot.suggests, 1);
    }

    // -------- apply_bot_move --------

    fn bot_move(kind: MinoType, orient: Orientation, x: i8, y: i8) -> Move {
        Move {
            location: PieceLocation {
                kind,
                orientation: orient,
                x,
                y,
            },
            spin: Spin::None,
        }
    }

    #[rstest]
    fn apply_bot_move_no_rotation_translates_to_target_position() {
        let mut p = empty_player();
        // T-piece spawns at bbox (3, 18); center at (4, 19).
        // Bot asks for T at center (5, 19) → bbox anchor (4, 18).
        let mv = bot_move(MinoType::T, Orientation::North, 5, 19);
        let result = apply_bot_move(&mut p, mv, &Ruleset::guideline());
        assert!(result.is_some());
        assert_eq!(p.current.orientation, Orientation::North);
        assert_eq!(p.current.pos, v2![4, 18]);
    }

    #[rstest]
    fn apply_bot_move_cw_rotation_succeeds() {
        let mut p = empty_player();
        // T-piece spawns North, center at (4, 19). Bot asks for East, center (4, 19).
        // East bbox anchor = (4 - 1, 19 - 1) = (3, 18). Empty board, no collision.
        let mv = bot_move(MinoType::T, Orientation::East, 4, 19);
        let result = apply_bot_move(&mut p, mv, &Ruleset::guideline());
        assert!(result.is_some());
        assert_eq!(p.current.orientation, Orientation::East);
        assert_eq!(p.current.pos, v2![3, 18]);
    }

    #[rstest]
    fn apply_bot_move_ccw_rotation_succeeds() {
        let mut p = empty_player();
        let mv = bot_move(MinoType::T, Orientation::West, 4, 19);
        let result = apply_bot_move(&mut p, mv, &Ruleset::guideline());
        assert!(result.is_some());
        assert_eq!(p.current.orientation, Orientation::West);
        assert_eq!(p.current.pos, v2![3, 18]);
    }

    #[rstest]
    fn apply_bot_move_180_degree_rotation_returns_none() {
        let mut p = empty_player();
        // North → South is 180°, invalid in SRS.
        let mv = bot_move(MinoType::T, Orientation::South, 4, 19);
        let result = apply_bot_move(&mut p, mv, &Ruleset::guideline());
        assert!(result.is_none());
        // State unchanged
        assert_eq!(p.current.orientation, Orientation::North);
        assert_eq!(p.current.pos, v2![3, 18]);
    }

    #[rstest]
    fn apply_bot_move_collision_reverts_state_and_returns_none() {
        let mut p = empty_player();
        // T spawns at cells (3, 19), (4, 19), (5, 19), (4, 20). Block one.
        p.board.set(v2![3, 19], Cell::Block(MinoType::I));
        // Bot asks for T at center (4, 19) — same as spawn, but (3, 19) is taken.
        let mv = bot_move(MinoType::T, Orientation::North, 4, 19);
        let result = apply_bot_move(&mut p, mv, &Ruleset::guideline());
        assert!(result.is_none());
        // State reverted
        assert_eq!(p.current.orientation, Orientation::North);
        assert_eq!(p.current.pos, v2![3, 18]);
    }
}
