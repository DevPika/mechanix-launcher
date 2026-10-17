use mecha_wayland::prelude::*;
use nav_dock_api::{Section, placeholder_page};

const ICON: &[u8] = include_bytes!("../assets/stack-fill.svg");

pub struct Recents;

impl Section for Recents {
    fn id(&self) -> &'static str {
        "recents"
    }

    fn label(&self) -> &'static str {
        "Recent Apps"
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
        assert!(Bitmap::from_svg(Recents.icon_svg(), 24).is_ok());
        assert_eq!(Recents.id(), "recents");
        assert_eq!(Recents.label(), "Recent Apps");
    }
}
