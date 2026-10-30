//! [`KeyboardModule`] — install the on-screen keyboard into a host app.

use mecha_wayland::prelude::*;

use crate::latch::LatchedState;
use crate::layout;
use crate::layout::icons::Icons;
use crate::layout::tokens::Fonts;
use crate::protocol::input_method::InputMethodModule;
use crate::protocol::virtual_keyboard::VirtualKeyboardModule;
use crate::view::on_text_input_focused;
use crate::widget::surface::KeyboardUIBuilder;

/// Installs the on-screen keyboard: binds `zwp_virtual_keyboard_v1` and
/// `zwp_input_method_v2`, loads the keymap, fonts and icons, and spawns the
/// keyboard's own bottom-anchored layer surface. Unlike the screens, the
/// keyboard owns its window (it resizes the surface as it shows and hides), so
/// it is not placed by the nav.
///
/// Expects the host to have installed the [`WaylandModule`] (with [`WlSeat`]
/// and [`ZwlrLayerShellV1`] bound), the `WindowModule`, `InteractivityModule`
/// and a theme, and to have inserted the [`Atlas`]. The theme is the host's:
/// the keymap's `mode` is ignored.
///
/// Add this after any other `Layer::Top` windows (such as the `NavModule`'s) so
/// the keyboard surface stacks above them.
pub struct KeyboardModule;

impl Module for KeyboardModule {
    fn install(self, app: &mut App) {
        // Virtual-keyboard-v1 and input-method-v2 modules — bind their globals
        // conditionally from `Globals` and handle their protocol events. These
        // must install after `WaylandModule` and after the seat is bound.
        app.add_module(VirtualKeyboardModule)
            .add_module(InputMethodModule);
        app.system(on_text_input_focused);
        spawn_ui(app);
    }
}

/// The keyboard without its protocols: the keymap, latch and visibility
/// state, fonts and icons, and its surface. What [`KeyboardModule`] installs after binding
/// them; public so a host can build the keyboard headlessly, as the
/// launcher's atlas budget test does.
#[doc(hidden)]
pub fn spawn_ui(app: &mut App) {
    // Load the keymap from layout.yaml (env var, local file, or bundled fallback).
    let keymap = layout::load_keymap();

    app.insert_resource(LatchedState::default());
    // Keyboard surface visibility — starts hidden; IM2 `activate` and the
    // debug visibility bar flip it through `KeyboardVisibilityExt`.
    crate::visibility::install(app);

    let fonts = Fonts::load(&mut app.resource_mut::<Atlas>());
    let icons = Icons::load(
        &mut app.resource_mut::<Atlas>(),
        keymap.keyboard.font_size as u32,
    );
    app.insert_resource(icons);

    let root = app.root();
    app.spawn(
        root,
        KeyboardUIBuilder {
            root,
            fonts,
            keymap,
        },
    );
}
