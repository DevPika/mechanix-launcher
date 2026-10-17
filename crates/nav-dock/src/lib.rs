//! The navigation dock: gesture config and the pure state machine behind it.

mod config;
mod state;

pub use config::NavDockConfig;
pub use state::{DockEvent, Mode, NavState, OverlayView, Surface, View};
