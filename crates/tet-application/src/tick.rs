use tet_domain::{Cell, MinoType, Orientation, Rng, Rotation, Ruleset, Vec2, v2};

use crate::{LOCK_DELAY_FRAMES, Phase, Piece, player::Player};

#[derive(PartialEq, Eq)]
pub enum TSpinStatus {
    None,
    Mini,
    Full,
}

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
            cancel_pending_garbage(player, result.lines_cleared);
            update_score(player, &result, ruleset);
            spawn_next_piece(player);
            if check_topout(player) {
                player.phase = Phase::GameOver;
            }
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
    let mut ghost = player.current.clone(); // Piece is Copy
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
