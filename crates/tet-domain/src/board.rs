use std::cmp::Reverse;

use crate::{Cell, Vec2};

#[derive(Debug)]
pub struct Board {
    grid: Vec<Vec<Cell>>,
    num_cols: u8,
    num_rows: u8,
}

impl Board {
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
        for _ in 0..rows.len() {
            self.grid.push(vec![Cell::Empty; self.num_cols as usize]);
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

        let hole = hole as usize;
        let num_cols = self.num_cols as usize;
        assert!(hole < num_cols, "hole column {hole} >= width {num_cols}");

        let garbage_row = {
            let mut row = vec![Cell::Garbage; num_cols];
            row[hole] = Cell::Empty;
            row
        };

        let count = count as usize;
        let mut grid = std::mem::take(&mut self.grid);
        let keep_from = count.min(grid.len());
        let kept = grid.split_off(keep_from);

        self.grid = Vec::with_capacity(kept.len() + count);
        for _ in 0..count {
            self.grid.push(garbage_row.clone());
        }
        self.grid.extend(kept);
    }
}
