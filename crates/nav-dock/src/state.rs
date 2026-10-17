//! The dock's pure state machine: touch events in, a description of what
//! should be on screen out. No windows, no Wayland.

use std::time::Instant;

use gestures::{Axis, AxisLock, VelocityTracker, snap_page};
use mecha_wayland::prelude::{Point, Size};

use crate::config::NavDockConfig;

/// The dock window a touch went down on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Strip,
    Bar,
    Dock,
    Overlay,
}

/// A touch event, positions in screen space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DockEvent {
    Down {
        contact: u32,
        surface: Surface,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Hidden,
    Dragging { hover: usize },
    Idle { active: usize },
    Minimized { active: usize },
}

/// What should be on screen right now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub mode: Mode,
    /// The strip window should exist.
    pub strip: bool,
    pub overlay: Option<OverlayView>,
    /// Surface owning the live touch: must not be removed.
    pub held_surface: Option<Surface>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayView {
    pub page: usize,
    /// Live horizontal drag.
    pub dx: f32,
    /// Live close drag (>= 0).
    pub dy: f32,
}

pub struct NavState {
    config: NavDockConfig,
    section_count: usize,
    default_index: usize,
    screen: Size,
    mode: Mode,
    gesture: Option<Gesture>,
    velocity: VelocityTracker,
}

impl NavState {
    pub fn new(config: NavDockConfig, section_ids: &[&str], screen: Size) -> Self {
        let default_index = section_ids
            .iter()
            .position(|id| *id == config.default_section)
            .unwrap_or(0);
        let velocity = VelocityTracker::new(config.velocity_window, config.velocity_stale);
        Self {
            config,
            section_count: section_ids.len(),
            default_index,
            screen,
            mode: Mode::Hidden,
            gesture: None,
            velocity,
        }
    }

    pub fn set_screen(&mut self, screen: Size) {
        self.screen = screen;
    }

    /// Centre of icon `i`: `n` icons spread evenly across the dock width,
    /// the dock centred on screen.
    pub fn icon_center_x(&self, i: usize) -> f32 {
        let width = self.config.dock_size.0;
        let n = self.section_count.max(1) as f32;
        self.screen.width / 2.0 - width / 2.0 + width * (i as f32 + 0.5) / n
    }

    pub fn handle(&mut self, ev: DockEvent) {
        match ev {
            DockEvent::Down {
                contact,
                surface,
                pos,
                t,
            } => self.down(contact, surface, pos, t),
            DockEvent::Move { contact, pos, t } => self.motion(contact, pos, t),
            DockEvent::Up { contact, t } => self.up(contact, t),
            DockEvent::Cancel => self.cancel(),
        }
    }

    pub fn view(&self) -> View {
        let page = match self.mode {
            Mode::Hidden => None,
            Mode::Dragging { hover } => Some(hover),
            Mode::Idle { active } | Mode::Minimized { active } => Some(active),
        };
        let (dx, dy) = self
            .gesture
            .as_ref()
            .map_or((0.0, 0.0), Gesture::overlay_offset);
        View {
            mode: self.mode,
            strip: self.mode == Mode::Hidden,
            overlay: page.map(|page| OverlayView { page, dx, dy }),
            held_surface: self.gesture.as_ref().map(|g| g.surface),
        }
    }

    fn down(&mut self, contact: u32, surface: Surface, pos: Point, t: Instant) {
        if self.gesture.is_some() {
            return;
        }
        let kind = match (self.mode, surface) {
            (Mode::Hidden, Surface::Strip) => Kind::StripPending,
            (Mode::Minimized { .. }, Surface::Bar) => Kind::BarPending,
            (Mode::Idle { .. }, Surface::Dock) => Kind::DockTap,
            (Mode::Minimized { .. } | Mode::Idle { .. }, Surface::Overlay) => Kind::Overlay {
                lock: AxisLock::new(self.config.axis_lock_px),
                axis: None,
            },
            _ => Kind::Inert,
        };
        self.velocity.reset();
        self.velocity.push(t, pos.x, pos.y);
        self.gesture = Some(Gesture {
            contact,
            surface,
            start: pos,
            start_t: t,
            last: pos,
            kind,
        });
    }

    fn motion(&mut self, contact: u32, pos: Point, t: Instant) {
        let Some(mut g) = self.gesture.take_if(|g| g.contact == contact) else {
            return;
        };
        self.velocity.push(t, pos.x, pos.y);
        g.last = pos;
        let travel = pos - g.start;
        match &mut g.kind {
            Kind::StripPending => {
                if -travel.y >= self.config.reveal_px {
                    self.mode = Mode::Dragging {
                        hover: self.default_index,
                    };
                    g.kind = Kind::Drag { held: true };
                    self.drag(&mut g, t);
                }
            }
            Kind::Drag { .. } => self.drag(&mut g, t),
            Kind::BarPending => {
                if travel.x.hypot(travel.y) > self.config.reveal_px {
                    g.kind = Kind::Drag { held: false };
                    self.drag(&mut g, t);
                }
            }
            Kind::Overlay { lock, axis } => *axis = lock.update(travel.x, travel.y),
            Kind::DockTap | Kind::Inert => {}
        }
        self.gesture = Some(g);
    }

    /// Hover update for a drag: held on the default section until the
    /// finger has gone far enough up and long enough, then nearest icon.
    fn drag(&mut self, g: &mut Gesture, t: Instant) {
        let Kind::Drag { held } = &mut g.kind else {
            return;
        };
        if *held
            && g.start.y - g.last.y >= self.config.hold_px
            && t.saturating_duration_since(g.start_t) >= self.config.hold_time
        {
            *held = false;
        }
        if !*held {
            self.mode = Mode::Dragging {
                hover: self.nearest_icon(g.last.x),
            };
        }
    }

    fn up(&mut self, contact: u32, t: Instant) {
        let Some(g) = self.gesture.take_if(|g| g.contact == contact) else {
            return;
        };
        let (vx, vy) = self.velocity.velocity(t);
        let (dx, dy) = g.overlay_offset();
        match (g.kind, self.mode) {
            (Kind::Drag { .. }, Mode::Dragging { hover }) => {
                self.mode = if vy < -self.config.flick_px_per_ms {
                    Mode::Hidden
                } else {
                    Mode::Minimized { active: hover }
                };
            }
            (Kind::BarPending, Mode::Minimized { active }) => self.mode = Mode::Idle { active },
            (Kind::DockTap, _) => {
                self.mode = Mode::Minimized {
                    active: self.nearest_icon(g.last.x),
                };
            }
            (Kind::Overlay { axis, .. }, Mode::Idle { active } | Mode::Minimized { active }) => {
                match axis {
                    None => self.mode = Mode::Minimized { active },
                    Some(Axis::Horizontal) => {
                        let page = snap_page(
                            active,
                            dx,
                            vx,
                            self.screen.width,
                            self.section_count,
                            self.config.page_momentum_ms,
                            self.config.page_min_fraction,
                        );
                        self.mode = Mode::Minimized { active: page };
                    }
                    Some(Axis::Vertical) => {
                        if dy > self.config.close_px {
                            self.mode = Mode::Hidden;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn cancel(&mut self) {
        if let Some(Gesture {
            kind: Kind::Drag { .. },
            ..
        }) = self.gesture.take()
        {
            self.mode = Mode::Hidden;
        }
    }

    fn nearest_icon(&self, x: f32) -> usize {
        (0..self.section_count)
            .min_by(|&a, &b| {
                let da = (x - self.icon_center_x(a)).abs();
                let db = (x - self.icon_center_x(b)).abs();
                da.total_cmp(&db)
            })
            .unwrap_or(0)
    }
}

/// The one live touch the dock is following.
struct Gesture {
    contact: u32,
    surface: Surface,
    start: Point,
    start_t: Instant,
    last: Point,
    kind: Kind,
}

impl Gesture {
    /// Live (dx, dy) of an overlay drag, zero for anything else.
    fn overlay_offset(&self) -> (f32, f32) {
        match self.kind {
            Kind::Overlay {
                axis: Some(Axis::Horizontal),
                ..
            } => (self.last.x - self.start.x, 0.0),
            Kind::Overlay {
                axis: Some(Axis::Vertical),
                ..
            } => (0.0, (self.last.y - self.start.y).max(0.0)),
            _ => (0.0, 0.0),
        }
    }
}

enum Kind {
    /// Hidden, finger on the strip, not yet `reveal_px` up.
    StripPending,
    /// Dragging; `held` while the hold keeps the default hover.
    Drag { held: bool },
    /// Minimized, finger on the bar, not yet moved.
    BarPending,
    /// Idle, finger on the dock.
    DockTap,
    /// Over an open overlay. No axis yet means a tap.
    Overlay { lock: AxisLock, axis: Option<Axis> },
    /// A touch with no gesture; only held so its window survives.
    Inert,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const IDS: [&str; 5] = [
        "favourites",
        "search",
        "recents",
        "notifications",
        "quick-settings",
    ];
    const BAR_Y: f32 = 1346.0;

    struct Rig {
        s: NavState,
        t0: Instant,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                s: NavState::new(NavDockConfig::default(), &IDS, Size::new(612.0, 1360.0)),
                t0: Instant::now(),
            }
        }

        fn at(&self, ms: u64) -> Instant {
            self.t0 + Duration::from_millis(ms)
        }

        fn down_on(&mut self, contact: u32, surface: Surface, x: f32, y: f32, ms: u64) {
            let t = self.at(ms);
            self.s.handle(DockEvent::Down {
                contact,
                surface,
                pos: Point::new(x, y),
                t,
            });
        }

        fn down(&mut self, surface: Surface, x: f32, y: f32, ms: u64) {
            self.down_on(0, surface, x, y, ms);
        }

        fn mv_on(&mut self, contact: u32, x: f32, y: f32, ms: u64) {
            let t = self.at(ms);
            self.s.handle(DockEvent::Move {
                contact,
                pos: Point::new(x, y),
                t,
            });
        }

        fn mv(&mut self, x: f32, y: f32, ms: u64) {
            self.mv_on(0, x, y, ms);
        }

        fn up(&mut self, ms: u64) {
            let t = self.at(ms);
            self.s.handle(DockEvent::Up { contact: 0, t });
        }

        fn mode(&self) -> Mode {
            self.s.view().mode
        }

        fn overlay(&self) -> OverlayView {
            self.s.view().overlay.expect("overlay should be shown")
        }

        /// Down on the strip at (306, 1350) and move 10 px up: revealed.
        fn reveal(&mut self) {
            self.down(Surface::Strip, 306.0, 1350.0, 0);
            self.mv(330.0, 1340.0, 16);
        }

        /// Reveal, then pass the hold (60 px up at 230 ms) at `x`.
        fn past_hold(&mut self, x: f32) {
            self.reveal();
            self.mv(x, 1290.0, 230);
        }

        /// Ends in `Minimized { active: i }` by t = 1200 ms.
        fn minimized(i: usize) -> Self {
            let mut r = Self::new();
            r.reveal();
            r.up(50);
            if i != 2 {
                r.down(Surface::Bar, 306.0, BAR_Y, 1000);
                let x = r.s.icon_center_x(i);
                r.mv(x, BAR_Y, 1016);
                r.up(1200);
            }
            assert_eq!(r.mode(), Mode::Minimized { active: i });
            r
        }

        /// Ends in `Idle { active: i }` by t = 2050 ms.
        fn idle(i: usize) -> Self {
            let mut r = Self::minimized(i);
            r.down(Surface::Bar, 306.0, BAR_Y, 2000);
            r.up(2050);
            assert_eq!(r.mode(), Mode::Idle { active: i });
            r
        }
    }

    #[test]
    fn hidden_shows_only_strip() {
        let r = Rig::new();
        let v = r.s.view();
        assert_eq!(v.mode, Mode::Hidden);
        assert!(v.strip);
        assert_eq!(v.overlay, None);
    }

    #[test]
    fn small_upward_move_reveals_with_default_hover() {
        let mut r = Rig::new();
        r.reveal();
        assert_eq!(r.mode(), Mode::Dragging { hover: 2 });
        assert_eq!(r.overlay().page, 2);
    }

    #[test]
    fn hold_keeps_default_until_distance_and_time() {
        let mut r = Rig::new();
        r.reveal();
        r.mv(60.0, 1290.0, 100);
        assert_eq!(r.mode(), Mode::Dragging { hover: 2 });
        r.mv(60.0, 1290.0, 230);
        assert_eq!(r.mode(), Mode::Dragging { hover: 0 });
    }

    #[test]
    fn hold_needs_distance_too() {
        let mut r = Rig::new();
        r.reveal();
        r.mv(60.0, 1320.0, 300);
        assert_eq!(r.mode(), Mode::Dragging { hover: 2 });
    }

    #[test]
    fn release_during_hold_commits_default() {
        let mut r = Rig::new();
        r.reveal();
        r.up(50);
        assert_eq!(r.mode(), Mode::Minimized { active: 2 });
        assert!(!r.s.view().strip);
        assert_eq!(r.overlay().page, 2);
    }

    #[test]
    fn tap_on_strip_does_nothing() {
        let mut r = Rig::new();
        r.down(Surface::Strip, 306.0, 1350.0, 0);
        r.up(40);
        assert_eq!(r.mode(), Mode::Hidden);
        assert!(r.s.view().strip);
    }

    #[test]
    fn flick_goes_home() {
        let mut r = Rig::new();
        r.reveal();
        let mut y = 1340.0;
        let mut t = 16;
        for _ in 0..10 {
            y -= 12.0;
            t += 8;
            r.mv(330.0, y, t);
        }
        r.up(t + 4);
        assert_eq!(r.mode(), Mode::Hidden);
        assert_eq!(r.s.view().overlay, None);
    }

    #[test]
    fn slow_release_commits_hovered() {
        let mut r = Rig::new();
        let x = r.s.icon_center_x(4);
        r.past_hold(x);
        assert_eq!(r.mode(), Mode::Dragging { hover: 4 });
        r.up(330);
        assert_eq!(r.mode(), Mode::Minimized { active: 4 });
    }

    #[test]
    fn hover_clamps_outside_screen() {
        let mut r = Rig::new();
        r.past_hold(306.0);
        r.mv(-50.0, 1290.0, 240);
        assert_eq!(r.mode(), Mode::Dragging { hover: 0 });
        r.mv(900.0, 1290.0, 250);
        assert_eq!(r.mode(), Mode::Dragging { hover: 4 });
    }

    #[test]
    fn second_contact_is_ignored() {
        let mut r = Rig::new();
        r.past_hold(306.0);
        assert_eq!(r.mode(), Mode::Dragging { hover: 2 });
        r.down_on(1, Surface::Overlay, 50.0, 600.0, 240);
        r.mv_on(1, 600.0, 600.0, 250);
        assert_eq!(r.mode(), Mode::Dragging { hover: 2 });
        assert_eq!(r.s.view().held_surface, Some(Surface::Strip));
    }

    #[test]
    fn cancel_while_dragging_from_strip_hides_everything() {
        let mut r = Rig::new();
        r.reveal();
        r.s.handle(DockEvent::Cancel);
        let v = r.s.view();
        assert_eq!(v.mode, Mode::Hidden);
        assert_eq!(v.overlay, None);
        assert_eq!(v.held_surface, None);
    }

    #[test]
    fn overlay_horizontal_swipe_switches_page() {
        let mut r = Rig::minimized(2);
        r.down(Surface::Overlay, 306.0, 600.0, 3000);
        r.mv(150.0, 605.0, 3016);
        assert_eq!(r.overlay().dx, -156.0);
        r.up(3200);
        assert_eq!(r.mode(), Mode::Minimized { active: 3 });
        assert_eq!(r.overlay().dx, 0.0);
    }

    #[test]
    fn switch_clamps_at_ends() {
        let mut r = Rig::minimized(4);
        r.down(Surface::Overlay, 500.0, 600.0, 3000);
        for k in 1..=8u64 {
            r.mv(500.0 - 50.0 * k as f32, 600.0, 3000 + 8 * k);
        }
        r.up(3068);
        assert_eq!(r.mode(), Mode::Minimized { active: 4 });

        let mut r = Rig::minimized(0);
        r.down(Surface::Overlay, 100.0, 600.0, 3000);
        for k in 1..=8u64 {
            r.mv(100.0 + 50.0 * k as f32, 600.0, 3000 + 8 * k);
        }
        r.up(3068);
        assert_eq!(r.mode(), Mode::Minimized { active: 0 });
    }

    #[test]
    fn swipe_down_closes() {
        let mut r = Rig::minimized(2);
        r.down(Surface::Overlay, 306.0, 600.0, 3000);
        r.mv(306.0, 650.0, 3016);
        assert_eq!(r.overlay().dy, 50.0);
        r.mv(306.0, 695.0, 3032);
        r.up(3200);
        let v = r.s.view();
        assert_eq!(v.mode, Mode::Hidden);
        assert!(v.strip);
        assert_eq!(v.overlay, None);
    }

    #[test]
    fn short_swipe_down_springs_back() {
        let mut r = Rig::minimized(2);
        r.down(Surface::Overlay, 306.0, 600.0, 3000);
        r.mv(306.0, 660.0, 3016);
        r.up(3200);
        assert_eq!(r.mode(), Mode::Minimized { active: 2 });
        assert_eq!(r.overlay().dy, 0.0);
    }

    #[test]
    fn bar_tap_expands_idle() {
        let mut r = Rig::minimized(1);
        r.down(Surface::Bar, 306.0, BAR_Y, 3000);
        r.up(3050);
        assert_eq!(r.mode(), Mode::Idle { active: 1 });
    }

    #[test]
    fn bar_drag_selects_without_hold() {
        let mut r = Rig::minimized(1);
        r.down(Surface::Bar, 306.0, BAR_Y, 3000);
        let x = r.s.icon_center_x(4);
        r.mv(x, BAR_Y, 3008);
        assert_eq!(r.mode(), Mode::Dragging { hover: 4 });
        r.up(3200);
        assert_eq!(r.mode(), Mode::Minimized { active: 4 });
    }

    #[test]
    fn idle_tap_icon_commits() {
        let mut r = Rig::idle(1);
        let x = r.s.icon_center_x(3);
        r.down(Surface::Dock, x, 1300.0, 3000);
        r.up(3050);
        assert_eq!(r.mode(), Mode::Minimized { active: 3 });
    }

    #[test]
    fn idle_tap_outside_collapses() {
        let mut r = Rig::idle(1);
        r.down(Surface::Overlay, 306.0, 600.0, 3000);
        r.up(3050);
        assert_eq!(r.mode(), Mode::Minimized { active: 1 });
    }

    #[test]
    fn held_surface_reported() {
        let mut r = Rig::new();
        r.reveal();
        assert_eq!(r.s.view().held_surface, Some(Surface::Strip));
        r.up(50);
        assert_eq!(r.s.view().held_surface, None);
    }
}
