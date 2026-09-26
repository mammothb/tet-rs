use std::cmp::Reverse;
use std::collections::VecDeque;

use crate::{Cell, Vec2};

#[derive(Clone)]
pub struct Board {
    grid: VecDeque<Vec<Cell>>,
    num_cols: u8,
    num_rows: u8,
}

impl Board {
    /// Construct an empty board of the given dimensions. All cells are `Empty`.
    #[must_use]
    pub fn new(num_cols: u8, num_rows: u8) -> Self {
        Self {
            grid: std::iter::repeat_with(|| vec![Cell::Empty; num_cols as usize])
                .take(num_rows as usize)
                .collect(),
            num_cols,
            num_rows,
        }
    }

    /// Iterator over all rows, bottom (y=0) to top (y=num_rows-1).
    /// Each item is a slice of `num_cols` cells.
    pub fn rows(&self) -> impl Iterator<Item = &[Cell]> + '_ {
        self.grid.iter().map(Vec::as_slice)
    }

    #[must_use]
    pub fn get(&self, p: Vec2) -> Cell {
        let (Ok(x), Ok(y)) = (usize::try_from(p.x), usize::try_from(p.y)) else {
            return Cell::Empty;
        };
        if x >= self.num_cols as usize || y >= self.num_rows as usize {
            return Cell::Empty;
        }
        self.grid[y][x]
    }

    /// # Panics
    ///
    /// Coordinate `p` is out of bounds.
    pub fn set(&mut self, p: Vec2, c: Cell) {
        let (Ok(x), Ok(y)) = (usize::try_from(p.x), usize::try_from(p.y)) else {
            panic!("coordinate out of bounds");
        };
        assert!(
            x < self.num_cols as usize && y < self.num_rows as usize,
            "coordinate out of bounds"
        );
        self.grid[y][x] = c;
    }

    /// True if any cell is out of bounds OR occupied by a non-Empty cell.
    /// Walls are part of collision — the caller doesn't pre-clamp.
    #[must_use]
    pub fn collides(&self, cells: &[Vec2]) -> bool {
        cells.iter().any(|&p| {
            let (Ok(x), Ok(y)) = (usize::try_from(p.x), usize::try_from(p.y)) else {
                return true;
            };
            if x >= self.num_cols as usize || y >= self.num_rows as usize {
                return true;
            }
            self.grid[y][x] != Cell::Empty
        })
    }

    /// Yields each row index whose cells are all non-Empty.
    #[allow(clippy::cast_possible_truncation)]
    pub fn full_rows(&self) -> impl Iterator<Item = u8> + '_ {
        self.grid
            .iter()
            .enumerate()
            .filter(|(_, row)| row.iter().all(|&c| c != Cell::Empty))
            .map(|(y, _)| y as u8)
    }

    /// Removes the given rows and inserts empty rows at the top so total
    /// height is preserved. `rows` may be in any order - we sort descending
    /// internally so indices stay valid mid-clear.
    pub fn clear_rows(&mut self, rows: &[u8]) {
        if rows.is_empty() {
            return;
        }
        let mut sorted_rows = rows.to_vec();
        sorted_rows.sort_unstable_by_key(|&y| Reverse(y));
        sorted_rows.dedup();

        for &y in &sorted_rows {
            self.grid.remove(y as usize);
        }
        for _ in 0..sorted_rows.len() {
            self.grid
                .push_back(vec![Cell::Empty; self.num_cols as usize]);
        }
    }

    /// Inserts `count` rows of garbage at the bottom: existing rows shift
    /// up by `count` (top `count` fall off), bottom `count` rows become
    /// `Cell::Garbage` except at column `hole`.
    ///
    /// # Panics
    ///
    /// hole is out of bounds
    pub fn insert_garbage(&mut self, count: u8, hole: u8) {
        if count == 0 {
            return;
        }

        let num_cols = self.num_cols as usize;
        let num_rows = self.num_rows as usize;
        let hole = hole as usize;
        assert!(hole < num_cols, "hole column {hole} >= width {num_cols}");

        let garbage_row = {
            let mut row = vec![Cell::Garbage; num_cols];
            row[hole] = Cell::Empty;
            row
        };

        let count = count as usize;
        let cap_count = count.min(num_rows);

        let new_len = self.grid.len().saturating_sub(cap_count);
        self.grid.truncate(new_len);

        for _ in 0..cap_count {
            self.grid.push_front(garbage_row.clone());
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::MinoType;

    use rstest::rstest;

    fn empty_board(cols: u8, rows: u8) -> Board {
        Board {
            grid: VecDeque::from(vec![vec![Cell::Empty; cols as usize]; rows as usize]),
            num_cols: cols,
            num_rows: rows,
        }
    }

    // -------- get: OOB policy --------

    #[rstest]
    #[case::x_neg(Vec2::new(-1, 0))]
    #[case::y_neg(Vec2::new(0, -1))]
    #[case::x_too_large(Vec2::new(10, 0))]
    #[case::y_too_large(Vec2::new(0, 20))]
    #[case::both_oob(Vec2::new(-1, 20))]
    fn get_out_of_bounds_returns_empty(#[case] p: Vec2) {
        let b = empty_board(10, 20);
        assert_eq!(b.get(p), Cell::Empty);
    }

    // -------- collides --------

    #[rstest]
    fn collides_empty_cells_no_collision() {
        let b = empty_board(10, 20);
        assert!(!b.collides(&[Vec2::new(3, 5), Vec2::new(4, 5)]));
    }

    #[rstest]
    fn collides_occupied_cell() {
        let mut b = empty_board(10, 20);
        b.set(Vec2::new(3, 5), Cell::Block(MinoType::I));
        assert!(b.collides(&[Vec2::new(3, 5)]));
    }

    #[rstest]
    #[case::x_neg(Vec2::new(-1, 0))]
    #[case::y_neg(Vec2::new(0, -1))]
    #[case::x_too_large(Vec2::new(10, 0))]
    #[case::y_too_large(Vec2::new(0, 20))]
    fn collides_oob_is_wall_collision(#[case] p: Vec2) {
        let b = empty_board(10, 20);
        assert!(b.collides(&[p]));
    }

    #[rstest]
    fn collides_any_one_oob_means_collision() {
        let b = empty_board(10, 20);
        assert!(b.collides(&[Vec2::new(3, 5), Vec2::new(-1, 5)]));
    }

    // -------- full_rows --------

    #[rstest]
    fn full_rows_empty_board_yields_nothing() {
        let b = empty_board(10, 20);
        assert_eq!(b.full_rows().count(), 0);
    }

    #[rstest]
    fn full_rows_partial_row_yields_nothing() {
        let mut b = empty_board(10, 20);
        b.set(Vec2::new(3, 5), Cell::Block(MinoType::T));
        assert_eq!(b.full_rows().count(), 0);
    }

    #[rstest]
    fn full_rows_garbage_only_counts_as_full() {
        let mut b = empty_board(10, 20);
        for x in 0..10 {
            b.set(Vec2::new(x, 5), Cell::Garbage);
        }
        assert_eq!(b.full_rows().collect::<Vec<_>>(), vec![5]);
    }

    #[rstest]
    fn full_rows_mixed_block_and_garbage_counts_as_full() {
        let mut b = empty_board(10, 20);
        for x in 0..5 {
            b.set(Vec2::new(x, 5), Cell::Block(MinoType::T));
        }
        for x in 5..10 {
            b.set(Vec2::new(x, 5), Cell::Garbage);
        }
        assert_eq!(b.full_rows().collect::<Vec<_>>(), vec![5]);
    }

    #[rstest]
    fn full_rows_multiple_yields_all() {
        let mut b = empty_board(10, 20);
        for x in 0..10 {
            b.set(Vec2::new(x, 3), Cell::Block(MinoType::T));
            b.set(Vec2::new(x, 7), Cell::Block(MinoType::I));
        }
        let mut rows: Vec<u8> = b.full_rows().collect();
        rows.sort_unstable();
        assert_eq!(rows, vec![3, 7]);
    }

    // -------- clear_rows --------

    #[rstest]
    fn clear_rows_unsorted_input_still_clears() {
        let mut b = empty_board(10, 20);
        for x in 0..10 {
            b.set(Vec2::new(x, 3), Cell::Block(MinoType::T));
            b.set(Vec2::new(x, 7), Cell::Block(MinoType::I));
        }
        b.clear_rows(&[7, 3]); // arbitrary order
        assert_eq!(b.full_rows().count(), 0);
        assert_eq!(b.grid.len(), 20);
    }

    #[rstest]
    fn clear_rows_duplicates_dont_panic_or_grow_board() {
        let mut b = empty_board(10, 20);
        for x in 0..10 {
            b.set(Vec2::new(x, 5), Cell::Block(MinoType::T));
        }
        b.clear_rows(&[5, 5, 5]);
        assert_eq!(b.full_rows().count(), 0);
        assert_eq!(b.grid.len(), 20);
    }

    #[rstest]
    fn clear_rows_empty_is_noop() {
        let mut b = empty_board(10, 20);
        b.set(Vec2::new(3, 5), Cell::Block(MinoType::T));
        b.clear_rows(&[]);
        assert_eq!(b.grid.len(), 20);
        assert_eq!(b.get(Vec2::new(3, 5)), Cell::Block(MinoType::T));
    }

    // -------- insert_garbage --------

    #[rstest]
    fn insert_garbage_zero_is_noop() {
        let mut b = empty_board(10, 20);
        b.set(Vec2::new(3, 5), Cell::Block(MinoType::T));
        b.insert_garbage(0, 0);
        assert_eq!(b.grid.len(), 20);
        assert_eq!(b.get(Vec2::new(3, 5)), Cell::Block(MinoType::T));
    }

    #[rstest]
    fn insert_garbage_shifts_existing_rows_up() {
        let mut b = empty_board(10, 20);
        b.set(Vec2::new(3, 5), Cell::Block(MinoType::T));
        b.insert_garbage(2, 4);
        // T was at y=5; shifted up by 2, now at y=7
        assert_eq!(b.get(Vec2::new(3, 7)), Cell::Block(MinoType::T));
        assert_eq!(b.grid.len(), 20);
    }

    #[rstest]
    fn insert_garbage_hole_at_correct_column() {
        let mut b = empty_board(10, 20);
        b.insert_garbage(1, 4);
        for x in 0..10 {
            let expected = if x == 4 { Cell::Empty } else { Cell::Garbage };
            assert_eq!(b.get(Vec2::new(x, 0)), expected, "x={x}");
        }
    }

    #[rstest]
    fn insert_garbage_caps_at_height_when_count_exceeds() {
        let mut b = empty_board(10, 5);
        b.set(Vec2::new(3, 4), Cell::Block(MinoType::T));
        b.insert_garbage(10, 0);
        // T at y=4 was at the top, gets pushed off; row is now garbage
        assert_eq!(b.get(Vec2::new(3, 4)), Cell::Garbage);
        // Board height must NOT grow past num_rows
        assert_eq!(b.grid.len(), 5);
    }

    #[rstest]
    #[should_panic(expected = "hole column")]
    fn insert_garbage_hole_out_of_bounds_panics() {
        let mut b = empty_board(10, 20);
        b.insert_garbage(1, 10);
    }
}
