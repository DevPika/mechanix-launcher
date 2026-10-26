//! An on-screen keyboard built with themed widgets on the mecha-wayland UI
//! core. Install it into a host app with [`KeyboardModule`].
//!
//! Builds a tree of themed `Widget` nodes: each key is a `Key` widget whose
//! background, label colour, and corner radius come from `MechanixTheme` /
//! `ColorRole`, and whose `on_theme` callback re-resolves them instantly when
//! the user toggles dark/light at runtime.
//!
//! The keymap (parsed from `resources/layout.yaml` by `layout.rs`) drives which
//! keys appear and what they do. View switching (Shift_L `locking`,
//! `show_symbols`, `show_eschars`) and one-shot modifier latching (Ctrl, Alt) are
//! supported. Text keys commit through `zwp_input_method_v2` when a text input
//! is focused, falling back to `zwp_virtual_keyboard_v1` keysym transport.

#![recursion_limit = "1024"]

mod color_role;
mod font;
mod icons;
mod input_method;
mod key_style;
mod keyboard;
mod layout;
mod module;
mod shape;
mod ui;
mod spacing;
mod virtual_keyboard;

pub use module::KeyboardModule;
