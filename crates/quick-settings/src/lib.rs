use mecha_wayland::prelude::*;
use nav_dock_api::{Section, placeholder_page};

const ICON: &[u8] = include_bytes!("../assets/sliders-fill.svg");

pub struct QuickSettings;

impl Section for QuickSettings {
    fn id(&self) -> &'static str {
        "quick-settings"
    }

    fn label(&self) -> &'static str {
        "Quick Settings"
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
        assert!(Bitmap::from_svg(QuickSettings.icon_svg(), 24).is_ok());
        assert_eq!(QuickSettings.id(), "quick-settings");
        assert_eq!(QuickSettings.label(), "Quick Settings");
    }
}
