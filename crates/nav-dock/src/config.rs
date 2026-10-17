use std::time::Duration;

/// Every gesture and layout number the dock uses.
#[derive(Debug, Clone)]
pub struct NavDockConfig {
    /// Swipe strip at the bottom edge, (width, height) in px.
    pub strip_size: (f32, f32),
    /// Upward travel from the strip that reveals the dock.
    pub reveal_px: f32,
    /// Upward travel from the touch-down point before the hold releases.
    pub hold_px: f32,
    /// Time since touch-down before the hold releases.
    pub hold_time: Duration,
    /// Upward speed at release above which the gesture goes home.
    pub flick_px_per_ms: f32,
    pub velocity_window: Duration,
    pub velocity_stale: Duration,
    /// Downward drag over the overlay that closes it.
    pub close_px: f32,
    pub axis_lock_px: f32,
    pub page_momentum_ms: f32,
    pub page_min_fraction: f32,
    /// Expanded dock, (width, height) in px.
    pub dock_size: (f32, f32),
    pub dock_bottom_gap: f32,
    /// Minimized bar, (width, height) in px.
    pub bar_size: (f32, f32),
    pub bar_bottom_gap: f32,
    pub anim: Duration,
    /// Section id hovered when the dock is revealed.
    pub default_section: &'static str,
}

impl Default for NavDockConfig {
    fn default() -> Self {
        Self {
            strip_size: (120.0, 20.0),
            reveal_px: 8.0,
            hold_px: 56.0,
            hold_time: Duration::from_millis(220),
            flick_px_per_ms: 0.9,
            velocity_window: Duration::from_millis(80),
            velocity_stale: Duration::from_millis(80),
            close_px: 90.0,
            axis_lock_px: 10.0,
            page_momentum_ms: 180.0,
            page_min_fraction: 0.16,
            dock_size: (464.0, 73.0),
            dock_bottom_gap: 26.0,
            bar_size: (160.0, 12.0),
            bar_bottom_gap: 8.0,
            anim: Duration::from_millis(280),
            default_section: "recents",
        }
    }
}
