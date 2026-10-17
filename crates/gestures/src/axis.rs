#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Locks a drag to one axis once either delta passes the threshold.
pub struct AxisLock {
    threshold: f32,
    locked: Option<Axis>,
}

impl AxisLock {
    pub fn new(threshold: f32) -> Self {
        Self { threshold, locked: None }
    }

    pub fn update(&mut self, dx: f32, dy: f32) -> Option<Axis> {
        if self.locked.is_none() && (dx.abs() > self.threshold || dy.abs() > self.threshold) {
            self.locked = Some(if dx.abs() >= dy.abs() {
                Axis::Horizontal
            } else {
                Axis::Vertical
            });
        }
        self.locked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_locks_after_threshold() {
        let mut lock = AxisLock::new(10.0);
        assert_eq!(lock.update(4.0, 3.0), None);
        assert_eq!(lock.update(12.0, 3.0), Some(Axis::Horizontal));
        assert_eq!(lock.update(0.0, 50.0), Some(Axis::Horizontal));
    }
}
