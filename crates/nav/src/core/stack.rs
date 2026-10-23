//! Routes touches and ticks to every container's triggers and motions, and
//! holds each container's open/closed state.

use std::rc::Rc;
use std::time::{Duration, Instant};

use gestures::{AxisLock, VelocityTracker};
use screen::Screen;

use super::AreaId;
use super::config::NavConfig;
use super::container::{ContainerId, ContainerSpec, Entry};
use super::gesture::{Detect, Gesture, Phase};
use super::input::Input;
use super::motion::{Final, INSTANT, Motion, Visual};
use super::trigger::ExternalEvent;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ContainerState {
    /// How far open it is right now, 0 to 1.
    pub presence: f32,
    /// Where it is heading.
    pub target: Final,
}

impl ContainerState {
    pub fn visible(&self) -> bool {
        self.presence > 0.0 || self.target == Final::Open
    }
}

struct Node {
    id: ContainerId,
    parent: Option<usize>,
    children: Vec<usize>,
    entries: Vec<Entry>,
    state: ContainerState,
    /// The motion that last drove this container.
    driver: Rc<dyn Motion>,
}

/// An entry of a node that the live gesture may still fire.
#[derive(Clone, Copy)]
struct Candidate {
    node: usize,
    entry: usize,
}

struct Live {
    contact: u32,
    start_t: Instant,
    gesture: Gesture,
    velocity: VelocityTracker,
    axis: AxisLock,
    candidates: Vec<Candidate>,
}

/// A container's `(id, parent, screen)`, as returned by [`Stack::new`].
pub type ScreenRow = (ContainerId, Option<ContainerId>, Box<dyn Screen>);

pub struct Stack {
    cfg: NavConfig,
    /// In z order: depth-first, declaration order, a parent before its children.
    nodes: Vec<Node>,
    live: Option<Live>,
}

impl Stack {
    /// Returns the stack and, in z order, each container's `(id, parent, screen)`.
    pub fn new(cfg: NavConfig, roots: Vec<ContainerSpec>) -> (Stack, Vec<ScreenRow>) {
        let mut stack = Stack {
            cfg,
            nodes: Vec::new(),
            live: None,
        };
        let mut rows = Vec::new();
        for root in roots {
            stack.add(root, None, &mut rows);
        }
        (stack, rows)
    }

    fn add(&mut self, spec: ContainerSpec, parent: Option<usize>, rows: &mut Vec<ScreenRow>) {
        assert!(
            self.nodes.iter().all(|n| n.id != spec.id),
            "duplicate container id {:?}",
            spec.id
        );
        let index = self.nodes.len();
        let target = if spec.initially_open {
            Final::Open
        } else {
            Final::Closed
        };
        self.nodes.push(Node {
            id: spec.id,
            parent,
            children: Vec::new(),
            entries: spec.entries,
            state: ContainerState {
                presence: target.value(),
                target,
            },
            driver: Rc::new(INSTANT),
        });
        if let Some(p) = parent {
            self.nodes[p].children.push(index);
        }
        rows.push((spec.id, parent.map(|p| self.nodes[p].id), spec.screen));
        for child in spec.children {
            self.add(child, Some(index), rows);
        }
    }

    pub fn handle(&mut self, input: Input) {
        match input {
            Input::Down {
                contact,
                area,
                area_size,
                pos,
                t,
            } => {
                if self.live.is_some() {
                    return;
                }
                let candidates = self.candidates(area);
                let mut velocity = VelocityTracker::new(
                    self.cfg.velocity_window,
                    self.cfg.velocity_stale,
                    self.cfg.velocity_min_span,
                );
                velocity.push(t, pos.x, pos.y);
                self.live = Some(Live {
                    contact,
                    start_t: t,
                    gesture: Gesture {
                        area,
                        area_size,
                        start: pos,
                        pos,
                        elapsed: Duration::ZERO,
                        velocity: (0.0, 0.0),
                        axis: None,
                    },
                    velocity,
                    axis: AxisLock::new(self.cfg.axis_lock_px),
                    candidates,
                });
            }
            Input::Move { contact, pos, t } => {
                let Some(live) = self.live.as_mut().filter(|l| l.contact == contact) else {
                    return;
                };
                live.velocity.push(t, pos.x, pos.y);
                let g = &mut live.gesture;
                g.pos = pos;
                g.elapsed = t.saturating_duration_since(live.start_t);
                g.velocity = live.velocity.velocity(t);
                let (dx, dy) = g.delta();
                g.axis = live.axis.update(dx, dy);
                let nodes = &mut self.nodes;
                let cfg = &self.cfg;
                live.candidates.retain(|c| {
                    let node = &mut nodes[c.node];
                    let entry = &node.entries[c.entry];
                    match entry.trigger.detect(&live.gesture, Phase::Moving, cfg) {
                        Detect::Progress(p) => {
                            node.state.presence =
                                entry.motion.follow(node.state.presence, entry.toward, p);
                            true
                        }
                        Detect::Ignore => false,
                        Detect::Commit | Detect::Cancel => true,
                    }
                });
            }
            Input::Up { contact, t } => {
                if self.live.as_ref().is_none_or(|l| l.contact != contact) {
                    return;
                }
                let mut live = self.live.take().unwrap();
                let g = &mut live.gesture;
                g.elapsed = t.saturating_duration_since(live.start_t);
                g.velocity = live.velocity.velocity(t);
                let mut driven = Vec::new();
                for c in live.candidates {
                    let entry = &self.nodes[c.node].entries[c.entry];
                    match entry
                        .trigger
                        .detect(&live.gesture, Phase::Released, &self.cfg)
                    {
                        Detect::Commit => self.commit(c, &mut driven),
                        Detect::Cancel => {
                            self.drive(c);
                            driven.push(c.node);
                        }
                        Detect::Progress(_) | Detect::Ignore => {}
                    }
                }
                self.settle(&driven, Duration::ZERO);
            }
            Input::Cancel => {
                let Some(live) = self.live.take() else { return };
                let mut driven = Vec::new();
                for c in live.candidates {
                    let state = self.nodes[c.node].state;
                    if state.presence != state.target.value() {
                        self.drive(c);
                        driven.push(c.node);
                    }
                }
                self.settle(&driven, Duration::ZERO);
            }
            Input::Tick { dt } => {
                // A container the finger is holding stays where the finger put it.
                let held: Vec<usize> = self
                    .live
                    .iter()
                    .flat_map(|l| l.candidates.iter().map(|c| c.node))
                    .collect();
                let moving: Vec<usize> = (0..self.nodes.len())
                    .filter(|i| self.is_animating(*i) && !held.contains(i))
                    .collect();
                self.settle(&moving, dt);
            }
            Input::External(e) => self.external(&e),
        }
    }

    fn external(&mut self, e: &ExternalEvent) {
        let mut driven = Vec::new();
        for node in 0..self.nodes.len() {
            for entry in 0..self.nodes[node].entries.len() {
                if self.nodes[node].entries[entry].trigger.on_external(e) {
                    self.commit(Candidate { node, entry }, &mut driven);
                }
            }
        }
        self.settle(&driven, Duration::ZERO);
    }

    /// The entries on `area` that may fire from the current state.
    fn candidates(&self, area: AreaId) -> Vec<Candidate> {
        let mut out = Vec::new();
        for (i, node) in self.nodes.iter().enumerate() {
            let target = node.state.target;
            let can_open = target == Final::Closed
                && node.parent.is_none_or(|p| {
                    let parent = &self.nodes[p];
                    parent.state.target == Final::Open
                        && parent
                            .children
                            .iter()
                            .all(|s| *s == i || self.nodes[*s].state.target != Final::Open)
                });
            for (e, entry) in node.entries.iter().enumerate() {
                if entry.trigger.area() != area {
                    continue;
                }
                let eligible = match entry.toward {
                    Final::Open => can_open,
                    Final::Closed => target == Final::Open,
                };
                if eligible {
                    out.push(Candidate { node: i, entry: e });
                }
            }
        }
        out
    }

    fn drive(&mut self, c: Candidate) {
        let node = &mut self.nodes[c.node];
        node.driver = node.entries[c.entry].motion.clone();
    }

    /// Sets the target from the entry, records its motion and, on close,
    /// closes every descendant at once.
    fn commit(&mut self, c: Candidate, driven: &mut Vec<usize>) {
        self.drive(c);
        driven.push(c.node);
        let toward = self.nodes[c.node].entries[c.entry].toward;
        self.nodes[c.node].state.target = toward;
        if toward == Final::Closed {
            let mut stack = self.nodes[c.node].children.clone();
            while let Some(d) = stack.pop() {
                let node = &mut self.nodes[d];
                node.state = ContainerState {
                    presence: 0.0,
                    target: Final::Closed,
                };
                stack.extend(node.children.iter().copied());
            }
        }
    }

    fn settle(&mut self, nodes: &[usize], dt: Duration) {
        for &i in nodes {
            let node = &mut self.nodes[i];
            node.state.presence =
                node.driver
                    .settle(node.state.presence, node.state.target, dt, &self.cfg);
        }
    }

    fn index(&self, id: ContainerId) -> usize {
        self.nodes
            .iter()
            .position(|n| n.id == id)
            .unwrap_or_else(|| panic!("unknown container {id:?}"))
    }

    fn is_animating(&self, i: usize) -> bool {
        let s = self.nodes[i].state;
        s.presence != s.target.value()
    }

    pub fn state(&self, id: ContainerId) -> ContainerState {
        self.nodes[self.index(id)].state
    }

    /// How the container is drawn, from the motion that last drove it.
    pub fn visual(&self, id: ContainerId) -> Visual {
        let node = &self.nodes[self.index(id)];
        node.driver.visual(node.state.presence)
    }

    /// The containers whose presence has not reached their target.
    pub fn animating(&self) -> Vec<ContainerId> {
        (0..self.nodes.len())
            .filter(|i| self.is_animating(*i))
            .map(|i| self.nodes[i].id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use mecha_wayland::prelude::{App, NodeId, Point, Size};
    use screen::Screen;

    use super::*;
    use crate::core::{
        AreaId, BOTTOM_EDGE, Direction, FADE, Final, INSTANT, Motion, NavConfig, Span, TOP_EDGE,
        Timing, container, swipe,
    };

    const HOME: AreaId = AreaId("home");
    const PANEL: AreaId = AreaId("panel");
    const CLOSER: AreaId = AreaId("closer");

    const HOME_ID: ContainerId = ContainerId("home");
    const DRAWER: ContainerId = ContainerId("drawer");
    const SEARCH: ContainerId = ContainerId("search");
    const RUNNING: ContainerId = ContainerId("running");
    const NOTIFICATIONS: ContainerId = ContainerId("notifications");
    const QUICK_SETTINGS: ContainerId = ContainerId("quick_settings");

    const ALL: [ContainerId; 6] = [
        HOME_ID,
        DRAWER,
        SEARCH,
        RUNNING,
        NOTIFICATIONS,
        QUICK_SETTINGS,
    ];

    struct Dummy;
    impl Screen for Dummy {
        fn build(&self, _app: &mut App, parent: NodeId) -> NodeId {
            parent
        }
    }

    fn bottom_quick() -> crate::core::Swipe {
        swipe(BOTTOM_EDGE, Direction::Up, Timing::Quick)
    }

    fn drawer(m: impl Motion + Clone) -> ContainerSpec {
        container(DRAWER, Dummy)
            .open_close(swipe(HOME, Direction::Up, Timing::Any), m)
            .close(bottom_quick(), INSTANT)
    }

    fn home(initially_open: bool, drawer: ContainerSpec) -> ContainerSpec {
        let home = container(HOME_ID, Dummy)
            .open(bottom_quick(), FADE)
            .child(drawer)
            .child(
                container(SEARCH, Dummy)
                    .open_close(swipe(HOME, Direction::Right, Timing::Any), INSTANT)
                    .close(bottom_quick(), INSTANT),
            );
        if initially_open {
            home.initially_open()
        } else {
            home
        }
    }

    fn rest(home: ContainerSpec) -> Vec<ContainerSpec> {
        let panel = |id, span| {
            container(id, Dummy)
                .open(
                    swipe(TOP_EDGE, Direction::Down, Timing::Any).span(span),
                    INSTANT,
                )
                .close(swipe(PANEL, Direction::Up, Timing::Any), INSTANT)
        };
        vec![
            home,
            container(RUNNING, Dummy)
                .open(swipe(BOTTOM_EDGE, Direction::Up, Timing::Held), INSTANT)
                .close(bottom_quick(), INSTANT),
            panel(NOTIFICATIONS, Span::LeftHalf),
            panel(QUICK_SETTINGS, Span::RightHalf),
        ]
    }

    fn build(roots: Vec<ContainerSpec>) -> Stack {
        Stack::new(NavConfig::default(), roots).0
    }

    fn tree() -> Stack {
        build(rest(home(true, drawer(INSTANT))))
    }

    fn tree_home_closed() -> Stack {
        build(rest(home(false, drawer(INSTANT))))
    }

    fn size_of(area: AreaId) -> Size {
        if area == BOTTOM_EDGE || area == TOP_EDGE {
            Size {
                width: 600.0,
                height: 20.0,
            }
        } else {
            Size {
                width: 600.0,
                height: 1000.0,
            }
        }
    }

    fn pt(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    const STEP: Duration = Duration::from_millis(8);
    /// Long enough to fill the velocity window, so the release velocity is exactly the tail's.
    const TAIL: Duration = Duration::from_millis(80);

    /// Moves along a line from `from`, then ends with `TAIL` of motion at
    /// `end_velocity` (px/ms) that lands on `to`, at `duration` (> `TAIL`).
    fn moves(
        from: Point,
        to: Point,
        duration: Duration,
        end_velocity: (f32, f32),
    ) -> Vec<(Duration, Point)> {
        let tail_ms = TAIL.as_secs_f32() * 1000.0;
        let tail_start = pt(
            to.x - end_velocity.0 * tail_ms,
            to.y - end_velocity.1 * tail_ms,
        );
        let body = duration - TAIL;
        let mut out = Vec::new();
        let mut t = STEP;
        while t <= duration {
            let p = if t <= body {
                let f = t.as_secs_f32() / body.as_secs_f32();
                pt(
                    from.x + (tail_start.x - from.x) * f,
                    from.y + (tail_start.y - from.y) * f,
                )
            } else {
                let f = (t - body).as_secs_f32() / TAIL.as_secs_f32();
                pt(
                    tail_start.x + (to.x - tail_start.x) * f,
                    tail_start.y + (to.y - tail_start.y) * f,
                )
            };
            out.push((t, p));
            t += STEP;
        }
        out
    }

    fn down(stack: &mut Stack, contact: u32, area: AreaId, pos: Point, t: Instant) {
        stack.handle(Input::Down {
            contact,
            area,
            area_size: size_of(area),
            pos,
            t,
        });
    }

    fn swipe_on(
        stack: &mut Stack,
        area: AreaId,
        from: Point,
        to: Point,
        duration: Duration,
        end_velocity: (f32, f32),
    ) {
        let t0 = Instant::now();
        down(stack, 1, area, from, t0);
        for (dt, pos) in moves(from, to, duration, end_velocity) {
            stack.handle(Input::Move {
                contact: 1,
                pos,
                t: t0 + dt,
            });
        }
        stack.handle(Input::Up {
            contact: 1,
            t: t0 + duration,
        });
    }

    fn quick_bottom(stack: &mut Stack) {
        swipe_on(
            stack,
            BOTTOM_EDGE,
            pt(300.0, 10.0),
            pt(300.0, -110.0),
            Duration::from_millis(150),
            (0.0, -1.5),
        );
    }

    fn held_bottom(stack: &mut Stack) {
        swipe_on(
            stack,
            BOTTOM_EDGE,
            pt(300.0, 10.0),
            pt(300.0, -110.0),
            Duration::from_millis(500),
            (0.0, 0.0),
        );
    }

    fn home_up(stack: &mut Stack) {
        swipe_on(
            stack,
            HOME,
            pt(300.0, 800.0),
            pt(300.0, 600.0),
            Duration::from_millis(200),
            (0.0, -1.0),
        );
    }

    fn home_right(stack: &mut Stack) {
        swipe_on(
            stack,
            HOME,
            pt(100.0, 500.0),
            pt(300.0, 500.0),
            Duration::from_millis(200),
            (1.0, 0.0),
        );
    }

    fn top_down(stack: &mut Stack, x: f32) {
        swipe_on(
            stack,
            TOP_EDGE,
            pt(x, 0.0),
            pt(x, 200.0),
            Duration::from_millis(200),
            (0.0, 1.0),
        );
    }

    fn panel_up(stack: &mut Stack) {
        swipe_on(
            stack,
            PANEL,
            pt(300.0, 600.0),
            pt(300.0, 400.0),
            Duration::from_millis(200),
            (0.0, -1.0),
        );
    }

    fn target(stack: &Stack, id: ContainerId) -> Final {
        stack.state(id).target
    }

    fn open_ones(stack: &Stack) -> Vec<ContainerId> {
        ALL.into_iter()
            .filter(|id| target(stack, *id) == Final::Open)
            .collect()
    }

    fn settled(stack: &Stack) -> bool {
        ALL.iter().all(|id| {
            let s = stack.state(*id);
            s.presence == s.target.value()
        })
    }

    #[test]
    fn home_starts_open_others_closed() {
        let (stack, rows) = Stack::new(NavConfig::default(), rest(home(true, drawer(INSTANT))));
        assert_eq!(open_ones(&stack), [HOME_ID]);
        assert_eq!(stack.state(HOME_ID).presence, 1.0);
        assert!(stack.state(HOME_ID).visible());
        assert!(!stack.state(DRAWER).visible());
        assert!(stack.animating().is_empty());
        let order: Vec<_> = rows.iter().map(|(id, parent, _)| (*id, *parent)).collect();
        assert_eq!(
            order,
            [
                (HOME_ID, None),
                (DRAWER, Some(HOME_ID)),
                (SEARCH, Some(HOME_ID)),
                (RUNNING, None),
                (NOTIFICATIONS, None),
                (QUICK_SETTINGS, None),
            ]
        );
    }

    #[test]
    fn quick_bottom_from_app_opens_home_with_fade() {
        let mut stack = tree_home_closed();
        assert!(open_ones(&stack).is_empty());
        quick_bottom(&mut stack);
        assert_eq!(target(&stack, HOME_ID), Final::Open);
        assert_eq!(stack.state(HOME_ID).presence, 0.0);
        assert_eq!(stack.visual(HOME_ID).cover, 1.0);
        assert_eq!(stack.animating(), [HOME_ID]);
        assert_eq!(open_ones(&stack), [HOME_ID]);

        stack.handle(Input::Tick {
            dt: Duration::from_millis(80),
        });
        assert_eq!(stack.visual(HOME_ID).cover, 0.5);
        stack.handle(Input::Tick {
            dt: Duration::from_millis(80),
        });
        assert_eq!(stack.state(HOME_ID).presence, 1.0);
        assert!(stack.animating().is_empty());
    }

    #[test]
    fn held_bottom_opens_running_apps() {
        let mut stack = tree();
        held_bottom(&mut stack);
        assert_eq!(open_ones(&stack), [HOME_ID, RUNNING]);
        assert!(settled(&stack));

        let mut stack = tree_home_closed();
        held_bottom(&mut stack);
        assert_eq!(open_ones(&stack), [RUNNING]);
        assert!(settled(&stack));
    }

    #[test]
    fn quick_bottom_from_running_closes_running_and_keeps_home() {
        let mut stack = tree();
        held_bottom(&mut stack);
        quick_bottom(&mut stack);
        assert_eq!(open_ones(&stack), [HOME_ID]);
        assert!(!stack.animating().contains(&HOME_ID));
        assert!(settled(&stack));
    }

    #[test]
    fn quick_bottom_from_drawer_returns_to_plain_home() {
        let mut stack = tree();
        home_up(&mut stack);
        assert_eq!(open_ones(&stack), [HOME_ID, DRAWER]);
        quick_bottom(&mut stack);
        assert_eq!(open_ones(&stack), [HOME_ID]);
        assert!(settled(&stack));
    }

    #[test]
    fn home_swipes_open_children_only_while_home_open() {
        let mut stack = tree_home_closed();
        home_up(&mut stack);
        home_right(&mut stack);
        assert!(open_ones(&stack).is_empty());

        let mut stack = tree();
        home_right(&mut stack);
        assert_eq!(open_ones(&stack), [HOME_ID, SEARCH]);
    }

    #[test]
    fn sibling_rule() {
        let mut stack = tree();
        home_up(&mut stack);
        home_right(&mut stack);
        assert_eq!(open_ones(&stack), [HOME_ID, DRAWER]);
    }

    #[test]
    fn top_left_opens_notifications_top_right_opens_quick_settings() {
        let mut stack = tree();
        top_down(&mut stack, 100.0);
        assert_eq!(open_ones(&stack), [HOME_ID, NOTIFICATIONS]);

        let mut stack = tree();
        top_down(&mut stack, 500.0);
        assert_eq!(open_ones(&stack), [HOME_ID, QUICK_SETTINGS]);
    }

    #[test]
    fn panel_swipe_up_closes_both() {
        let mut stack = tree();
        top_down(&mut stack, 100.0);
        top_down(&mut stack, 500.0);
        assert_eq!(open_ones(&stack), [HOME_ID, NOTIFICATIONS, QUICK_SETTINGS]);
        panel_up(&mut stack);
        assert_eq!(open_ones(&stack), [HOME_ID]);
        assert!(settled(&stack));
    }

    #[test]
    fn closing_home_closes_children() {
        let home =
            home(true, drawer(INSTANT)).close(swipe(CLOSER, Direction::Down, Timing::Any), INSTANT);
        let mut stack = build(rest(home));
        home_up(&mut stack);
        assert_eq!(open_ones(&stack), [HOME_ID, DRAWER]);
        swipe_on(
            &mut stack,
            CLOSER,
            pt(300.0, 100.0),
            pt(300.0, 300.0),
            Duration::from_millis(200),
            (0.0, 1.0),
        );
        assert!(open_ones(&stack).is_empty());
        assert_eq!(stack.state(DRAWER).presence, 0.0);
        assert!(settled(&stack));
    }

    /// Records every `follow` progress; otherwise behaves like `INSTANT`.
    #[derive(Clone)]
    struct Recorder(Rc<RefCell<Vec<f32>>>);

    impl Motion for Recorder {
        fn follow(&self, current: f32, _toward: Final, progress: f32) -> f32 {
            self.0.borrow_mut().push(progress);
            current
        }
        fn settle(&self, _current: f32, target: Final, _dt: Duration, _cfg: &NavConfig) -> f32 {
            target.value()
        }
    }

    #[test]
    fn progress_reaches_follow_every_move() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut stack = build(rest(home(true, drawer(Recorder(log.clone())))));
        let t0 = Instant::now();
        down(&mut stack, 1, HOME, pt(300.0, 800.0), t0);
        for (i, y) in [770.0, 740.0, 710.0].into_iter().enumerate() {
            stack.handle(Input::Move {
                contact: 1,
                pos: pt(300.0, y),
                t: t0 + STEP * (i as u32 + 1),
            });
        }
        let got = log.borrow().clone();
        assert_eq!(got.len(), 3, "{got:?}");
        assert!(got.windows(2).all(|w| w[0] < w[1]), "{got:?}");
        assert!(got.iter().all(|p| (0.0..=1.0).contains(p)), "{got:?}");
        assert_eq!(got[2], 1.0);
    }

    #[test]
    fn second_contact_is_ignored() {
        let mut stack = tree();
        let t0 = Instant::now();
        let from = pt(300.0, 800.0);
        let to = pt(300.0, 600.0);
        let duration = Duration::from_millis(200);
        down(&mut stack, 1, HOME, from, t0);
        for (n, (dt, pos)) in moves(from, to, duration, (0.0, -1.0))
            .into_iter()
            .enumerate()
        {
            stack.handle(Input::Move {
                contact: 1,
                pos,
                t: t0 + dt,
            });
            if n == 3 {
                down(&mut stack, 2, HOME, pt(100.0, 500.0), t0 + dt);
                stack.handle(Input::Move {
                    contact: 2,
                    pos: pt(400.0, 500.0),
                    t: t0 + dt,
                });
                stack.handle(Input::Up {
                    contact: 2,
                    t: t0 + dt,
                });
                stack.handle(Input::Up {
                    contact: 7,
                    t: t0 + dt,
                });
                assert_eq!(open_ones(&stack), [HOME_ID]);
            }
        }
        stack.handle(Input::Up {
            contact: 1,
            t: t0 + duration,
        });
        assert_eq!(open_ones(&stack), [HOME_ID, DRAWER]);
    }

    #[test]
    fn cancel_changes_nothing() {
        let mut stack = tree();
        let t0 = Instant::now();
        let from = pt(300.0, 800.0);
        down(&mut stack, 1, HOME, from, t0);
        for (dt, pos) in moves(
            from,
            pt(300.0, 600.0),
            Duration::from_millis(200),
            (0.0, -1.0),
        ) {
            stack.handle(Input::Move {
                contact: 1,
                pos,
                t: t0 + dt,
            });
        }
        stack.handle(Input::Cancel);
        stack.handle(Input::Up {
            contact: 1,
            t: t0 + Duration::from_millis(200),
        });
        assert_eq!(open_ones(&stack), [HOME_ID]);
        assert!(settled(&stack));
        // The stack takes a fresh gesture afterwards.
        home_up(&mut stack);
        assert_eq!(open_ones(&stack), [HOME_ID, DRAWER]);
    }

    #[test]
    fn diagonal_swipe_opens_one_child() {
        let mut stack = tree();
        swipe_on(
            &mut stack,
            HOME,
            pt(100.0, 800.0),
            pt(220.0, 700.0),
            Duration::from_millis(200),
            (0.6, -0.5),
        );
        assert_eq!(open_ones(&stack), [HOME_ID, SEARCH]);
    }
}
