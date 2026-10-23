use std::time::Duration;

use super::config::NavConfig;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Final {
    Open,
    Closed,
}

impl Final {
    pub fn value(self) -> f32 {
        match self {
            Final::Open => 1.0,
            Final::Closed => 0.0,
        }
    }
}

/// How a screen is drawn at a given presence.
pub struct Visual {
    /// Alpha of the black cover over the content.
    pub cover: f32,
}

/// Together: `(input, current, final, dt) -> new current`, split by input kind.
pub trait Motion: 'static {
    fn follow(&self, current: f32, _toward: Final, _progress: f32) -> f32 {
        current
    }
    fn settle(&self, current: f32, target: Final, dt: Duration, cfg: &NavConfig) -> f32;
    fn visual(&self, _presence: f32) -> Visual {
        Visual { cover: 0.0 }
    }
}

/// Jumps straight to the target.
#[derive(Clone, Copy)]
pub struct Snap;
pub const INSTANT: Snap = Snap;

impl Motion for Snap {
    fn settle(&self, _current: f32, target: Final, _dt: Duration, _cfg: &NavConfig) -> f32 {
        target.value()
    }
}

/// Fades linearly over `cfg.fade`.
#[derive(Clone, Copy)]
pub struct Fade;
pub const FADE: Fade = Fade;

impl Motion for Fade {
    fn settle(&self, current: f32, target: Final, dt: Duration, cfg: &NavConfig) -> f32 {
        let step = dt.as_secs_f32() / cfg.fade.as_secs_f32();
        let goal = target.value();
        if goal > current {
            (current + step).min(goal)
        } else {
            (current - step).max(goal)
        }
        .clamp(0.0, 1.0)
    }

    fn visual(&self, presence: f32) -> Visual {
        Visual {
            cover: 1.0 - presence,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: fn(u64) -> Duration = Duration::from_millis;

    #[test]
    fn instant_settles_immediately() {
        assert_eq!(
            INSTANT.settle(0.3, Final::Open, MS(0), &NavConfig::default()),
            1.0
        );
    }

    #[test]
    fn fade_advances_by_dt() {
        let cfg = NavConfig::default();
        let v = FADE.settle(0.0, Final::Open, MS(80), &cfg);
        assert_eq!(v, 0.5);
        assert_eq!(FADE.visual(v).cover, 0.5);
    }

    #[test]
    fn fade_clamps_large_dt() {
        let cfg = NavConfig::default();
        assert_eq!(
            FADE.settle(0.9, Final::Open, Duration::from_secs(10), &cfg),
            1.0
        );
        assert_eq!(
            FADE.settle(0.1, Final::Closed, Duration::from_secs(10), &cfg),
            0.0
        );
    }
}
