use mecha_wayland::prelude::*;
use nav_dock_api::{Section, placeholder_page};

const ICON: &[u8] = include_bytes!("../assets/bell-fill.svg");

pub struct Notifications;

impl Section for Notifications {
    fn id(&self) -> &'static str {
        "notifications"
    }

    fn label(&self) -> &'static str {
        "Notifications"
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
        assert!(Bitmap::from_svg(Notifications.icon_svg(), 24).is_ok());
        assert_eq!(Notifications.id(), "notifications");
        assert_eq!(Notifications.label(), "Notifications");
    }
}
