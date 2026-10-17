//! The dock's pure state machine: touch events in, a description of what
//! should be on screen out. No windows, no Wayland.

use std::time::Instant;

use gestures::{snap_page, Axis, AxisLock, VelocityTracker};
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
    /// Upward travel since Down (>= 0) while a drag that began on the strip
    /// is held; `None` otherwise, bar drags included.
    pub reveal_travel: Option<f32>,
    /// Upward travel since Down (>= 0) while a drag that began on the bar is
    /// held; `None` otherwise.
    pub lift: Option<f32>,
    /// The page the bar drag started on, valid while `lift` is Some.
    pub lift_page: usize,
    /// The most recent release of a gesture.
    pub release: Option<Release>,
    /// How the overlay left, set when the mode becomes Hidden and cleared by
    /// the next reveal.
    pub exit: Option<Exit>,
    /// The overlay page the exit started from, valid while `exit` is Some.
    pub last_page: usize,
}

/// A finger lift, with the velocity it left at in px/ms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Release {
    /// Increments once per Up that ends a gesture.
    pub seq: u64,
    pub vx: f32,
    pub vy: f32,
}

/// Which way the overlay left the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// A flick up, going home.
    Up,
    /// A close swipe down, or a cancelled strip drag.
    Down,
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
    release: Option<Release>,
    exit: Option<Exit>,
    last_page: usize,
}

impl NavState {
    pub fn new(config: NavDockConfig, section_ids: &[&str], screen: Size) -> Self {
        let default_index = section_ids
            .iter()
            .position(|id| *id == config.default_section)
            .unwrap_or(0);
        let velocity = VelocityTracker::new(
            config.velocity_window,
            config.velocity_stale,
            config.velocity_min_span,
        );
        Self {
            config,
            section_count: section_ids.len(),
            default_index,
            screen,
            mode: Mode::Hidden,
            gesture: None,
            velocity,
            release: None,
            exit: None,
            last_page: 0,
        }
    }

    pub fn set_screen(&mut self, screen: Size) {
        self.screen = screen;
    }

    pub fn screen(&self) -> Size {
        self.screen
    }

    pub fn config(&self) -> &NavDockConfig {
        &self.config
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
        let (lift, lift_page) = match self.gesture.as_ref() {
            Some(g) => match g.kind {
                Kind::Drag {
                    from: Mode::Idle { active } | Mode::Minimized { active },
                    ..
                } => (Some((g.start.y - g.last.y).max(0.0)), active),
                _ => (None, 0),
            },
            None => (None, 0),
        };
        View {
            mode: self.mode,
            strip: self.mode == Mode::Hidden,
            overlay: page.map(|page| OverlayView {
                page,
                dx: self.resist(page, dx),
                dy,
            }),
            held_surface: self.gesture.as_ref().map(|g| g.surface),
            reveal_travel: self.gesture.as_ref().and_then(|g| {
                let from_strip = matches!(
                    g.kind,
                    Kind::StripPending
                        | Kind::Drag {
                            from: Mode::Hidden,
                            ..
                        }
                );
                from_strip.then(|| (g.start.y - g.last.y).max(0.0))
            }),
            lift,
            lift_page,
            release: self.release,
            exit: self.exit,
            last_page: self.last_page,
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
                    self.exit = None;
                    g.kind = Kind::Drag {
                        held: true,
                        from: Mode::Hidden,
                    };
                    self.drag(&mut g, t);
                }
            }
            Kind::Drag { .. } => self.drag(&mut g, t),
            Kind::BarPending => {
                if travel.x.hypot(travel.y) > self.config.reveal_px {
                    g.kind = Kind::Drag {
                        held: false,
                        from: self.mode,
                    };
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
        let Kind::Drag { held, .. } = &mut g.kind else {
            return;
        };
        if *held
            && g.start.y - g.last.y >= self.config.hold_px
            && t.saturating_duration_since(g.start_t) >= self.config.hold_time
        {
            *held = false;
        }
        if !*held {
            let hover = match self.mode {
                Mode::Dragging { hover } => self.sticky_icon(hover, g.last.x),
                _ => self.nearest_icon(g.last.x),
            };
            self.mode = Mode::Dragging { hover };
        }
    }

    fn up(&mut self, contact: u32, t: Instant) {
        let Some(g) = self.gesture.take_if(|g| g.contact == contact) else {
            return;
        };
        let (vx, vy) = self.velocity.velocity(t);
        let (dx, dy) = g.overlay_offset();
        if matches!(
            g.kind,
            Kind::Drag { .. } | Kind::BarPending | Kind::DockTap | Kind::Overlay { .. }
        ) {
            let seq = self.release.map_or(0, |r| r.seq) + 1;
            self.release = Some(Release { seq, vx, vy });
        }
        match (g.kind, self.mode) {
            (Kind::Drag { from, .. }, Mode::Dragging { hover }) => {
                // A swipe that started on home never flicks home.
                let flick = from != Mode::Hidden && vy < -self.config.flick_px_per_ms;
                self.mode = if flick {
                    self.exit = Some(Exit::Up);
                    self.last_page = hover;
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
                            self.exit = Some(Exit::Down);
                            self.last_page = active;
                            self.mode = Mode::Hidden;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Drops the live touch; a drag goes back to the mode it started from.
    fn cancel(&mut self) {
        if let Some(Gesture {
            kind: Kind::Drag { from, .. },
            ..
        }) = self.gesture.take()
        {
            if let (Mode::Hidden, Mode::Dragging { hover }) = (from, self.mode) {
                self.exit = Some(Exit::Down);
                self.last_page = hover;
            }
            self.mode = from;
        }
    }

    /// A live page drag `dx` on `page`: 1:1 while another page is still
    /// under the row, scaled by `page_edge_resistance` past the first or
    /// last page.
    fn resist(&self, page: usize, dx: f32) -> f32 {
        let width = self.screen.width;
        let max = page as f32 * width;
        let min = -(self.section_count.saturating_sub(page + 1) as f32) * width;
        let k = self.config.page_edge_resistance;
        if dx > max {
            max + (dx - max) * k
        } else if dx < min {
            min + (dx - min) * k
        } else {
            dx
        }
    }

    /// Hover with hysteresis: from `current`, step toward `x` one icon at a
    /// time, each step only once `x` is past `hover_stickiness` of the way
    /// to the next centre.
    fn sticky_icon(&self, current: usize, x: f32) -> usize {
        let last = self.section_count.saturating_sub(1);
        let s = self.config.hover_stickiness;
        let mut k = current.min(last);
        loop {
            let c = self.icon_center_x(k);
            if k < last && x > c + s * (self.icon_center_x(k + 1) - c) {
                k += 1;
            } else if k > 0 && x < c + s * (self.icon_center_x(k - 1) - c) {
                k -= 1;
            } else {
                return k;
            }
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
    /// Dragging; `held` while the hold keeps the default hover. `from` is
    /// the mode the drag started in, restored on cancel.
    Drag { held: bool, from: Mode },
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

    impl Rig {
        /// From `Minimized`, a fast bar drag up at `x` and release.
        fn bar_flick(&mut self, x: f32) {
            self.down(Surface::Bar, x, BAR_Y, 1000);
            let mut y = BAR_Y;
            let mut t = 1000;
            for _ in 0..10 {
                y -= 12.0;
                t += 8;
                self.mv(x, y, t);
            }
            self.up(t + 4);
        }
    }

    #[test]
    fn flick_goes_home() {
        let mut r = Rig::minimized(2);
        r.bar_flick(306.0);
        assert_eq!(r.mode(), Mode::Hidden);
        assert_eq!(r.s.view().exit, Some(Exit::Up));
        assert_eq!(r.s.view().overlay, None);
    }

    #[test]
    fn fast_strip_release_commits_instead_of_home() {
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
        let hover = r.s.default_index;
        assert_eq!(r.mode(), Mode::Minimized { active: hover });
        assert_eq!(r.s.view().exit, None);
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
    fn hover_resists_until_75_percent() {
        let mut r = Rig::new();
        r.past_hold(306.0);
        let (c2, c3) = (r.s.icon_center_x(2), r.s.icon_center_x(3));
        r.mv(c2 + 0.7 * (c3 - c2), 1290.0, 240);
        assert_eq!(r.mode(), Mode::Dragging { hover: 2 });
        r.mv(c2 + 0.8 * (c3 - c2), 1290.0, 250);
        assert_eq!(r.mode(), Mode::Dragging { hover: 3 });
        r.mv(c3 - 0.3 * (c3 - c2), 1290.0, 260);
        assert_eq!(r.mode(), Mode::Dragging { hover: 3 });
        r.mv(c3 - 0.8 * (c3 - c2), 1290.0, 270);
        assert_eq!(r.mode(), Mode::Dragging { hover: 2 });
    }

    #[test]
    fn fast_move_across_icons_lands_correctly() {
        let mut r = Rig::new();
        r.past_hold(r.s.icon_center_x(0));
        let x = r.s.icon_center_x(4);
        r.mv(x, 1290.0, 240);
        assert_eq!(r.mode(), Mode::Dragging { hover: 4 });
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
    fn cancel_while_dragging_from_bar_restores_minimized() {
        let mut r = Rig::minimized(1);
        r.down(Surface::Bar, 306.0, BAR_Y, 3000);
        let x = r.s.icon_center_x(4);
        r.mv(x, BAR_Y, 3008);
        assert_eq!(r.mode(), Mode::Dragging { hover: 4 });
        r.s.handle(DockEvent::Cancel);
        let v = r.s.view();
        assert_eq!(v.mode, Mode::Minimized { active: 1 });
        assert_eq!(v.held_surface, None);
    }

    #[test]
    fn cancel_during_overlay_drag_snaps_back() {
        let mut r = Rig::minimized(2);
        r.down(Surface::Overlay, 306.0, 600.0, 3000);
        r.mv(150.0, 605.0, 3016);
        assert_eq!(r.overlay().dx, -156.0);
        r.s.handle(DockEvent::Cancel);
        assert_eq!(r.mode(), Mode::Minimized { active: 2 });
        assert_eq!((r.overlay().dx, r.overlay().dy), (0.0, 0.0));
        assert_eq!(r.s.view().held_surface, None);
    }

    #[test]
    fn page_drag_resists_past_ends() {
        let drag = |page: usize, finger: f32| {
            let mut r = Rig::minimized(page);
            r.down(Surface::Overlay, 306.0, 600.0, 3000);
            r.mv(306.0 + finger, 600.0, 3016);
            r.overlay().dx
        };
        let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
        assert!(close(drag(0, 200.0), 60.0), "page 0: {}", drag(0, 200.0));
        assert!(close(drag(4, -200.0), -60.0), "last: {}", drag(4, -200.0));
        assert_eq!(drag(2, 200.0), 200.0);
        assert_eq!(drag(2, -200.0), -200.0);
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
    fn reveal_travel_follows_finger() {
        let mut r = Rig::new();
        r.down(Surface::Strip, 306.0, 1350.0, 0);
        r.mv(306.0, 1200.0, 16);
        assert_eq!(r.s.view().reveal_travel, Some(150.0));
        let mut b = Rig::minimized(1);
        b.down(Surface::Bar, 306.0, BAR_Y, 3000);
        b.mv(400.0, BAR_Y, 3008);
        assert_eq!(b.s.view().reveal_travel, None);
    }

    #[test]
    fn bar_drag_reports_lift() {
        let mut r = Rig::minimized(1);
        assert_eq!(r.s.view().lift, None);
        r.down(Surface::Bar, 306.0, BAR_Y, 3000);
        r.mv(306.0, BAR_Y - 40.0, 3008);
        assert_eq!(r.s.view().lift, Some(40.0));
        r.mv(306.0, BAR_Y - 200.0, 3016);
        assert_eq!(r.s.view().lift, Some(200.0));
        assert_eq!(r.s.view().lift_page, 1);
        r.mv(306.0, BAR_Y + 20.0, 3024);
        assert_eq!(r.s.view().lift, Some(0.0));
        r.up(3030);
        assert_eq!(r.s.view().lift, None);
    }

    #[test]
    fn strip_drag_has_no_lift() {
        let mut r = Rig::new();
        r.down(Surface::Strip, 306.0, 1350.0, 0);
        r.mv(306.0, 1200.0, 16);
        assert_eq!(r.s.view().lift, None);
    }

    #[test]
    fn reveal_travel_never_negative() {
        let mut r = Rig::new();
        r.down(Surface::Strip, 306.0, 1350.0, 0);
        r.mv(306.0, 1200.0, 16);
        r.mv(306.0, 1400.0, 32);
        assert_eq!(r.s.view().reveal_travel, Some(0.0));
    }

    #[test]
    fn release_seq_increments_once_per_up() {
        let mut r = Rig::new();
        r.reveal();
        r.up(50);
        assert_eq!(r.s.view().release.map(|x| x.seq), Some(1));
        r.down(Surface::Bar, 306.0, BAR_Y, 1000);
        r.up(1050);
        assert_eq!(r.s.view().release.map(|x| x.seq), Some(2));
        r.mv(100.0, 100.0, 1100);
        assert_eq!(r.s.view().release.map(|x| x.seq), Some(2));
    }

    #[test]
    fn release_carries_velocity() {
        let mut r = Rig::new();
        r.down(Surface::Strip, 306.0, 1350.0, 0);
        for k in 1..=10u64 {
            r.mv(306.0, 1350.0 - 4.0 * k as f32, 8 * k);
        }
        r.up(84);
        assert!(matches!(r.mode(), Mode::Minimized { .. }));
        assert!(r.s.view().release.unwrap().vy < 0.0);
    }

    #[test]
    fn flick_sets_exit_up_and_last_page() {
        let mut r = Rig::minimized(2);
        let x = r.s.icon_center_x(2);
        r.bar_flick(x);
        let v = r.s.view();
        assert_eq!(v.exit, Some(Exit::Up));
        assert_eq!(v.last_page, 2);
        assert_eq!(v.overlay, None);
    }

    #[test]
    fn close_sets_exit_down() {
        let mut r = Rig::minimized(3);
        r.down(Surface::Overlay, 306.0, 600.0, 3000);
        r.mv(306.0, 650.0, 3016);
        r.mv(306.0, 695.0, 3032);
        r.up(3200);
        let v = r.s.view();
        assert_eq!(v.mode, Mode::Hidden);
        assert_eq!(v.exit, Some(Exit::Down));
        assert_eq!(v.last_page, 3);
    }

    #[test]
    fn reveal_clears_exit() {
        let mut r = Rig::minimized(3);
        r.down(Surface::Overlay, 306.0, 600.0, 3000);
        r.mv(306.0, 650.0, 3016);
        r.mv(306.0, 695.0, 3032);
        r.up(3200);
        assert_eq!(r.s.view().exit, Some(Exit::Down));
        r.down(Surface::Strip, 306.0, 1350.0, 4000);
        r.mv(306.0, 1340.0, 4016);
        assert_eq!(r.s.view().exit, None);
    }

    #[test]
    fn cancel_strip_drag_exits_down() {
        let mut r = Rig::new();
        r.reveal();
        r.s.handle(DockEvent::Cancel);
        let v = r.s.view();
        assert_eq!(v.exit, Some(Exit::Down));
        assert_eq!(v.last_page, 2);
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
