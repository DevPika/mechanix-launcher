use mecha_wayland::prelude::*;
use nav_dock_api::{Section, placeholder_page};

const ICON: &[u8] = include_bytes!("../assets/magnifying-glass-fill.svg");

pub struct Search;

impl Section for Search {
    fn id(&self) -> &'static str {
        "search"
    }

    fn label(&self) -> &'static str {
        "Search"
    }

    fn icon_svg(&self) -> &'static [u8] {
        ICON
    }

    fn spawn_page(&self, app: &mut App, parent: NodeId) -> NodeId {
        placeholder_page(app, parent, self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_rasterises() {
        assert!(Bitmap::from_svg(Search.icon_svg(), 24).is_ok());
        assert_eq!(Search.id(), "search");
        assert_eq!(Search.label(), "Search");
    }
}
