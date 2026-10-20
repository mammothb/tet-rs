use std::path::PathBuf;

use tet_infrastructure::GridStyle;

pub fn parse_grid_style(s: &str) -> Result<GridStyle, String> {
    match s {
        "none" => Ok(GridStyle::None),
        "horizontal" => Ok(GridStyle::Horizontal),
        "vertical" => Ok(GridStyle::Vertical),
        "full" => Ok(GridStyle::Full),
        _ => Err(format!("unknown grid style: {s}")),
    }
}

pub fn resolve_skin_path(name: Option<&String>) -> Option<PathBuf> {
    name.map(|n| PathBuf::from(format!("assets/skins/{n}.png")))
}
