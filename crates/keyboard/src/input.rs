//! Routes a tapped key's [`KeyAction`]: view switches and modifier latches go
//! to the view/latch controllers; every other action is a keystroke emitted
//! through the input method (preferred) or the virtual keyboard (fallback),
//! consuming any latched modifier.

use mecha_wayland::prelude::*;
use xkbcommon::xkb::Keysym;

use crate::latch::{LatchedState, consume_latch, toggle_latch};
use crate::layout::{KeyAction, Modifier};
use crate::protocol::input_method::{self, InputMethodState};
use crate::protocol::virtual_keyboard::{self, VirtualKeyboardState};
use crate::view::switch_view;
use crate::widget::keyboard::Keyboard;

/// Route a tapped key's action. View switches and modifier latches mutate the
/// Keyboard widget state; every other action is a keystroke the virtual
/// keyboard emits (consuming any latched modifier).
pub(crate) fn dispatch(ctx: &mut Context<'_, Keyboard>, action: &KeyAction) {
    match action {
        KeyAction::SetView(name) => {
            switch_view(ctx, name);
        }
        KeyAction::ToggleView { lock, unlock } => {
            let target = if ctx.me().current_view_name() == lock {
                unlock
            } else {
                lock
            };
            switch_view(ctx, target);
        }
        KeyAction::LatchModifier(m) => {
            toggle_latch(ctx, *m);
        }
        KeyAction::EmitKeysym(ks) => {
            let latched_mask = latched_mask(ctx);
            // Printable keysyms commit via IM2; control keysyms fall back to
            // the virtual-keyboard-v1 keysym transport. A latched modifier
            // forces the keycode path — IM2 commits plain text and would drop
            // the modifier.
            if latched_mask == 0
                && let Some(text) = virtual_keyboard::keysym_text(*ks)
                && im_commit_text(ctx, &text)
            {
                tracing::info!(input = %text, "input method");
            } else {
                vk_emit_keysym(ctx, *ks, latched_mask);
            }
            consume_latch(ctx);
        }
        KeyAction::EmitText(text) => {
            let latched_mask = latched_mask(ctx);
            if latched_mask == 0 && im_commit_text(ctx, text) {
                tracing::info!("Input method: {text}");
            } else {
                for ch in text.chars() {
                    vk_emit_keysym(ctx, Keysym::from_char(ch), latched_mask);
                }
            }
            consume_latch(ctx);
        }
        KeyAction::Unhandled(name) => {
            tracing::info!(action = %name, "tapped key with no wired action");
        }
    }
}

/// The combined modifier mask of every currently-latched modifier, to OR into
/// a keystroke. Reads from the [`LatchedState`] resource.
fn latched_mask(ctx: &Context<'_, Keyboard>) -> u32 {
    let latched = ctx.resource::<LatchedState>();
    let masks = &ctx.resource::<VirtualKeyboardState>().mod_masks;
    Modifier::ALL
        .into_iter()
        .filter(|&m| latched.get(m))
        .filter_map(|m| masks.get(&m).copied())
        .fold(0, |acc, mask| acc | mask)
}

/// Commit `text` through the input method when it's bound and active.
fn im_commit_text(ctx: &mut Context<'_, Keyboard>, text: &str) -> bool {
    let (st, mut wl) = ctx.fetch::<(Res<InputMethodState>, ResMut<Wayland>)>();
    input_method::commit_text(st, &mut wl, text)
}

/// Send one keysym as a keycode down+up through the virtual keyboard.
fn vk_emit_keysym(ctx: &mut Context<'_, Keyboard>, ks: Keysym, latched_mask: u32) {
    let (vk, mut wl) = ctx.fetch::<(Res<VirtualKeyboardState>, ResMut<Wayland>)>();
    virtual_keyboard::emit_keysym(vk, &mut wl, ks, latched_mask)
}
