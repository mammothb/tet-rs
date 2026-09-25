use tet_domain::Ruleset;

use crate::PlayerSnapshot;

pub struct Frame<'a> {
    pub ruleset: &'a Ruleset,
    pub views: Vec<PlayerView<'a>>,
}

pub struct PlayerView<'a> {
    pub snapshot: &'a PlayerSnapshot,
    pub label: &'a str,
    /// Where on screen this board sits. Renderer interprets.
    pub layout: Layout,
}

pub struct Layout {
    pub origin: (i32, i32), // top-left in screen pixels
    pub cell_px: i32,       // cell size in pixels
    pub show_queue: bool,
    pub show_hold: bool,
    pub show_stats: bool,
}

pub trait Renderer {
    fn render(&mut self, frame: &Frame<'_>);
}
