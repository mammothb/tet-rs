#![allow(clippy::cast_precision_loss)]

use std::path::Path;

use macroquad::math::Rect;
use macroquad::texture::{FilterMode, Texture2D, load_texture};
use tet_domain::{Cell, MinoType};

pub struct SpriteSheet {
    pub texture: Texture2D,
    tile_size: f32,
}

impl SpriteSheet {
    pub async fn load(path: &Path) -> Option<Self> {
        let texture = load_texture(path.to_str()?).await.ok()?;
        texture.set_filter(FilterMode::Nearest);
        let tile_size = texture.height();
        Some(Self { texture, tile_size })
    }

    #[must_use]
    pub fn source_rect(&self, cell: Cell) -> Rect {
        let col = cell_to_column(cell) as f32;
        Rect::new(col * self.tile_size, 0.0, self.tile_size, self.tile_size)
    }
}

fn cell_to_column(cell: Cell) -> usize {
    match cell {
        Cell::Block(kind) => kind_to_column(kind),
        Cell::Garbage => 1,
        Cell::Empty => unreachable!("empty cells are never drawn"),
    }
}

fn kind_to_column(kind: MinoType) -> usize {
    match kind {
        MinoType::J => 7,
        MinoType::L => 3,
        MinoType::S => 5,
        MinoType::Z => 2,
        MinoType::T => 8,
        MinoType::I => 6,
        MinoType::O => 4,
    }
}
