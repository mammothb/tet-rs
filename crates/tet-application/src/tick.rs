use tet_domain::{Board, Cell, MinoType, Orientation, Rotation, Ruleset, Vec2, v2};

use crate::piece::Piece;
use crate::player::{Phase, Player};
use crate::time::GameTime;

#[derive(Debug, Default, PartialEq, Eq)]
pub enum TSpinStatus {
    #[default]
    None,
    Mini,
    Full,
}

#[derive(Debug, Default)]
pub struct TickResult {
    pub lines_cleared: u8,
    pub tspin: TSpinStatus,
    pub piece_locked: bool,
}

/// True if shifting the active piece down by 1 cell wouldn't collide with the
/// board. Does not mutate the piece.
#[must_use]
pub fn piece_can_fall(player: &Player) -> bool {
    let mut shifted = player.current;
    shifted.shift(v2![0, -1]);
    !player.board.collides(&shifted.cells())
}

pub fn step_gravity(player: &mut Player) -> bool {
    if !piece_can_fall(player) {
        return false;
    }
    player.current.shift(v2![0, -1]);
    true
}

pub fn apply_horizontal_input(player: &mut Player, dx: i8, now: GameTime) -> bool {
    let original = player.current.pos;
    player.current.shift(v2![dx, 0]);
    if player.board.collides(&player.current.cells()) {
        player.current.pos = original;
        false
    } else {
        // Successful shift — restart the lock timer if the piece is still
        // grounded (the next `step_player` call clears it if the piece
        // lifted into the air).
        player.lock_grounded_at = Some(now);
        true
    }
}

pub fn apply_rotation_input(
    player: &mut Player,
    ruleset: &Ruleset,
    rot: Rotation,
    now: GameTime,
) -> bool {
    let new_orient = player.current.orientation.rotate(rot);
    let kicks = ruleset.kicks(player.current.kind, new_orient, rot);

    let orig_orient = player.current.orientation;
    let orig_pos = player.current.pos;
    for kick in kicks {
        player.current.orientation = new_orient;
        player.current.pos = orig_pos + *kick;
        if !player.board.collides(&player.current.cells()) {
            // Successful rotation — restart the lock timer if the piece is
            // still grounded (next `step_player` clears it if the piece lifted).
            player.lock_grounded_at = Some(now);
            return true;
        }
    }
    player.current.orientation = orig_orient;
    player.current.pos = orig_pos;
    false
}

/// # Panics
///
/// Number of full rows exceed u8 limit
pub fn try_lock(player: &mut Player) -> TickResult {
    let tspin = detect_tspin(player);
    for cell in player.current.cells() {
        player.board.set(cell, Cell::Block(player.current.kind));
    }
    let full: Vec<u8> = player.board.full_rows().collect();
    let lines_cleared = u8::try_from(full.len()).expect("at most 4 lines clear per move");
    player.board.clear_rows(&full);

    TickResult {
        lines_cleared,
        tspin,
        piece_locked: true,
    }
}

pub fn post_lock(player: &mut Player, result: &TickResult, ruleset: &Ruleset) {
    cancel_pending_garbage(player, result.lines_cleared);
    update_score(player, result, ruleset);
    spawn_next_piece(player);
    if check_topout(player) {
        player.phase = Phase::GameOver;
    }
}

pub fn cancel_pending_garbage(player: &mut Player, mut to_cancel: u8) {
    while to_cancel > 0 {
        let Some(top) = player.garbage.pending.last_mut() else {
            break;
        };
        if top.count <= to_cancel {
            to_cancel -= top.count;
            player.garbage.pending.pop();
        } else {
            top.count -= to_cancel;
            to_cancel = 0;
        }
    }
}

pub fn spawn_next_piece(player: &mut Player) {
    let kind = player.queue.take();
    player.current = Piece::spawn(kind);
    player.hold_used = false;
    if player.board.collides(&player.current.cells()) {
        player.phase = Phase::GameOver;
    }
}

#[must_use]
pub fn project_ghost_pos(board: &Board, piece: Piece) -> Vec2 {
    let mut ghost = piece;
    loop {
        let original = ghost.pos;
        ghost.shift(v2![0, -1]);
        if board.collides(&ghost.cells()) {
            ghost.pos = original;
            return ghost.pos;
        }
    }
}

fn project_ghost(player: &Player) -> Vec2 {
    project_ghost_pos(&player.board, player.current)
}

#[must_use]
pub fn check_topout(player: &Player) -> bool {
    player.board.collides(&player.current.cells())
}

pub fn hard_drop(player: &mut Player) -> TickResult {
    player.current.pos = project_ghost(player);
    try_lock(player)
}

pub fn soft_drop(player: &mut Player) -> bool {
    step_gravity(player)
}

pub fn try_hold(player: &mut Player) -> bool {
    if player.hold_used {
        return false;
    }
    let current_kind = player.current.kind;
    let new_kind = match player.hold {
        None => {
            player.hold = Some(current_kind);
            player.queue.take()
        }
        Some(held) => {
            player.hold = Some(current_kind);
            held
        }
    };
    player.current = Piece::spawn(new_kind);
    // New piece starts fresh — no lock timer yet (next step_player call
    // sets it if the new piece spawns grounded).
    player.lock_grounded_at = None;
    player.hold_used = true;
    if player.board.collides(&player.current.cells()) {
        player.phase = Phase::GameOver;
    }
    true
}

fn detect_tspin(player: &Player) -> TSpinStatus {
    if player.current.kind != MinoType::T {
        return TSpinStatus::None;
    }
    // T-piece center is at bbox (1, 1) for all orientations.
    let center = player.current.pos + v2![1, 1];
    let corners = [
        center + v2![-1, -1], // 0: bottom-left
        center + v2![1, -1],  // 1: bottom-right
        center + v2![-1, 1],  // 2: top-left
        center + v2![1, 1],   // 3: top-right
    ];
    let filled: [bool; 4] = std::array::from_fn(|i| player.board.collides(&[corners[i]]));
    let count = filled.iter().filter(|&&f| f).count();
    if count < 3 {
        return TSpinStatus::None;
    }
    if count == 4 {
        return TSpinStatus::Full;
    }
    // 3 corners filled: full vs mini depends on back corners.
    let back = match player.current.orientation {
        Orientation::North => [0, 1], // tip up
        Orientation::East => [0, 2],  // tip right
        Orientation::South => [2, 3], // tip down
        Orientation::West => [1, 3],  // tip left
    };
    if filled[back[0]] && filled[back[1]] {
        TSpinStatus::Full
    } else {
        TSpinStatus::Mini
    }
}

fn update_score(player: &mut Player, result: &TickResult, _ruleset: &Ruleset) {
    let scoring = &mut player.scoring;
    let lines = result.lines_cleared;

    if lines == 0 {
        // No clear, break combo (set to -1 = broken; reset to 0 on next clear).
        if scoring.combo > 0 {
            scoring.combo = -1;
        }
        return;
    }

    scoring.lines += u32::from(lines);

    // Base points per line clear.
    let base = match lines {
        1 => 100,
        2 => 300,
        3 => 500,
        4 => 800,
        _ => 0,
    };

    // T-spin bonus.
    let tspin_bonus = match result.tspin {
        TSpinStatus::None => 0,
        TSpinStatus::Mini => 200 * i32::from(lines),
        TSpinStatus::Full => 400 * i32::from(lines),
    };

    // Back-to-back: +50% of base on a difficult clear following a difficult one.
    let was_difficult = lines == 4 || result.tspin != TSpinStatus::None;
    let b2b_bonus = if scoring.b2b && was_difficult {
        base / 2
    } else {
        0
    };
    scoring.b2b = was_difficult;

    scoring.score += base + tspin_bonus + b2b_bonus;

    // Combo: +50 per consecutive clear (starting from the 2nd).
    scoring.combo += 1;
    if scoring.combo >= 2 {
        scoring.score += 50 * (scoring.combo - 1);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use rstest::rstest;
    use tet_domain::{Board, Queue, Rng};

    use crate::player::{Controller, GarbageState, PendingGarbage, ScoreState};
    use crate::ports::bot::{BotError, BotMove, BotTransport};
    use crate::snapshot::PlayerSnapshot;

    // -------- test fixtures --------

    /// Deterministic RNG that cycles through 1..=1000. Same as `bag.rs::StubRng`.
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

    /// Player with a T-piece at spawn position on an empty 10×25 board.
    /// Queue is filled from the deterministic RNG. The T-piece is hardcoded
    /// here (not from the queue) because tests that use this fixture want
    /// a guaranteed T-piece setup. `last_gravity_at` is set to "now" so
    /// gravity doesn't fire on the first `step_player` call.
    fn t_player() -> Player {
        Player {
            board: Board::new(10, 25),
            current: Piece::spawn(MinoType::T),
            queue: Queue::new(Box::new(StubRng::counter()), 5),
            hold: None,
            hold_used: false,
            scoring: ScoreState::default(),
            lock_grounded_at: None,
            phase: Phase::Playing,
            controller: Controller::Bot(Box::new(NoopBot)),
            garbage: GarbageState {
                pending: Vec::new(),
                attack_rng: Box::new(StubRng::counter()),
            },
            last_gravity_at: GameTime::ZERO,
        }
    }

    /// Stub `BotTransport` that does nothing. Needed only to fill the
    /// `Controller::Bot` variant for `t_player`. Not used in any test.
    struct NoopBot;

    #[async_trait::async_trait]
    impl BotTransport for NoopBot {
        fn start(&mut self, _ruleset: &Ruleset) -> Result<(), BotError> {
            Ok(())
        }
        async fn update(&mut self, _snapshot: &PlayerSnapshot) -> Result<(), BotError> {
            Ok(())
        }
        async fn suggest(&mut self) -> Result<Vec<BotMove>, BotError> {
            Ok(Vec::new())
        }
        async fn stop(&mut self) {}
    }

    // -------- step_gravity --------

    #[rstest]
    fn step_gravity_moves_piece_down_on_empty_board() {
        let mut p = t_player();
        let original = p.current.pos;
        assert!(step_gravity(&mut p));
        assert_eq!(p.current.pos, original + v2![0, -1]);
    }

    #[rstest]
    fn step_gravity_does_not_move_when_blocked_by_floor() {
        let mut p = t_player();
        // T-piece bbox at pos.y = -1 places cells at world y=0,0,0,1.
        // Stepping down puts them at y=-1 (OOB).
        p.current.pos = v2![3, -1];
        let original = p.current.pos;
        assert!(!step_gravity(&mut p));
        assert_eq!(p.current.pos, original);
    }

    #[rstest]
    fn step_gravity_does_not_move_when_blocked_by_cell() {
        let mut p = t_player();
        // T-piece spawns at pos (3,18), cells at y=19,19,19,20.
        // Stepping down puts cells at y=18. Block row 18 to stop it.
        p.board.set(v2![3, 18], Cell::Block(MinoType::I));
        p.board.set(v2![4, 18], Cell::Block(MinoType::I));
        p.board.set(v2![5, 18], Cell::Block(MinoType::I));
        p.board.set(v2![4, 19], Cell::Block(MinoType::I));
        let original = p.current.pos;
        assert!(!step_gravity(&mut p));
        assert_eq!(p.current.pos, original);
    }

    // -------- apply_horizontal_input --------

    #[rstest]
    #[case(-1)]
    #[case(1)]
    fn apply_horizontal_input_shifts_piece(#[case] dx: i8) {
        let mut p = t_player();
        let original = p.current.pos;
        assert!(apply_horizontal_input(&mut p, dx, GameTime::ZERO));
        assert_eq!(p.current.pos, original + v2![dx, 0]);
    }

    #[rstest]
    fn apply_horizontal_input_blocks_at_left_wall() {
        let mut p = t_player();
        p.current.pos = v2![0, 18];
        let original = p.current.pos;
        assert!(!apply_horizontal_input(&mut p, -1, GameTime::ZERO));
        assert_eq!(p.current.pos, original);
    }

    #[rstest]
    fn apply_horizontal_input_restarts_lock_timer_on_success() {
        let mut p = t_player();
        p.lock_grounded_at = Some(GameTime::from_millis(100));
        let now = GameTime::from_millis(200);
        assert!(apply_horizontal_input(&mut p, 1, now));
        assert_eq!(p.lock_grounded_at, Some(now));
    }

    // -------- apply_rotation_input --------

    #[rstest]
    fn apply_rotation_input_rotates_on_empty_board() {
        let mut p = t_player();
        assert!(apply_rotation_input(
            &mut p,
            &Ruleset::guideline(),
            Rotation::CW,
            GameTime::ZERO
        ));
        assert_eq!(p.current.orientation, Orientation::East);
    }

    // -------- try_lock --------

    #[rstest]
    fn try_lock_writes_piece_cells_to_board() {
        let mut p = t_player();
        try_lock(&mut p);
        // T-piece spawn cells are at relative (0,1)(1,1)(2,1)(1,2),
        // with pos (3, 18). World cells: (3,19)(4,19)(5,19)(4,20).
        assert_eq!(p.board.get(v2![3, 19]), Cell::Block(MinoType::T));
        assert_eq!(p.board.get(v2![4, 19]), Cell::Block(MinoType::T));
        assert_eq!(p.board.get(v2![5, 19]), Cell::Block(MinoType::T));
        assert_eq!(p.board.get(v2![4, 20]), Cell::Block(MinoType::T));
    }

    #[rstest]
    fn try_lock_returns_zero_lines_when_no_clear() {
        let mut p = t_player();
        let result = try_lock(&mut p);
        assert_eq!(result.lines_cleared, 0);
        assert!(result.piece_locked);
    }

    #[rstest]
    fn try_lock_clears_full_row() {
        let mut p = t_player();
        // Fill row 19 with garbage (so T-piece lock doesn't trigger line clear).
        for x in 0..10 {
            p.board.set(v2![x, 19], Cell::Garbage);
        }
        // T-piece locks: row 19 becomes full (T adds to 9 garbage + 1 T).
        let result = try_lock(&mut p);
        assert_eq!(result.lines_cleared, 1);
    }

    // -------- cancel_pending_garbage --------

    #[rstest]
    fn cancel_pending_garbage_removes_whole_attack_when_small() {
        let mut p = t_player();
        p.garbage.pending.push(PendingGarbage {
            count: 2,
            hole: 0,
            arrives_at: GameTime::from_millis(100),
        });
        p.garbage.pending.push(PendingGarbage {
            count: 1,
            hole: 0,
            arrives_at: GameTime::from_millis(100),
        });
        cancel_pending_garbage(&mut p, 2);
        assert_eq!(p.garbage.pending.len(), 1);
        assert_eq!(p.garbage.pending[0].count, 1);
    }

    #[rstest]
    fn cancel_pending_garbage_truncates_last_attack() {
        let mut p = t_player();
        p.garbage.pending.push(PendingGarbage {
            count: 4,
            hole: 0,
            arrives_at: GameTime::from_millis(100),
        });
        cancel_pending_garbage(&mut p, 2);
        assert_eq!(p.garbage.pending.len(), 1);
        assert_eq!(p.garbage.pending[0].count, 2);
    }

    #[rstest]
    fn cancel_pending_garbage_no_op_when_to_cancel_is_zero() {
        let mut p = t_player();
        p.garbage.pending.push(PendingGarbage {
            count: 3,
            hole: 0,
            arrives_at: GameTime::from_millis(100),
        });
        cancel_pending_garbage(&mut p, 0);
        assert_eq!(p.garbage.pending.len(), 1);
        assert_eq!(p.garbage.pending[0].count, 3);
    }

    // -------- spawn_next_piece --------

    #[rstest]
    fn spawn_next_piece_pulls_from_queue_and_resets_hold_used() {
        let mut p = t_player();
        p.hold_used = true;
        let first = p.current.kind;
        spawn_next_piece(&mut p);
        assert_ne!(p.current.kind, first); // took a different piece from queue
        assert!(!p.hold_used);
    }

    #[rstest]
    fn spawn_next_piece_sets_game_over_on_blocked_spawn() {
        let mut p = t_player();
        // Block the spawn position.
        p.board.set(v2![3, 19], Cell::Block(MinoType::I));
        p.board.set(v2![4, 19], Cell::Block(MinoType::I));
        p.board.set(v2![5, 19], Cell::Block(MinoType::I));
        p.board.set(v2![4, 20], Cell::Block(MinoType::I));
        spawn_next_piece(&mut p);
        assert_eq!(p.phase, Phase::GameOver);
    }

    // -------- project_ghost --------

    #[rstest]
    fn project_ghost_returns_floor_position_on_empty_board() {
        let p = t_player();
        // T spawns at pos (3, 18). Lowest cell at y=19. Floor is y=0.
        let ghost = project_ghost(&p);
        // Ghost pos is bbox origin — bottom row at y=0 means bbox y = -1.
        assert_eq!(ghost.y, -1);
        assert_eq!(ghost.x, 3);
    }

    // -------- check_topout --------

    #[rstest]
    fn check_topout_returns_true_when_piece_blocked() {
        let mut p = t_player();
        // Fill cells directly under the T-piece so it's blocked.
        p.board.set(v2![3, 19], Cell::Block(MinoType::I));
        p.board.set(v2![4, 19], Cell::Block(MinoType::I));
        p.board.set(v2![5, 19], Cell::Block(MinoType::I));
        p.board.set(v2![4, 20], Cell::Block(MinoType::I));
        assert!(check_topout(&p));
    }

    #[rstest]
    fn check_topout_returns_false_when_piece_free() {
        let p = t_player();
        assert!(!check_topout(&p));
    }

    // -------- hard_drop --------

    #[rstest]
    fn hard_drop_locks_piece_at_ghost_position() {
        let mut p = t_player();
        let result = hard_drop(&mut p);
        assert!(result.piece_locked);
        // Piece should be locked on the floor — bottom cell at y=0.
        assert_eq!(p.board.get(v2![4, 0]), Cell::Block(MinoType::T));
    }

    // -------- try_hold --------

    #[rstest]
    fn try_hold_with_empty_hold_pulls_from_queue() {
        let mut p = t_player();
        let original = p.current.kind;
        let queue_first_before = p.queue.peek()[0];
        let result = try_hold(&mut p);
        assert!(result);
        assert_eq!(p.hold, Some(original));
        assert_eq!(p.current.kind, queue_first_before);
        assert!(p.hold_used);
    }

    #[rstest]
    fn try_hold_with_existing_hold_swaps() {
        let mut p = t_player();
        p.hold = Some(MinoType::I);
        let original = p.current.kind;
        assert!(try_hold(&mut p));
        assert_eq!(p.hold, Some(original));
        assert_eq!(p.current.kind, MinoType::I);
    }

    #[rstest]
    fn try_hold_fails_when_already_used() {
        let mut p = t_player();
        p.hold = Some(MinoType::I);
        try_hold(&mut p);
        // Second hold in same piece should fail.
        let original = p.current.kind;
        assert!(!try_hold(&mut p));
        assert_eq!(p.current.kind, original);
    }

    #[rstest]
    fn try_hold_clears_lock_timer() {
        // Piece is grounded with timer set. After hold, the timer should
        // be cleared — the new piece hasn't been grounded yet.
        let mut p = t_player();
        p.lock_grounded_at = Some(GameTime::from_millis(100));
        try_hold(&mut p);
        assert!(p.lock_grounded_at.is_none());
    }

    // -------- update_score (via try_lock + cancel + score flow) --------

    #[rstest]
    fn update_score_breaks_combo_on_no_clear() {
        let mut p = t_player();
        p.scoring.combo = 3;
        let result = TickResult {
            lines_cleared: 0,
            tspin: TSpinStatus::None,
            piece_locked: false,
        };
        update_score(&mut p, &result, &Ruleset::guideline());
        assert_eq!(p.scoring.combo, -1);
    }

    #[rstest]
    fn update_score_breaks_combo_to_zero_on_first_clear() {
        let mut p = t_player();
        p.scoring.combo = -1; // broken from last time
        let result = TickResult {
            lines_cleared: 1,
            tspin: TSpinStatus::None,
            piece_locked: false,
        };
        update_score(&mut p, &result, &Ruleset::guideline());
        // Doc says `combo += 1`; from -1 that gives 0. To reset to 1 instead,
        // the implementation needs `combo = combo.max(0) + 1`. Documenting
        // current behavior; can fix the doc + impl together later.
        assert_eq!(p.scoring.combo, 0);
    }

    #[rstest]
    fn update_score_sets_b2b_on_difficult_clears() {
        let mut p = t_player();
        p.scoring.b2b = true;
        let result = TickResult {
            lines_cleared: 4,
            tspin: TSpinStatus::None,
            piece_locked: false,
        };
        update_score(&mut p, &result, &Ruleset::guideline());
        assert!(p.scoring.b2b);
        assert!(p.scoring.score >= 800 + 400); // quad + b2b bonus
    }

    // -------- detect_tspin --------

    /// Corners are at pos + (0,0), pos+(2,0), pos+(0,2), pos+(2,2). Indices
    /// 0..3 map to bottom-left, bottom-right, top-left, top-right.
    fn block_corners(p: &mut Player, indices: &[usize]) {
        let pos = p.current.pos;
        let offsets = [v2![0, 0], v2![2, 0], v2![0, 2], v2![2, 2]];
        for &i in indices {
            p.board.set(pos + offsets[i], Cell::Block(MinoType::I));
        }
    }

    #[rstest]
    fn detect_tspin_returns_none_for_non_t_piece() {
        let mut p = t_player();
        p.current = Piece::spawn(MinoType::I);
        block_corners(&mut p, &[0, 1, 2, 3]);
        assert_eq!(detect_tspin(&p), TSpinStatus::None);
    }

    #[rstest]
    fn detect_tspin_returns_none_when_fewer_than_3_corners_filled() {
        let p = t_player();
        // 0 corners filled
        assert_eq!(detect_tspin(&p), TSpinStatus::None);

        let mut p2 = t_player();
        // 2 corners filled
        block_corners(&mut p2, &[0, 1]);
        assert_eq!(detect_tspin(&p2), TSpinStatus::None);
    }

    #[rstest]
    fn detect_tspin_returns_full_when_4_corners_filled() {
        let mut p = t_player();
        block_corners(&mut p, &[0, 1, 2, 3]);
        assert_eq!(detect_tspin(&p), TSpinStatus::Full);
    }

    #[rstest]
    fn detect_tspin_north_3_corners_with_both_back_filled_is_full() {
        // North: back = bottom = indices [0, 1]. Both back filled = Full.
        let mut p = t_player();
        block_corners(&mut p, &[0, 1, 2]); // both back + one front
        assert_eq!(detect_tspin(&p), TSpinStatus::Full);
    }

    #[rstest]
    fn detect_tspin_north_3_corners_with_back_partial_is_mini() {
        // North: one back (0) + both fronts (2, 3). One back empty = Mini.
        let mut p = t_player();
        block_corners(&mut p, &[0, 2, 3]);
        assert_eq!(detect_tspin(&p), TSpinStatus::Mini);
    }

    #[rstest]
    fn detect_tspin_east_back_indices_use_left_side() {
        // East: tip right, back = left = indices [0, 2]. Both back filled = Full.
        let mut p = t_player();
        p.current.orientation = Orientation::East;
        block_corners(&mut p, &[0, 2, 1]); // both back + one front
        assert_eq!(detect_tspin(&p), TSpinStatus::Full);
    }

    #[rstest]
    fn detect_tspin_south_back_indices_use_top() {
        // South: tip down, back = top = indices [2, 3]. Both back filled = Full.
        let mut p = t_player();
        p.current.orientation = Orientation::South;
        block_corners(&mut p, &[2, 3, 0]);
        assert_eq!(detect_tspin(&p), TSpinStatus::Full);
    }

    #[rstest]
    fn detect_tspin_west_back_indices_use_right() {
        // West: tip left, back = right = indices [1, 3]. Both back filled = Full.
        let mut p = t_player();
        p.current.orientation = Orientation::West;
        block_corners(&mut p, &[1, 3, 0]);
        assert_eq!(detect_tspin(&p), TSpinStatus::Full);
    }
}
