use mecha_wayland::prelude::*;

use crate::{BluetoothModule, DEFAULT_BAR_HEIGHT, UpowerModule, WifiModule, status_bar};

/// Installs the status bar's services and spawns its window: a layer-shell
/// surface anchored to the top edge that reserves [`DEFAULT_BAR_HEIGHT`].
///
/// Expects the host to have installed layout, paint, window, render, the
/// theme, D-Bus, Wayland (with `ZwlrLayerShellV1` bound) and presentation,
/// and to have inserted the [`Atlas`], `FontBook` and [`StatusIcons`](crate::StatusIcons).
pub struct StatusBarModule;

impl Module for StatusBarModule {
    fn install(self, app: &mut App) {
        app.add_module(WifiModule).add_module(BluetoothModule);

        let role = Role::Layer(LayerRole {
            layer: Layer::Top,
            anchor: Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
            exclusive_zone: DEFAULT_BAR_HEIGHT as i32,
            namespace: "mechanix-status-bar".into(),
            keyboard_interactivity: KeyboardInteractivity::None,
        });

        let root = app.root();
        let win = app.spawn_with(
            root,
            window()
                .title("mechanix-status-bar")
                .layout(LayoutStyle::default().height(px(DEFAULT_BAR_HEIGHT))),
            (role,),
        );
        app.spawn(win, status_bar());

        // The initial UPower reply emits directly to the existing status bar.
        app.add_module(UpowerModule);
    }
}
