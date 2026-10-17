//! The navigation dock: gesture config, the pure state machine behind it,
//! and the windows that show it.

mod config;
mod dock;
mod input;
mod module;
mod overlay;
mod state;
mod strip;
mod windows;

pub use config::NavDockConfig;
pub use module::NavDockModule;
pub use state::{DockEvent, Exit, Mode, NavState, OverlayView, Release, Surface, View};
