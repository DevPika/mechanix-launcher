//! The keyboard's layer surface: a `zwlr_layer_surface` anchored to the bottom
//! edge that hosts the keyboard tree.
//!
//! The surface is unmapped while the keyboard is hidden, so it takes no
//! space and catches no touches; visibility flips — from input-method
//! `activate`/`deactivate`, a corner swipe or the hide key — map and unmap it.

use mecha_wayland::prelude::*;

use crate::layout::Keymap;
use crate::layout::tokens::Fonts;
use crate::visibility::{ApplyKeyboardVisibility, KeyboardVisibilityExt};
use crate::widget::keyboard::KeyboardBuilder;
use crate::zone::ExclusiveZone;

/// The keyboard's window and height, for the controls that change its
/// surface (visibility, exclusive zone).
#[derive(Debug, Clone, Copy)]
pub(crate) struct KeyboardSurface {
    pub(crate) window: NodeId,
    pub(crate) height: f32,
}
impl Resource for KeyboardSurface {}

/// Spawns a `zwlr_layer_surface` anchored to the bottom edge of the output.
///
/// When no fixed width is configured it is stretched full-width (left + right
/// anchors); otherwise it is anchored to the bottom only at the configured
/// width. The exclusive zone follows [`ExclusiveZone`]: `-1` (overlap other
/// content without reserving space) while off. `KeyboardInteractivity::None`
/// keeps the OSK from stealing keyboard focus from the focused app; pointer
/// taps on the layer surface are still delivered.
pub(crate) struct KeyboardUI {
    pub(crate) surface: KeyboardSurface,
}

pub(crate) struct KeyboardUIBuilder {
    pub(crate) root: NodeId,
    pub(crate) fonts: Fonts,
    pub(crate) keymap: Keymap,
}

impl Build for KeyboardUIBuilder {
    type Widget = KeyboardUI;
}

impl Widget for KeyboardUI {
    type Builder = KeyboardUIBuilder;
    fn build(b: KeyboardUIBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let style = b.keymap.keyboard;
        let kb_height = style.height;
        let (anchor, width) = match style.width {
            Some(w) => (Anchor::BOTTOM, px(w)),
            _ => (Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT, auto()),
        };
        let role = Role::Layer(LayerRole {
            layer: Layer::Top,
            anchor,
            exclusive_zone: s.resource::<ExclusiveZone>().zone(kb_height),
            namespace: "keyboard".into(),
            keyboard_interactivity: KeyboardInteractivity::None,
        });

        let win = s.spawn_with(
            b.root,
            window().layout(
                LayoutStyle::default()
                    .column()
                    .justify(Justify::End)
                    .size(width, px(kb_height)),
            ),
            (role,),
        );
        let window = win.id();

        s.spawn(
            win,
            KeyboardBuilder {
                fonts: b.fonts,
                keymap: b.keymap,
            },
        );

        // Map the surface while shown, unmap it while hidden.
        s.on::<ApplyKeyboardVisibility>(me, move |ctx, _| {
            let visible = ctx.keyboard_visible();
            let (mut surfaces, mut wl) = ctx.fetch::<(ResMut<Surfaces>, ResMut<Wayland>)>();
            surfaces.set_mapped(window, visible, &mut wl);
        });

        KeyboardUI {
            surface: KeyboardSurface {
                window,
                height: kb_height,
            },
        }
    }
}
