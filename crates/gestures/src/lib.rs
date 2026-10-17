//! Pure gesture math: velocity tracking, axis lock and page snapping.

mod axis;
mod snap;
mod velocity;

pub use axis::{Axis, AxisLock};
pub use snap::snap_page;
pub use velocity::VelocityTracker;
