//! Wayland-facing half of nav: spawns the windows and container nodes and
//! pushes the stack's state onto them.

mod apply;
mod module;

pub use module::{NavModule, WindowSpec, nav_window};
