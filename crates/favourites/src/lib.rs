use mecha_wayland::prelude::*;
use nav_dock_api::{Section, placeholder_page};

const ICON: &[u8] = include_bytes!("../assets/heart-fill.svg");

pub struct Favourites;

impl Section for Favourites {
    fn id(&self) -> &'static str {
        "favourites"
    }

    fn label(&self) -> &'static str {
        "Favourites"
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
        assert!(Bitmap::from_svg(Favourites.icon_svg(), 24).is_ok());
        assert_eq!(Favourites.id(), "favourites");
        assert_eq!(Favourites.label(), "Favourites");
    }
}
