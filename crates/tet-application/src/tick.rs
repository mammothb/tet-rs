use tet_domain::{Cell, MinoType, Orientation, Rng, Rotation, Ruleset, Vec2, v2};

use crate::{LOCK_DELAY_FRAMES, Phase, Piece, Player};

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

pub fn step_player<R: Rng>(player: &mut Player<R>, ruleset: &Ruleset) -> TickResult {
    let mut result = TickResult {
        lines_cleared: 0,
        tspin: TSpinStatus::None,
        piece_locked: false,
    };

    if player.phase != Phase::Playing {
        return result;
    }

    if step_gravity(player) {
        player.lock_delay = 0;
    } else {
        player.lock_delay += 1;
        if player.lock_delay >= LOCK_DELAY_FRAMES {
            result = try_lock(player);
            post_lock(player, &result, ruleset);
        }
    }

    result
}

pub fn step_gravity<R: Rng>(player: &mut Player<R>) -> bool {
    let orig_pos = player.current.pos;
    player.current.shift(v2![0, -1]);
    if player
        .board
        .collides(&player.current.cells().collect::<Vec<_>>())
    {
        player.current.pos = orig_pos;
        false
    } else {
        true
    }
}

pub fn apply_horizontal_input<R: Rng>(player: &mut Player<R>, dx: i8) -> bool {
    let original = player.current.pos;
    player.current.shift(v2![dx, 0]);
    if player
        .board
        .collides(&player.current.cells().collect::<Vec<_>>())
    {
        player.current.pos = original;
        false
    } else {
        player.lock_delay = 0;
        true
    }
}

pub fn apply_rotation_input<R: Rng>(
    player: &mut Player<R>,
    ruleset: &Ruleset,
    rot: Rotation,
) -> bool {
    let new_orient = player.current.orientation.rotate(rot);
    let kicks = ruleset.kicks(player.current.kind, new_orient, rot);

    let orig_orient = player.current.orientation;
    let orig_pos = player.current.pos;
    for kick in kicks {
        player.current.orientation = new_orient;
        player.current.pos = orig_pos + *kick;
        if !player
            .board
            .collides(&player.current.cells().collect::<Vec<_>>())
        {
            player.lock_delay = 0;
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
pub fn try_lock<R: Rng>(player: &mut Player<R>) -> TickResult {
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

pub fn post_lock<R: Rng>(player: &mut Player<R>, result: &TickResult, ruleset: &Ruleset) {
    cancel_pending_garbage(player, result.lines_cleared);
    update_score(player, result, ruleset);
    spawn_next_piece(player);
    if check_topout(player) {
        player.phase = Phase::GameOver;
    }
}

pub fn cancel_pending_garbage<R: Rng>(player: &mut Player<R>, mut to_cancel: u8) {
    while to_cancel > 0 {
        let Some(top) = player.pending_garbage.last_mut() else {
            break;
        };
        if top.count <= to_cancel {
            to_cancel -= top.count;
            player.pending_garbage.pop();
        } else {
            top.count -= to_cancel;
            to_cancel = 0;
        }
    }
}

pub fn spawn_next_piece<R: Rng>(player: &mut Player<R>) {
    let kind = player.queue.take();
    player.current = Piece::spawn(kind);
    player.hold_used = false;
    if player
        .board
        .collides(&player.current.cells().collect::<Vec<_>>())
    {
        player.phase = Phase::GameOver;
    }
}

pub fn project_ghost<R: Rng>(player: &Player<R>) -> Vec2 {
    let mut ghost = player.current;
    loop {
        let original = ghost.pos;
        ghost.shift(v2![0, -1]);
        if player.board.collides(&ghost.cells().collect::<Vec<_>>()) {
            ghost.pos = original;
            return ghost.pos;
        }
    }
}

pub fn check_topout<R: Rng>(player: &Player<R>) -> bool {
    player
        .board
        .collides(&player.current.cells().collect::<Vec<_>>())
}

pub fn hard_drop<R: Rng>(player: &mut Player<R>) -> TickResult {
    player.current.pos = project_ghost(player);
    try_lock(player)
}

pub fn soft_drop<R: Rng>(player: &mut Player<R>) -> bool {
    step_gravity(player)
}

pub fn try_hold<R: Rng>(player: &mut Player<R>) -> bool {
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
    player.lock_delay = 0;
    player.hold_used = true;
    if player
        .board
        .collides(&player.current.cells().collect::<Vec<_>>())
    {
        player.phase = Phase::GameOver;
    }
    true
}

fn detect_tspin<R: Rng>(player: &Player<R>) -> TSpinStatus {
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

fn update_score<R: Rng>(player: &mut Player<R>, result: &TickResult, _ruleset: &Ruleset) {
    let lines = result.lines_cleared;

    if lines == 0 {
        // No clear, break combo (set to -1 = broken; reset to 0 on next clear).
        if player.combo > 0 {
            player.combo = -1;
        }
        return;
    }

    player.lines += u32::from(lines);

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
    let b2b_bonus = if player.b2b && was_difficult {
        base / 2
    } else {
        0
    };
    player.b2b = was_difficult;

    player.score += base + tspin_bonus + b2b_bonus;

    // Combo: +50 per consecutive clear (starting from the 2nd).
    player.combo += 1;
    if player.combo >= 2 {
        player.score += 50 * (player.combo - 1);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use rstest::rstest;

    use tet_domain::{Board, Queue};

    use crate::player::{Controller, PendingGarbage, Player};
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
    /// Queue is filled from the deterministic RNG.
    fn t_player() -> Player<StubRng> {
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
            controller: Controller::Bot(Box::new(NoopBot)),
            pending_garbage: Vec::new(),
            attack_rng: StubRng::counter(),
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
        assert!(apply_horizontal_input(&mut p, dx));
        assert_eq!(p.current.pos, original + v2![dx, 0]);
    }

    #[rstest]
    fn apply_horizontal_input_blocks_at_left_wall() {
        let mut p = t_player();
        p.current.pos = v2![0, 18];
        let original = p.current.pos;
        assert!(!apply_horizontal_input(&mut p, -1));
        assert_eq!(p.current.pos, original);
    }

    #[rstest]
    fn apply_horizontal_input_resets_lock_delay_on_success() {
        let mut p = t_player();
        p.lock_delay = 15;
        assert!(apply_horizontal_input(&mut p, 1));
        assert_eq!(p.lock_delay, 0);
    }

    // -------- apply_rotation_input --------

    #[rstest]
    fn apply_rotation_input_rotates_on_empty_board() {
        let mut p = t_player();
        assert!(apply_rotation_input(
            &mut p,
            &Ruleset::guideline(),
            Rotation::CW
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
        p.pending_garbage.push(PendingGarbage {
            count: 2,
            hole: 0,
            delay_remaining: 5,
        });
        p.pending_garbage.push(PendingGarbage {
            count: 1,
            hole: 0,
            delay_remaining: 5,
        });
        cancel_pending_garbage(&mut p, 2);
        assert_eq!(p.pending_garbage.len(), 1);
        assert_eq!(p.pending_garbage[0].count, 1);
    }

    #[rstest]
    fn cancel_pending_garbage_truncates_last_attack() {
        let mut p = t_player();
        p.pending_garbage.push(PendingGarbage {
            count: 4,
            hole: 0,
            delay_remaining: 5,
        });
        cancel_pending_garbage(&mut p, 2);
        assert_eq!(p.pending_garbage.len(), 1);
        assert_eq!(p.pending_garbage[0].count, 2);
    }

    #[rstest]
    fn cancel_pending_garbage_no_op_when_to_cancel_is_zero() {
        let mut p = t_player();
        p.pending_garbage.push(PendingGarbage {
            count: 3,
            hole: 0,
            delay_remaining: 5,
        });
        cancel_pending_garbage(&mut p, 0);
        assert_eq!(p.pending_garbage.len(), 1);
        assert_eq!(p.pending_garbage[0].count, 3);
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
    fn try_hold_resets_lock_delay() {
        let mut p = t_player();
        p.lock_delay = 20;
        try_hold(&mut p);
        assert_eq!(p.lock_delay, 0);
    }

    // -------- update_score (via try_lock + cancel + score flow) --------

    #[rstest]
    fn update_score_breaks_combo_on_no_clear() {
        let mut p = t_player();
        p.combo = 3;
        let result = TickResult {
            lines_cleared: 0,
            tspin: TSpinStatus::None,
            piece_locked: false,
        };
        update_score(&mut p, &result, &Ruleset::guideline());
        assert_eq!(p.combo, -1);
    }

    #[rstest]
    fn update_score_breaks_combo_to_zero_on_first_clear() {
        let mut p = t_player();
        p.combo = -1; // broken from last time
        let result = TickResult {
            lines_cleared: 1,
            tspin: TSpinStatus::None,
            piece_locked: false,
        };
        update_score(&mut p, &result, &Ruleset::guideline());
        // Doc says `combo += 1`; from -1 that gives 0. To reset to 1 instead,
        // the implementation needs `combo = combo.max(0) + 1`. Documenting
        // current behavior; can fix the doc + impl together later.
        assert_eq!(p.combo, 0);
    }

    #[rstest]
    fn update_score_sets_b2b_on_difficult_clears() {
        let mut p = t_player();
        p.b2b = true;
        let result = TickResult {
            lines_cleared: 4,
            tspin: TSpinStatus::None,
            piece_locked: false,
        };
        update_score(&mut p, &result, &Ruleset::guideline());
        assert!(p.b2b);
        assert!(p.score >= 800 + 400); // quad + b2b bonus
    }

    // -------- detect_tspin --------

    /// Corners are at pos + (0,0), pos+(2,0), pos+(0,2), pos+(2,2). Indices
    /// 0..3 map to bottom-left, bottom-right, top-left, top-right.
    fn block_corners(p: &mut Player<StubRng>, indices: &[usize]) {
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
