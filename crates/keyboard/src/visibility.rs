//! The keyboard surface's shown/hidden state.
//!
//! Driven by input-method `activate`/`deactivate` (applied at `done`) and
//! force-toggled by the debug visibility bar; both write through
//! [`KeyboardVisibilityExt`]. A [`KeyboardVisibilityChanged`] signal is fanned
//! out as an [`ApplyKeyboardVisibility`] event by a system, which the keyboard
//! surface widget subscribes to.

use mecha_wayland::prelude::*;

/// The keyboard surface's shown/hidden state.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyboardVisibility {
    pub visible: bool,
}
impl Resource for KeyboardVisibility {}

/// `Signal`: the visibility flag changed — fan out [`ApplyKeyboardVisibility`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyboardVisibilityChanged;
impl Signal for KeyboardVisibilityChanged {}

/// `Event`: re-apply the current visibility to the surface. Broadcast to the
/// root and every descendant; the keyboard surface widget handles it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ApplyKeyboardVisibility;
impl Event for ApplyKeyboardVisibility {}

/// The single visibility setter: the input-method `done` handler and the debug
/// bar both go through here. Implemented for `App` (systems) and `Context`
/// (widget handlers) so the same call works at every site.
pub trait KeyboardVisibilityExt {
    /// The current visibility flag.
    fn keyboard_visible(&self) -> bool;
    /// Set the flag and signal the change (no-op if already `visible`).
    fn set_keyboard_visibility(&mut self, visible: bool);
    /// Flip the current visibility.
    fn toggle_keyboard_visibility(&mut self) {
        let next = !self.keyboard_visible();
        self.set_keyboard_visibility(next);
    }
}

impl KeyboardVisibilityExt for App {
    fn keyboard_visible(&self) -> bool {
        self.resource::<KeyboardVisibility>().visible
    }
    fn set_keyboard_visibility(&mut self, visible: bool) {
        if self.resource::<KeyboardVisibility>().visible == visible {
            return;
        }
        self.resource_mut::<KeyboardVisibility>().visible = visible;
        self.signal(KeyboardVisibilityChanged);
        tracing::info!(visible, "keyboard visibility");
    }
}

impl<W: Widget> KeyboardVisibilityExt for Context<'_, W> {
    fn keyboard_visible(&self) -> bool {
        self.resource::<KeyboardVisibility>().visible
    }
    fn set_keyboard_visibility(&mut self, visible: bool) {
        if self.resource::<KeyboardVisibility>().visible == visible {
            return;
        }
        self.resource_mut::<KeyboardVisibility>().visible = visible;
        self.signal(KeyboardVisibilityChanged);
        tracing::info!(visible, "keyboard visibility");
    }
}

/// On [`KeyboardVisibilityChanged`], broadcast [`ApplyKeyboardVisibility`] to
/// the root and every descendant so the surface widget re-applies `Display`.
fn on_keyboard_visibility_changed(app: &mut App, _: &KeyboardVisibilityChanged) {
    let mut targets = vec![app.root()];
    targets.extend(app.tree().descendants(app.root()));
    app.emit(ApplyKeyboardVisibility, targets);
}

/// Insert the visibility resource (starting hidden) and the signal→event bridge.
pub(crate) fn install(app: &mut App) {
    app.insert_resource(KeyboardVisibility { visible: false });
    app.system(on_keyboard_visibility_changed);
}
