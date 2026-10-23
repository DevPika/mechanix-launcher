//! Pure navigation core: config, gesture samples, triggers and motions.
//! No Wayland, windows or nodes; only `Point` and `Size` from mecha-wayland.

pub mod config;
pub mod gesture;
pub mod motion;
pub mod trigger;

pub use config::NavConfig;
pub use gesture::{Detect, Gesture, Phase};
pub use motion::{FADE, Fade, Final, INSTANT, Motion, Snap, Visual};
pub use trigger::{Direction, ExternalEvent, Span, Swipe, Timing, Trigger, swipe};

/// Names a touch area (an edge strip) that gestures start in.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AreaId(pub &'static str);

pub const BOTTOM_EDGE: AreaId = AreaId("bottom-edge");
pub const TOP_EDGE: AreaId = AreaId("top-edge");
