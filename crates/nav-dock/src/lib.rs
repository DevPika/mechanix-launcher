//! The navigation dock: gesture config, the pure state machine behind it,
//! and the windows that show it.

mod config;
mod dock;
mod module;
mod overlay;
mod state;
mod strip;
mod windows;

pub use config::NavDockConfig;
pub use module::NavDockModule;
pub use state::{DockEvent, Mode, NavState, OverlayView, Surface, View};
