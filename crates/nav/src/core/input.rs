use std::time::{Duration, Instant};

use mecha_wayland::prelude::{Point, Size};

use super::AreaId;
use super::trigger::ExternalEvent;

/// Everything the stack reacts to. Positions are local to the area the
/// contact went down on.
pub enum Input {
    Down {
        contact: u32,
        area: AreaId,
        area_size: Size,
        pos: Point,
        t: Instant,
    },
    Move {
        contact: u32,
        pos: Point,
        t: Instant,
    },
    Up {
        contact: u32,
        t: Instant,
    },
    Cancel,
    Tick {
        dt: Duration,
    },
    External(ExternalEvent),
}
