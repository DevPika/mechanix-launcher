use mecha_wayland::prelude::*;

use crate::font::Fonts;
use crate::input_method::{self, ApplyKeyboardVisibility, KeyboardVisibilityExt};
use crate::keyboard::KeyboardBuilder;
use crate::layout::Keymap;

/// The debug visibility bar's height — also the collapsed surface height.
///
/// The layer surface is created at this height (keyboard hidden at launch,
/// no text input focused) and grows to the keymap's `keyboard.height` once a
/// text input activates or the bar is tapped.
const BAR_HEIGHT: f32 = 8.0;

/// Spawns a `zwlr_layer_surface` anchored to the bottom edge of the output.
/// When no fixed width is configured it is stretched full-width (left + right
/// anchors); otherwise it is anchored to the bottom only at the configured
/// width. An exclusive zone of `-1` lets the surface overlap other content
/// without reserving compositor space. `KeyboardInteractivity::None` keeps the
/// OSK from stealing keyboard focus from the focused app; pointer taps on the
/// layer surface are still delivered.
pub(crate) struct Shell;

pub(crate) struct ShellBuilder {
    pub(crate) root: NodeId,
    pub(crate) fonts: Fonts,
    pub(crate) keymap: Keymap,
}

impl Build for ShellBuilder {
    type Widget = Shell;
}

impl Widget for Shell {
    type Builder = ShellBuilder;
    fn build(b: ShellBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let style = b.keymap.keyboard;
        let kb_height = style.height;
        let (anchor, width) = match style.width {
            Some(w) => (Anchor::BOTTOM, px(w)),
            _ => (Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT, auto()),
        };
        let role = Role::Layer(LayerRole {
            layer: Layer::Top,
            anchor,
            exclusive_zone: -1,
            namespace: "mechanix-keyboard".into(),
            keyboard_interactivity: KeyboardInteractivity::None,
        });

        // Window column. The window's requested height starts at the bar
        // height (8px) so the layer surface is created collapsed — the
        // keyboard starts hidden (no text input is focused at launch) and
        // only the bar is on screen. The visibility handler re-sends
        // `set_size` to grow the surface to the keyboard height on show
        // and shrink it back to the bar height on hide, so no empty black
        // rectangle remains behind the bar when hidden.
        //
        // The surface height is otherwise compositor-driven (WSI → layout):
        // `request_layer_size` asks the compositor for a new size, it
        // configures, and `settle` reports `Resized` so the window's
        // `LayoutStyle` is settled by the compositor answer.
        let win = s.spawn_with(
            b.root,
            window().layout(
                LayoutStyle::default()
                    .column()
                    .justify(Justify::End)
                    .size(width, px(BAR_HEIGHT)),
            ),
            (role,),
        );
        let win_id = win.id();
        // Width to (re-)request on resize: `0` only stretches full-width
        // when left+right anchors are set; a fixed-width keyboard must
        // echo its configured width, since it is anchored to `BOTTOM` only.
        let req_w = match style.width {
            Some(w) => w as u32,
            None => 0,
        };

        let keyboard = s.spawn(
            win,
            KeyboardBuilder {
                fonts: b.fonts,
                keymap: b.keymap,
            },
        );

        // ── always-visible toggle bar ───────────────────────────────────
        //
        // A short, full-width bar below the keyboard that force-toggles
        // surface visibility through the *same* `KeyboardVisibilityExt`
        // mechanism IM2 `activate`/`deactivate` use. Always visible (never
        // `Display::Hidden`) so it remains a handle to bring the keyboard
        // back when no text input is focused.
        let bar = s.spawn(
            win,
            div()
                .style(
                    LayoutStyle::default()
                        .row()
                        .width(percent(100.0))
                        .height(px(BAR_HEIGHT))
                        .shrink(0.0),
                )
                // A visible fill so the bar reads as a tappable handle.
                .background(s.color(ColorRole::PrimaryContainer)),
        );
        s.on::<Clicked>(bar, |ctx, _| {
            ctx.toggle_keyboard_visibility();
        });

        // Apply visibility on every change — activate/deactivate or the bar.
        // Two things happen, through the *same* `KeyboardVisibilityExt` flip:
        //   1. The keyboard subtree is `Display::Hidden`/`Flex` so it takes
        //      no space when hidden.
        //   2. The layer surface itself is resized via `request_layer_size`
        //      so the on-screen surface collapses to just the bar (no empty
        //      black rectangle remains) when hidden, and grows back to the
        //      keyboard + bar height when shown. The compositor answers
        //      with a configure; the framework's `settle` reports
        //      `Resized` and lays out at the new size.
        s.on::<ApplyKeyboardVisibility>(me, move |ctx, _| {
            let visible = ctx.keyboard_visible();
            ctx.at(keyboard).unwrap().set_display(if visible {
                Display::Flex
            } else {
                Display::Hidden
            });
            // `0` width stretches full-width only when left+right anchors
            // are set; a fixed-width keyboard echoes its configured width.
            let h = if visible { kb_height } else { BAR_HEIGHT };
            let (mut surfaces, mut wl) = ctx.fetch::<(ResMut<Surfaces>, ResMut<Wayland>)>();
            surfaces.request_layer_size(win_id, req_w, h as u32, &mut wl);
        });

        // Apply the initial visibility (starts hidden). The change handler
        // above only fires on subsequent flips, so seed the keyboard's
        // `Display` here to match the resource's initial value.
        if !s.resource::<input_method::KeyboardVisibility>().visible
            && let Some(mut kstyle) = s.component_mut::<LayoutStyle>(keyboard)
        {
            kstyle.display = Display::Hidden;
        }

        Shell
    }
}
