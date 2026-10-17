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
    /// Samples spanning less time than this give zero velocity.
    pub velocity_min_span: Duration,
    /// Downward drag over the overlay that closes it.
    pub close_px: f32,
    pub axis_lock_px: f32,
    pub page_momentum_ms: f32,
    pub page_min_fraction: f32,
    /// Share of a page drag past the first or last page that the row follows.
    pub page_edge_resistance: f32,
    /// Expanded dock, (width, height) in px.
    pub dock_size: (f32, f32),
    pub dock_bottom_gap: f32,
    /// Minimized bar, (width, height) in px.
    pub bar_size: (f32, f32),
    pub bar_bottom_gap: f32,
    pub anim: Duration,
    /// Section id hovered when the dock is revealed.
    pub default_section: &'static str,
    /// Screen size, (width, height) in px, until a window's `Resized`
    /// reports a real one.
    pub fallback_screen: (f32, f32),
    /// Upward finger travel from the strip at which the revealed dock is
    /// fully in place.
    pub reveal_full_px: f32,
    /// Floor on the speed of a throw animation, in px/s.
    pub min_throw_speed: f32,
    /// Slope of the throw easing curve at t = 0 (3.0 for `EaseOutCubic`).
    pub throw_easing_slope: f32,
    /// Share of finger travel past the dock's rest height that it follows.
    pub dock_rubber: f32,
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
            velocity_min_span: Duration::from_millis(8),
            close_px: 90.0,
            axis_lock_px: 10.0,
            page_momentum_ms: 180.0,
            page_min_fraction: 0.16,
            page_edge_resistance: 0.3,
            dock_size: (464.0, 73.0),
            dock_bottom_gap: 26.0,
            bar_size: (160.0, 12.0),
            bar_bottom_gap: 8.0,
            anim: Duration::from_millis(280),
            default_section: "recents",
            fallback_screen: (612.0, 1360.0),
            reveal_full_px: 300.0,
            min_throw_speed: 1500.0,
            throw_easing_slope: 3.0,
            dock_rubber: 0.3,
        }
    }
}
