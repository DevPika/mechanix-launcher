use std::time::Duration;

use gestures::Axis;
use mecha_wayland::prelude::{Point, Size};

use super::AreaId;

/// A touch in progress, in the local coordinates of its area.
pub struct Gesture {
    pub area: AreaId,
    pub area_size: Size,
    pub start: Point,
    pub pos: Point,
    pub elapsed: Duration,
    /// px/ms, screen-down positive.
    pub velocity: (f32, f32),
    pub axis: Option<Axis>,
}

impl Gesture {
    pub fn delta(&self) -> (f32, f32) {
        (self.pos.x - self.start.x, self.pos.y - self.start.y)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Moving,
    Released,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Detect {
    Progress(f32),
    Commit,
    Cancel,
    Ignore,
}
