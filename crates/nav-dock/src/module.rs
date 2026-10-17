//! `NavDockModule`: the sections, their icons, the `NavDock` resource and
//! the first reconcile, which puts up the strip.

use mecha_wayland::prelude::*;
use nav_dock_api::Section;

use crate::config::NavDockConfig;
use crate::state::NavState;
use crate::windows::{NavDock, WindowSet, on_overlay_resized, reconcile};

/// Atlas master size of a dock icon, in px.
const ICON_PX: u32 = 24;

/// The navigation dock. Sections appear in the order they are added.
///
/// Expects the host to have installed layout, paint, window, animation and
/// the theme, inserted the [`Atlas`] (and whatever the sections' pages
/// need, such as a `FontBook`), and for presentation, Wayland with
/// `ZwlrLayerShellV1` bound.
pub struct NavDockModule {
    config: NavDockConfig,
    sections: Vec<Box<dyn Section>>,
}

impl NavDockModule {
    pub fn new(config: NavDockConfig) -> Self {
        Self {
            config,
            sections: Vec::new(),
        }
    }

    pub fn section(mut self, s: impl Section) -> Self {
        self.sections.push(Box::new(s));
        self
    }
}

impl Module for NavDockModule {
    fn install(self, app: &mut App) {
        let icons = {
            let mut atlas = app.resource_mut::<Atlas>();
            self.sections
                .iter()
                .map(|s| {
                    let bitmap = Bitmap::from_svg(s.icon_svg(), ICON_PX)
                        .unwrap_or_else(|e| panic!("icon of section {:?}: {e:?}", s.id()));
                    atlas
                        .insert(Class::Icon, &bitmap)
                        .unwrap_or_else(|e| panic!("icon of section {:?}: {e:?}", s.id()))
                })
                .collect()
        };
        let ids: Vec<&str> = self.sections.iter().map(|s| s.id()).collect();
        // The real screen is only known once a window reports `Resized`.
        let (width, height) = self.config.fallback_screen;
        let state = NavState::new(self.config, &ids, Size::new(width, height));
        app.insert_resource(NavDock {
            state,
            sections: self.sections,
            icons,
            windows: WindowSet::default(),
        });
        app.system(on_overlay_resized);
        reconcile(app);
    }
}
