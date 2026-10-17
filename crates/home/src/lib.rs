use mecha_wayland::prelude::*;

/// Spawns the home surface: a black layer-shell window on the Bottom layer
/// anchored to all four edges with no explicit size, so the compositor fills
/// the output. It sits under every other surface.
///
/// Expects the host to have installed layout, window and (for presentation)
/// Wayland with `ZwlrLayerShellV1` bound.
pub struct HomeModule;

impl Module for HomeModule {
    fn install(self, app: &mut App) {
        let role = Role::Layer(LayerRole {
            layer: Layer::Bottom,
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            exclusive_zone: 0,
            namespace: "mechanix-home".into(),
            keyboard_interactivity: KeyboardInteractivity::None,
        });

        let root = app.root();
        app.spawn_with(
            root,
            window()
                .title("mechanix-home")
                .layout(LayoutStyle::default())
                .clear(Color::BLACK),
            (role,),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_spawns_one_window() {
        let mut app = App::new();
        app.add_module(LayoutModule).add_module(WindowModule);
        // PresentationModule registers Role; it needs a live compositor.
        app.register_component::<Role>();
        app.add_module(HomeModule);
        app.tick();

        let ids: Vec<_> = app.resource::<Windows>().iter().collect();
        assert_eq!(ids.len(), 1);
        let role = app.component::<Role>(ids[0]).expect("window has a Role");
        match role {
            Role::Layer(l) => assert_eq!(l.layer, Layer::Bottom),
            _ => panic!("expected a layer role"),
        }
    }
}
