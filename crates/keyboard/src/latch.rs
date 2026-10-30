//! One-shot modifier latches: arming a modifier (Ctrl, Alt) applies it to the
//! next keystroke only, then auto-clears. [`LatchedState`] is the shared
//! resource both the [`Key`](crate::widget::key::Key) widget's theme handler
//! and the latch controllers read and write.

use std::collections::HashSet;

use mecha_wayland::prelude::*;

use crate::layout::Modifier;
use crate::layout::style::KeyState;
use crate::widget::key::{KeyKind, key_roles, set_label_color};
use crate::widget::keyboard::Keyboard;

/// Shared state for the one-shot modifier latches.
#[derive(Default)]
pub(crate) struct LatchedState {
    armed: HashSet<Modifier>,
}
impl Resource for LatchedState {}

impl LatchedState {
    /// Whether `m` is currently armed.
    pub(crate) fn get(&self, m: Modifier) -> bool {
        self.armed.contains(&m)
    }

    /// Flip `m`'s latch; returns whether it is now armed.
    pub(crate) fn toggle(&mut self, m: Modifier) -> bool {
        if self.armed.remove(&m) {
            false
        } else {
            self.armed.insert(m);
            true
        }
    }
}

/// Toggle a modifier's one-shot latch and repaint every latching key across
/// all views to reflect the armed/disarmed state.
pub(crate) fn toggle_latch(ctx: &mut Context<'_, Keyboard>, m: Modifier) {
    let armed = ctx.resource_mut::<LatchedState>().toggle(m);
    repaint_latch_keys(ctx);
    tracing::info!(modifier = ?m, armed, "latch toggled");
}

/// Clear every modifier latch after a keystroke fires (one-shot).
pub(crate) fn consume_latch(ctx: &mut Context<'_, Keyboard>) {
    if !ctx.resource::<LatchedState>().armed.is_empty() {
        ctx.resource_mut::<LatchedState>().armed.clear();
        repaint_latch_keys(ctx);
    }
}

/// Repaint all latching modifier keys to reflect the current latched state.
pub(crate) fn repaint_latch_keys(ctx: &mut Context<'_, Keyboard>) {
    let radius = ctx.me().radius;

    let keys = ctx.me().latch_keys.clone();
    for (m, k) in keys {
        let state = KeyState::latched_if(ctx.resource::<LatchedState>().get(m));
        let mut key = ctx.at(k).unwrap();
        let (label, look) = (key.me().label, key.me().look);
        let (bg_role, fg_role) = key_roles(look, KeyKind::Function, state);
        let bg = key.color(bg_role);
        let fg = key.color(fg_role);

        key.set_paint(Paint::Quad(Quad::new(bg).radius(radius)));
        set_label_color(ctx, label, fg);
    }
}
