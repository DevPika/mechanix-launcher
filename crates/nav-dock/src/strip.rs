//! The swipe strip: an empty touch target at the bottom edge while the dock
//! is hidden.

use mecha_wayland::prelude::*;

use crate::config::NavDockConfig;
use crate::windows::bottom_centre_role;

pub(crate) const NAMESPACE: &str = "mechanix-nav-strip";

/// Spawns the strip window: `strip_size`, bottom-centre, on Overlay. It
/// draws nothing but its black clear, which matches home underneath.
pub(crate) fn spawn_strip(app: &mut App, config: &NavDockConfig) -> NodeId {
    let (width, height) = config.strip_size;
    let root = app.root();
    app.spawn_with(
        root,
        window()
            .title(NAMESPACE)
            .layout(LayoutStyle::default().size(px(width), px(height)))
            .clear(Color::BLACK),
        (bottom_centre_role(NAMESPACE),),
    )
    .id()
}
