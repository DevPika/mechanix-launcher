use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Estimates pointer velocity (px/ms) with a least-squares slope over recent samples.
pub struct VelocityTracker {
    window: Duration,
    stale: Duration,
    /// Shortest time between the oldest and newest sample in the window
    /// that gives a velocity; a tighter burst gives zero.
    min_span: Duration,
    samples: VecDeque<(Instant, f32, f32)>,
}

impl VelocityTracker {
    pub fn new(window: Duration, stale: Duration, min_span: Duration) -> Self {
        Self {
            window,
            stale,
            min_span,
            samples: VecDeque::new(),
        }
    }

    pub fn push(&mut self, t: Instant, x: f32, y: f32) {
        self.samples.push_back((t, x, y));
        while let Some(&(oldest, ..)) = self.samples.front() {
            if t.saturating_duration_since(oldest) > self.window {
                self.samples.pop_front();
            } else {
                break;
            }
        }
    }

    /// Velocity in px/ms, or `(0.0, 0.0)` if stale, underdetermined or
    /// spanning less than `min_span`.
    pub fn velocity(&self, now: Instant) -> (f32, f32) {
        let Some(&(newest, ..)) = self.samples.back() else {
            return (0.0, 0.0);
        };
        if now.saturating_duration_since(newest) > self.stale {
            return (0.0, 0.0);
        }
        let recent: Vec<_> = self
            .samples
            .iter()
            .filter(|(t, ..)| now.saturating_duration_since(*t) <= self.window)
            .collect();
        if recent.len() < 2 {
            return (0.0, 0.0);
        }
        let t0 = recent[0].0;
        if recent[recent.len() - 1].0.saturating_duration_since(t0) < self.min_span {
            return (0.0, 0.0);
        }
        let n = recent.len() as f64;
        let ts: Vec<f64> = recent
            .iter()
            .map(|(t, ..)| t.duration_since(t0).as_secs_f64() * 1000.0)
            .collect();
        let mean_t = ts.iter().sum::<f64>() / n;
        let var: f64 = ts.iter().map(|t| (t - mean_t).powi(2)).sum();
        if var <= f64::EPSILON {
            return (0.0, 0.0);
        }
        let slope = |pick: fn(&(Instant, f32, f32)) -> f32| -> f32 {
            let mean = recent.iter().map(|s| pick(s) as f64).sum::<f64>() / n;
            let cov: f64 = recent
                .iter()
                .zip(&ts)
                .map(|(s, t)| (t - mean_t) * (pick(s) as f64 - mean))
                .sum();
            (cov / var) as f32
        };
        (slope(|s| s.1), slope(|s| s.2))
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn tracker() -> VelocityTracker {
        VelocityTracker::new(
            Duration::from_millis(100),
            Duration::from_millis(80),
            Duration::from_millis(8),
        )
    }

    #[test]
    fn steady_motion_gives_constant_velocity() {
        let t0 = Instant::now();
        let mut v = tracker();
        for i in 0..10u64 {
            v.push(
                t0 + Duration::from_millis(i * 8),
                5.0,
                100.0 - 8.0 * i as f32,
            );
        }
        let now = t0 + Duration::from_millis(72);
        let (vx, vy) = v.velocity(now);
        assert!(vx.abs() < 0.05, "vx = {vx}");
        assert!((vy + 1.0).abs() < 0.05, "vy = {vy}");
    }

    #[test]
    fn bursty_timestamps_do_not_spike() {
        let t0 = Instant::now();
        let mut v = tracker();
        let mut y = 100.0;
        for i in 0..5u64 {
            v.push(t0 + Duration::from_millis(i * 8), 0.0, y);
            y -= 8.0;
        }
        // Two samples 0.1 ms apart, 10 px apart.
        let burst = t0 + Duration::from_millis(40);
        v.push(burst, 0.0, y);
        y -= 10.0;
        v.push(burst + Duration::from_micros(100), 0.0, y);
        for i in 1..5u64 {
            y -= 8.0;
            v.push(burst + Duration::from_millis(i * 8), 0.0, y);
        }
        let (_, vy) = v.velocity(burst + Duration::from_millis(32));
        assert!(vy.abs() < 1.5, "vy = {vy}");
    }

    #[test]
    fn burst_after_stall_gives_zero() {
        let t0 = Instant::now();
        let mut v = tracker();
        v.push(t0, 0.0, 100.0);
        v.push(t0 + Duration::from_micros(250), 0.0, 85.0);
        v.push(t0 + Duration::from_micros(500), 0.0, 70.0);
        assert_eq!(v.velocity(t0 + Duration::from_micros(500)), (0.0, 0.0));
    }

    #[test]
    fn stale_velocity_is_zero() {
        let t0 = Instant::now();
        let mut v = tracker();
        for i in 0..5u64 {
            v.push(t0 + Duration::from_millis(i * 8), 0.0, -8.0 * i as f32);
        }
        let last = t0 + Duration::from_millis(32);
        assert_eq!(v.velocity(last + Duration::from_millis(81)), (0.0, 0.0));
    }

    #[test]
    fn single_sample_and_reset_give_zero() {
        let t0 = Instant::now();
        let mut v = tracker();
        v.push(t0, 0.0, 0.0);
        assert_eq!(v.velocity(t0), (0.0, 0.0));
        v.push(t0 + Duration::from_millis(8), 0.0, -8.0);
        v.reset();
        assert_eq!(v.velocity(t0 + Duration::from_millis(8)), (0.0, 0.0));
    }
}
