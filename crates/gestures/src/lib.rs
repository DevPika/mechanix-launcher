//! Pure gesture math: velocity tracking, axis lock, page snapping and
//! throw speeds.

mod axis;
mod snap;
mod throw;
mod velocity;

pub use axis::{Axis, AxisLock};
pub use snap::snap_page;
pub use throw::throw_speed;
pub use velocity::VelocityTracker;
