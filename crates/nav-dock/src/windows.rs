//! The dock's windows and the reconciler that keeps them in step with
//! `NavState::view()`.
//!
//! A window never resizes (see the crate's constraints). There are two: the
//! swipe strip while Hidden, and the overlay, which holds the page row and
//! the dock/bar morph, in every other mode and while its exit motion runs.
//! Each is spawned when its mode needs it and removed when it does not,
//! except the window holding the live touch, which a touch stays bound to;
//! it goes on the first reconcile after that touch ends.

use std::time::Duration;

use gestures::Axis;
use mecha_wayland::prelude::*;
use nav_dock_api::Section;

use crate::dock::{Look, MorphUpdate, ScaleIcons};
use crate::motion;
use crate::overlay::{OverlayUpdate, parts, spawn_overlay};
use crate::state::{Mode, NavState, Surface, View};
use crate::strip::spawn_strip;

/// Everything the dock owns at run time.
pub(crate) struct NavDock {
    pub(crate) state: NavState,
    pub(crate) sections: Vec<Box<dyn Section>>,
    pub(crate) icons: Vec<SpriteId>,
    pub(crate) windows: WindowSet,
    /// The overlay window's last `Resized` height: it sits below the status
    /// bar, so it is shorter than the screen.
    pub(crate) overlay_height: Option<f32>,
    /// Contacts (in `DockEvent` ids) pressed on a dock window and not yet
    /// released or cancelled.
    pub(crate) pressed: Vec<u32>,
    /// The `Release::seq` whose throw has been applied.
    pub(crate) applied_release: u64,
    /// The axis the last held contact dragged the row or morph along, for
    /// its release's throw; `None` when it moved nothing.
    pub(crate) throw_axis: Option<Axis>,
    /// The last held contact was a bar drag driving the morph, so its
    /// release throws the morph too.
    pub(crate) lifted: bool,
    /// An overlay spawned at its hidden position whose first layout has not
    /// resolved yet. The framework snaps a node's first layout, so the real
    /// targets are only pushed once it has, and the page slides in.
    pub(crate) unresolved: Option<NodeId>,
}
impl Resource for NavDock {}

impl NavDock {
    /// The overlay's size: the screen's width, and its last `Resized`
    /// height (the screen's until then).
    pub(crate) fn overlay_size(&self) -> Size {
        let screen = self.state.screen();
        Size::new(screen.width, self.overlay_height.unwrap_or(screen.height))
    }

    /// The overlay window's laid-out size, read from `ComputedLayout`; the
    /// `Resized`-fed `overlay_size` until its first layout.
    fn laid_out_size(&self, app: &App) -> Size {
        let fallback = self.overlay_size();
        self.windows
            .overlay
            .and_then(|w| app.component::<ComputedLayout>(w))
            .map_or(fallback, |c| {
                let (w, h) = (c.rect.width(), c.rect.height());
                Size::new(
                    if w > 0.0 { w } else { fallback.width },
                    if h > 0.0 { h } else { fallback.height },
                )
            })
    }

    /// The row's and the morph's animation for the targets of `view`,
    /// consuming its release.
    fn motion_settings(
        &mut self,
        view: &View,
        distance: f32,
    ) -> (LayoutAnimationSettings, LayoutAnimationSettings) {
        if let Some(surface) = view.held_surface {
            let (dx, dy) = view.overlay.map_or((0.0, 0.0), |o| (o.dx, o.dy));
            self.throw_axis = match surface {
                // The row slides up on its own: nothing to throw.
                Surface::Strip => None,
                Surface::Overlay if dx != 0.0 => Some(Axis::Horizontal),
                Surface::Overlay if dy != 0.0 => Some(Axis::Vertical),
                Surface::Bar if view.lift.is_some() => Some(Axis::Vertical),
                Surface::Overlay | Surface::Bar | Surface::Dock => None,
            };
            self.lifted = view.lift.is_some();
        }
        let (seq, axis, config) = (self.applied_release, self.throw_axis, self.state.config());
        let settings = (
            motion::settings(view, seq, axis, distance, config),
            motion::morph_settings(view, seq, axis, distance, config, self.lifted),
        );
        self.applied_release = view.release.map_or(self.applied_release, |r| r.seq);
        settings
    }
}

/// The live dock windows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct WindowSet {
    pub(crate) strip: Option<NodeId>,
    pub(crate) overlay: Option<NodeId>,
}

/// The dock's two windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Strip,
    Overlay,
}

impl Kind {
    const ALL: [Kind; 2] = [Kind::Strip, Kind::Overlay];

    /// The window a touch on `surface` is bound to: the bar and the dock
    /// live in the overlay.
    fn of(surface: Surface) -> Kind {
        match surface {
            Surface::Strip => Kind::Strip,
            Surface::Bar | Surface::Dock | Surface::Overlay => Kind::Overlay,
        }
    }
}

impl WindowSet {
    fn slot(&mut self, kind: Kind) -> &mut Option<NodeId> {
        match kind {
            Kind::Strip => &mut self.strip,
            Kind::Overlay => &mut self.overlay,
        }
    }
}

/// Brings the windows in line with the state's view: removes what the mode
/// no longer wants (unless it holds the live touch, or is the overlay still
/// running its exit), spawns what is missing, and pushes positions, look
/// and emphasis into the widgets.
pub(crate) fn reconcile(app: &mut App) {
    let (view, mut set) = {
        let nav = app.resource::<NavDock>();
        (nav.state.view(), nav.windows)
    };
    let held = view.held_surface.map(Kind::of);

    for kind in Kind::ALL {
        let slot = set.slot(kind);
        let Some(window) = *slot else { continue };
        if !app.is_live(window) {
            *slot = None;
        } else if !wanted(view, kind)
            && held != Some(kind)
            && !(kind == Kind::Overlay && exiting(app, view, window))
        {
            app.remove(window);
            *slot = None;
        }
    }
    for kind in Kind::ALL {
        if wanted(view, kind) && set.slot(kind).is_none() {
            let window = spawn(app, kind, view);
            *set.slot(kind) = Some(window);
            if kind == Kind::Overlay {
                app.resource_mut::<NavDock>().unresolved = Some(window);
            }
        }
    }
    app.resource_mut::<NavDock>().windows = set;

    // A fresh overlay keeps its hidden spawn position until it has laid out.
    if let Some(window) = app.resource::<NavDock>().unresolved {
        let resolved = parts(app, window).is_some_and(|(row, _)| {
            app.component::<ComputedLayout>(row)
                .is_some_and(|c| c.rect.size.width > 0.0)
        });
        if app.is_live(window) && !resolved {
            return;
        }
        app.resource_mut::<NavDock>().unresolved = None;
    }

    let distance = travel_of(app, view, set.overlay);
    let (row, morph) = app
        .resource_mut::<NavDock>()
        .motion_settings(&view, distance);
    push(app, view, row, morph);
}

/// How far the overlay's row and morph have to travel from where they are
/// displayed to the targets of `view`; zero without an overlay.
fn travel_of(app: &App, view: View, overlay: Option<NodeId>) -> f32 {
    let Some((row, morph)) = overlay
        .and_then(|o| parts(app, o).map(|p| (o, p)))
        .map(|(_, p)| p)
    else {
        return 0.0;
    };
    let nav = app.resource::<NavDock>();
    let size = nav.laid_out_size(app);
    let targets = motion::targets(&view, nav.state.config(), size);
    let (Some(row), Some(morph)) = (app.component::<Layout>(row), app.component::<Layout>(morph))
    else {
        return 0.0;
    };
    motion::travel(
        targets,
        (row.rect.x(), row.rect.y()),
        size.height - morph.rect.bottom(),
        size,
    )
}

/// Sends the overlay's row and morph their targets for `view`, reached
/// under `row_settings` and `morph_settings`.
fn push(
    app: &mut App,
    view: View,
    row_settings: LayoutAnimationSettings,
    morph_settings: LayoutAnimationSettings,
) {
    let (overlay, size, targets, config) = {
        let nav = app.resource::<NavDock>();
        let size = nav.overlay_size();
        let targets = motion::targets(&view, nav.state.config(), nav.laid_out_size(app));
        (
            nav.windows.overlay,
            size,
            targets,
            nav.state.config().clone(),
        )
    };
    let Some((row, morph)) = overlay.and_then(|o| parts(app, o)) else {
        return;
    };
    let update = OverlayUpdate {
        left: targets.row_left,
        top: targets.row_top,
        size,
        settings: row_settings,
    };
    app.emit(update, row);
    let update = MorphUpdate {
        t: motion::morph_t(&view, &config),
        focus: Look::of(view.mode).map(|(_, focus)| focus),
        bottom: targets.morph_bottom,
        overlay: size,
        settings: morph_settings,
        snap: motion::morph_coupled(&view),
    };
    app.emit(update, morph);
}

/// Whether the overlay `window` is still running the exit motion of a
/// Hidden `view`: until the row's displayed rect has reached its target,
/// and that target is the exit's.
fn exiting(app: &App, view: View, window: NodeId) -> bool {
    if view.mode != Mode::Hidden || view.exit.is_none() {
        return false;
    }
    let Some((row, morph)) = parts(app, window) else {
        return false;
    };
    let (target, size) = {
        let nav = app.resource::<NavDock>();
        let size = nav.laid_out_size(app);
        (motion::targets(&view, nav.state.config(), size), size)
    };
    let settled = |node: NodeId| {
        app.component::<Layout>(node).map(|l| l.rect)
            == app.component::<ComputedLayout>(node).map(|c| c.rect)
    };
    let Some(computed) = app.component::<ComputedLayout>(row) else {
        return false;
    };
    // Until layout has resolved the exit's target, the row has not left.
    let top = motion::resolve(target.row_top, size.height);
    let resolved = (computed.rect.y() - top).abs() < 0.5;
    !(resolved && settled(row) && settled(morph))
}

/// The window table: the strip while Hidden, the overlay otherwise.
fn wanted(view: View, kind: Kind) -> bool {
    match kind {
        Kind::Strip => view.strip,
        Kind::Overlay => view.overlay.is_some(),
    }
}

fn spawn(app: &mut App, kind: Kind, view: View) -> NodeId {
    let (config, icons, size) = {
        let nav = app.resource::<NavDock>();
        (
            nav.state.config().clone(),
            nav.icons.clone(),
            // The last overlay height, so the first frame does not jump
            // from the full screen height to the overlay's own.
            nav.overlay_size(),
        )
    };
    match kind {
        Kind::Strip => spawn_strip(app, &config),
        Kind::Overlay => {
            let look = Look::of(view.mode).expect("overlay is only wanted while shown");
            // Spawned below the screen, at its hidden position: the first
            // layout is snapped, and the real targets then animate from it.
            let targets = motion::Targets {
                row_top: percent(100.0),
                morph_bottom: -(config.dock_size.1.max(config.bar_size.1) + config.bar_bottom_gap),
                ..motion::targets(&view, &config, size)
            };
            // `spawn_page` needs the whole app, so the sections step out
            // of the resource while their pages are built.
            let sections = std::mem::take(&mut app.resource_mut::<NavDock>().sections);
            let window = spawn_overlay(app, &config, &sections, &icons, targets, look);
            app.resource_mut::<NavDock>().sections = sections;
            window
        }
    }
}

/// Every window's settled size feeds the state's screen (icon centres,
/// paging and window origins). Windows other than home are smaller than the
/// screen, so the screen only grows, to the component-wise max of all of
/// them, except the width, which the overlay sets. The overlay's height is also kept for its own origin. A change of
/// the overlay's size moves its row and morph at once: a size change never
/// slides them.
pub(crate) fn on_resized(app: &mut App, e: &Emitted<Resized>) {
    let (view, before, after) = {
        let mut nav = app.resource_mut::<NavDock>();
        let before = nav.overlay_size();
        let old = nav.state.screen();
        let from_overlay = nav.windows.overlay.is_some_and(|w| e.targets.contains(&w));
        // The overlay spans the screen's width: its width is the width,
        // not a floor that a stale larger fallback could keep above it.
        let width = if from_overlay {
            e.event.size.width
        } else {
            old.width.max(e.event.size.width)
        };
        nav.state
            .set_screen(Size::new(width, old.height.max(e.event.size.height)));
        if from_overlay {
            nav.overlay_height = Some(e.event.size.height);
        }
        (nav.state.view(), before, nav.overlay_size())
    };
    if before != after {
        let snap = eased_layout(Duration::ZERO);
        push(app, view, snap, snap);
    }
}

/// On each overlay frame, after layout animation has advanced, the morph's
/// icons are redrawn at their displayed size. The frame drawn in the same
/// signal may still show the previous size: about one frame of lag. While the overlay runs
/// an exit, a reconcile then removes it once it is done.
pub(crate) fn on_frame(app: &mut App, frame: &Frame) {
    let overlay = app.resource::<NavDock>().windows.overlay;
    if overlay != Some(frame.0) {
        return;
    }
    if app.resource::<NavDock>().unresolved == overlay {
        reconcile(app);
    }
    if let Some((_, morph)) = parts(app, frame.0) {
        app.emit(ScaleIcons, morph);
    }
    let view = app.resource::<NavDock>().state.view();
    if view.mode == Mode::Hidden && view.exit.is_some() {
        reconcile(app);
    }
}

/// An Overlay-layer role anchored to the bottom edge only, so the
/// compositor centres the surface horizontally. The strip.
pub(crate) fn bottom_centre_role(namespace: &str) -> Role {
    Role::Layer(LayerRole {
        layer: Layer::Overlay,
        anchor: Anchor::BOTTOM,
        exclusive_zone: 0,
        namespace: namespace.into(),
        keyboard_interactivity: KeyboardInteractivity::None,
    })
}

/// Cubic ease-out over `anim`; a zero `anim` snaps.
pub(crate) fn eased_layout(anim: Duration) -> LayoutAnimationSettings {
    LayoutAnimationSettings::new(AnimationTime::Duration(anim), Easing::EaseOutCubic)
}

pub(crate) fn eased_paint(anim: Duration) -> PaintAnimationSettings {
    PaintAnimationSettings::new(AnimationTime::Duration(anim), Easing::EaseOutCubic)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::time::{Duration, Instant};

    use mecha_wayland::mechanix_widgets::prelude::FontBook;
    use nav_dock_api::placeholder_page;
    use status_bar::GEIST_MONO_BYTES;

    use super::*;
    use crate::config::NavDockConfig;
    use crate::dock::{DockMorph, Look};
    use crate::module::NavDockModule;
    use crate::overlay::parts;
    use crate::state::DockEvent;

    const DOT_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><circle cx="128" cy="128" r="96"/></svg>"#;
    const IDS: [&str; 5] = [
        "favourites",
        "search",
        "recents",
        "notifications",
        "quick-settings",
    ];

    struct Fake(&'static str);

    impl Section for Fake {
        fn id(&self) -> &'static str {
            self.0
        }
        fn label(&self) -> &'static str {
            self.0
        }
        fn icon_svg(&self) -> &'static [u8] {
            DOT_SVG
        }
        fn spawn_page(&self, app: &mut App, parent: NodeId) -> NodeId {
            placeholder_page(app, parent, self.0)
        }
    }

    struct Rig {
        app: App,
        t0: Instant,
    }

    /// A headless app with the dock installed over five fake sections,
    /// ticked once: only the strip is up.
    pub(crate) fn dock_app() -> App {
        dock_app_with(NavDockConfig::default())
    }

    fn dock_app_with(config: NavDockConfig) -> App {
        let mut app = App::new();
        app.add_module(LayoutModule)
            .add_module(PaintModule)
            .add_module(WindowModule)
            .add_module(AnimationModule)
            .add_module(MechanixTheme::dark())
            .insert_resource(Atlas::new());
        // PresentationModule registers Role; it needs a live compositor.
        app.register_component::<Role>();
        let font = app
            .resource_mut::<Atlas>()
            .add_font(GEIST_MONO_BYTES)
            .expect("GeistMono loads");
        app.insert_resource(FontBook::new(font));

        let mut module = NavDockModule::new(config);
        for id in IDS {
            module = module.section(Fake(id));
        }
        app.add_module(module);
        app.tick();
        app
    }

    impl Rig {
        fn new() -> Self {
            Self::with(NavDockConfig::default())
        }

        fn with(config: NavDockConfig) -> Self {
            Self {
                app: dock_app_with(config),
                t0: Instant::now(),
            }
        }

        fn handle(&mut self, ev: DockEvent) {
            self.app.resource_mut::<NavDock>().state.handle(ev);
            reconcile(&mut self.app);
            self.app.tick();
            self.size_overlay();
        }

        /// A compositor sizes a fresh overlay with a `Resized`; a headless
        /// window has no size until it is told (the screen's, as ever).
        fn size_overlay(&mut self) {
            let Some(overlay) = self.set().overlay else {
                return;
            };
            let laid_out = |r: &Rig| {
                r.app
                    .component::<ComputedLayout>(overlay)
                    .is_some_and(|c| c.rect.size.width > 0.0)
            };
            if !laid_out(self) {
                let size = self.app.resource::<NavDock>().state.screen();
                self.app.emit(Resized { size }, overlay);
                self.app.tick();
                reconcile(&mut self.app);
                self.app.tick();
            }
        }

        fn at(&self, ms: u64) -> Instant {
            self.t0 + Duration::from_millis(ms)
        }

        fn down(&mut self, surface: Surface, x: f32, y: f32, ms: u64) {
            let t = self.at(ms);
            self.handle(DockEvent::Down {
                contact: 0,
                surface,
                pos: Point::new(x, y),
                t,
            });
        }

        fn mv(&mut self, x: f32, y: f32, ms: u64) {
            let t = self.at(ms);
            self.handle(DockEvent::Move {
                contact: 0,
                pos: Point::new(x, y),
                t,
            });
        }

        fn up(&mut self, ms: u64) {
            let t = self.at(ms);
            self.handle(DockEvent::Up { contact: 0, t });
        }

        /// Strip touch 10 px up: Dragging, finger still on the strip.
        fn reveal(&mut self) {
            self.down(Surface::Strip, 306.0, 1350.0, 0);
            self.mv(306.0, 1340.0, 16);
        }

        fn set(&self) -> WindowSet {
            self.app.resource::<NavDock>().windows
        }

        /// Every live window's layer-shell namespace, in spawn order.
        fn namespaces(&self) -> Vec<String> {
            self.app
                .resource::<Windows>()
                .iter()
                .map(|w| match self.app.component::<Role>(w) {
                    Some(Role::Layer(layer)) => layer.namespace.clone(),
                    other => panic!("dock window without a layer role: {other:?}"),
                })
                .collect()
        }

        fn overlay(&self) -> NodeId {
            self.set().overlay.expect("overlay is up")
        }

        fn row(&self) -> NodeId {
            parts(&self.app, self.overlay())
                .expect("overlay is built")
                .0
        }

        fn morph(&self) -> NodeId {
            parts(&self.app, self.overlay())
                .expect("overlay is built")
                .1
        }

        /// The morph's target (not displayed) width.
        fn morph_width(&self) -> f32 {
            let morph = self.morph();
            self.app
                .component::<ComputedLayout>(morph)
                .unwrap()
                .rect
                .width()
        }

        fn morph_widget(&self) -> &DockMorph {
            self.app.widget::<DockMorph>(self.morph()).unwrap()
        }
    }

    const STRIP: &str = crate::strip::NAMESPACE;
    const OVERLAY: &str = crate::overlay::NAMESPACE;

    #[test]
    fn hidden_spawns_only_strip() {
        let r = Rig::new();
        assert_eq!(r.namespaces(), vec![STRIP]);
    }

    #[test]
    fn reveal_spawns_overlay_and_keeps_strip_while_held() {
        let mut r = Rig::new();
        r.reveal();
        assert_eq!(r.namespaces(), vec![STRIP, OVERLAY]);
    }

    #[test]
    fn only_strip_and_overlay_windows_exist() {
        let mut r = Rig::new();
        let only_ours = |r: &Rig| {
            let ns = r.namespaces();
            assert!(
                ns.iter().all(|n| n == STRIP || n == OVERLAY),
                "unexpected windows: {ns:?}"
            );
        };
        only_ours(&r);
        r.reveal();
        only_ours(&r);
        r.up(50);
        only_ours(&r);
        r.down(Surface::Overlay, 306.0, 600.0, 1000);
        r.mv(306.0, 650.0, 1016);
        only_ours(&r);
        r.mv(306.0, 700.0, 1032);
        r.up(1200);
        only_ours(&r);
    }

    #[test]
    fn commit_removes_strip_and_keeps_overlay() {
        let mut r = Rig::new();
        r.reveal();
        let (strip, overlay) = (r.set().strip.unwrap(), r.overlay());
        r.up(50);
        assert_eq!(r.set().strip, None);
        assert!(!r.app.is_live(strip));
        assert_eq!(r.set().overlay, Some(overlay), "the same overlay stays");
        assert_eq!(r.namespaces(), vec![OVERLAY]);
    }

    #[test]
    fn close_removes_everything_but_strip() {
        let mut r = Rig::new();
        r.reveal();
        r.up(50);
        r.down(Surface::Overlay, 306.0, 600.0, 1000);
        r.mv(306.0, 650.0, 1016);
        r.mv(306.0, 700.0, 1032);
        r.up(1200);
        assert_eq!(
            r.namespaces(),
            vec![OVERLAY, STRIP],
            "thrown off the bottom"
        );
        r.finish_exit();
        assert_eq!(r.namespaces(), vec![STRIP]);
        assert_eq!(r.set().overlay, None);
    }

    #[test]
    fn morph_is_expanded_while_dragging_and_bar_when_minimized() {
        let config = NavDockConfig::default();
        let mut r = Rig::new();
        r.reveal();
        assert_eq!(r.morph_width(), config.dock_size.0);
        r.up(50);
        assert_eq!(r.morph_width(), config.bar_size.0);
    }

    #[test]
    fn bar_drag_expands_morph_in_the_same_overlay() {
        let config = NavDockConfig::default();
        let mut r = Rig::new();
        r.reveal();
        r.up(50);
        let (overlay, morph) = (r.overlay(), r.morph());
        r.down(Surface::Bar, 306.0, 1346.0, 1000);
        r.mv(400.0, 1346.0, 1016);
        assert_eq!((r.overlay(), r.morph()), (overlay, morph));
        assert_eq!(r.morph_width(), config.bar_size.0, "not lifted yet");
        r.mv(400.0, 1146.0, 1100);
        assert_eq!(
            r.morph_width(),
            config.dock_size.0,
            "expanded once lifted past rest"
        );
        r.up(1200);
        assert_eq!((r.overlay(), r.morph()), (overlay, morph));
        assert_eq!(r.morph_width(), config.bar_size.0, "back to the bar");
    }

    #[test]
    fn morph_look_change_eases_over_anim() {
        let mut r = Rig::new();
        r.reveal();
        r.up(50);
        let morph = r.morph();
        let shown = r.app.component::<Layout>(morph).unwrap().rect;
        let target = r.app.component::<ComputedLayout>(morph).unwrap().rect;
        assert_ne!(
            shown, target,
            "the bar look is reached over time, not at once"
        );
    }

    #[test]
    fn looks_trade_icons_for_dots_through_layout() {
        // Per slot, the (icon, dot) target sizes.
        let sizes = |r: &Rig| -> Vec<(Size, Size)> {
            let size = |n: NodeId| {
                let rect = r.app.component::<ComputedLayout>(n).unwrap().rect;
                Size::new(rect.width(), rect.height())
            };
            r.app
                .children(r.morph())
                .unwrap()
                .iter()
                .map(|&slot| {
                    let [hl, dot] = r.app.children(slot).unwrap() else {
                        panic!("a slot holds the icon box and the dot");
                    };
                    let icon = r.app.children(*hl).unwrap()[0];
                    (size(icon), size(*dot))
                })
                .collect()
        };
        let mut r = Rig::new();
        r.reveal();
        for (icon, dot) in sizes(&r) {
            assert!(
                icon.width > 0.0 && icon.height > 0.0,
                "icon shown: {icon:?}"
            );
            assert_eq!(dot, Size::new(0.0, 0.0), "dot hidden");
        }
        r.up(50);
        for (icon, dot) in sizes(&r) {
            assert_eq!(icon, Size::new(0.0, 0.0), "icon hidden");
            assert_eq!(dot, Size::new(4.0, 4.0), "dot shown");
        }
    }

    #[test]
    fn morph_lays_out_where_hit_expects_it() {
        let config = NavDockConfig::default();
        let mut r = Rig::new();
        // Up to the dock's rest height, 73 + 26 px.
        r.down(Surface::Strip, 306.0, 1350.0, 0);
        r.mv(306.0, 1251.0, 16);
        // Held still, so the release commits rather than flicks.
        r.mv(306.0, 1251.0, 200);
        let overlay = r.overlay();
        let size = Size::new(612.0, 1324.0);
        r.app.emit(Resized { size }, overlay);
        r.app.tick();
        let target = |r: &Rig| r.app.component::<ComputedLayout>(r.morph()).unwrap().rect;
        assert_eq!(target(&r), Look::Expanded.target_rect(&config, size));
        r.up(210);
        assert_eq!(target(&r), Look::Bar.target_rect(&config, size));
    }

    #[test]
    fn icons_scale_with_their_box_mid_morph() {
        let config = NavDockConfig {
            anim: Duration::from_secs(1),
            ..NavDockConfig::default()
        };
        let mut r = Rig::with(config);
        r.reveal();
        r.up(50);
        // Part way into the 1 s Expanded -> Bar morph.
        std::thread::sleep(Duration::from_millis(100));
        r.app.signal(Frame(r.overlay()));
        r.app.flush();
        let slots = r.app.children(r.morph()).unwrap().to_vec();
        for slot in slots {
            let hl = r.app.children(slot).unwrap()[0];
            let icon = r.app.children(hl).unwrap()[0];
            let shown = r.app.component::<Layout>(icon).unwrap().content();
            let Some(Paint::Monochrome(run)) = r.app.component::<Paint>(icon) else {
                panic!("an icon paints a sprite");
            };
            let sprite = run[0].size;
            assert!(
                (sprite.width - shown.width()).abs() < 0.5
                    && (sprite.height - shown.height()).abs() < 0.5,
                "sprite {sprite:?} follows its box {shown:?}"
            );
            assert!(
                sprite.width > 0.0 && sprite.width < 24.0,
                "mid-morph, not at either end: {sprite:?}"
            );
        }
    }

    #[test]
    fn overlay_has_one_page_per_section() {
        let mut r = Rig::new();
        r.reveal();
        assert_eq!(r.app.children(r.row()).unwrap().len(), 5);
    }

    #[test]
    fn morph_is_drawn_above_the_row() {
        let mut r = Rig::new();
        r.reveal();
        let children = r.app.children(r.overlay()).unwrap();
        assert_eq!(children, &[r.row(), r.morph()], "a later sibling is nearer");
    }

    #[test]
    fn screen_falls_back_before_any_resize() {
        let r = Rig::new();
        assert_eq!(
            r.app.resource::<NavDock>().state.screen(),
            Size::new(612.0, 1360.0)
        );
    }

    #[test]
    fn first_overlay_uses_fallback_screen() {
        let mut r = Rig::new();
        r.reveal();
        let style = r.app.component::<LayoutStyle>(r.row()).unwrap();
        assert_eq!(style.inset.left, percent(-200.0));
        assert_eq!(style.width, percent(500.0));
        assert_eq!(style.height, percent(100.0));
    }

    #[test]
    fn pages_fill_the_window_width() {
        // 700 is wider than the 612 fallback, 500 narrower: the pages follow
        // the window either way.
        for width in [700.0_f32, 500.0] {
            let mut r = Rig::new();
            r.reveal();
            let overlay = r.overlay();
            r.app.emit(
                Resized {
                    size: Size::new(width, 1324.0),
                },
                overlay,
            );
            r.app.tick();
            r.app.tick();
            let cells = r.app.children(r.row()).unwrap().to_vec();
            assert_eq!(cells.len(), 5);
            for cell in cells {
                let w = r
                    .app
                    .component::<ComputedLayout>(cell)
                    .unwrap()
                    .rect
                    .width();
                assert_eq!(w, width);
            }
            let row = r.app.component::<ComputedLayout>(r.row()).unwrap().rect;
            assert_eq!(row.x(), -2.0 * width, "page 2 rests at -2 windows");
            assert_eq!(r.app.resource::<NavDock>().state.screen().width, width);
        }
    }

    #[test]
    fn pages_follow_overlay_size_and_snap() {
        let mut r = Rig::new();
        r.reveal();
        let overlay = r.overlay();
        r.app.emit(
            Resized {
                size: Size::new(700.0, 1324.0),
            },
            overlay,
        );
        r.app.tick();
        assert_eq!(
            r.app.resource::<NavDock>().state.screen(),
            Size::new(700.0, 1360.0),
            "the shorter overlay does not shrink the screen"
        );
        let row = r.row();
        let style = r.app.component::<LayoutStyle>(row).unwrap();
        assert_eq!(style.inset.left, percent(-200.0), "default page 2");
        assert_eq!(style.width, percent(500.0));
        let shown = r.app.component::<Layout>(row).unwrap().rect;
        let target = r.app.component::<ComputedLayout>(row).unwrap().rect;
        assert_eq!(shown, target, "a resize snaps, it does not slide");
        let cell = r.app.children(row).unwrap()[0];
        let cell = r.app.component::<LayoutStyle>(cell).unwrap();
        assert_eq!((cell.width, cell.height), (percent(20.0), percent(100.0)));
    }

    #[test]
    fn respawned_overlay_starts_at_overlay_height() {
        let mut r = Rig::new();
        r.reveal();
        r.up(50);
        let overlay = r.overlay();
        r.app.emit(
            Resized {
                size: Size::new(612.0, 1324.0),
            },
            overlay,
        );
        r.app.tick();
        r.down(Surface::Overlay, 306.0, 600.0, 1000);
        r.mv(306.0, 650.0, 1016);
        r.mv(306.0, 700.0, 1032);
        r.up(1200);
        r.finish_exit();
        assert_eq!(r.set().overlay, None);
        r.down(Surface::Strip, 306.0, 1350.0, 2000);
        r.mv(306.0, 1340.0, 2016);
        let cell = r.app.children(r.row()).unwrap()[0];
        let cell = r.app.component::<LayoutStyle>(cell).unwrap();
        assert_eq!((cell.width, cell.height), (percent(20.0), percent(100.0)));
    }

    impl Rig {
        /// Commit to the bar, then a fast bar throw up and release:
        /// Hidden, exiting up.
        fn flick(&mut self) {
            self.reveal();
            self.up(50);
            self.down(Surface::Bar, 306.0, 1346.0, 1000);
            let mut y = 1346.0;
            let mut t = 1000;
            for _ in 0..10 {
                y -= 12.0;
                t += 8;
                self.mv(306.0, y, t);
            }
            self.up(t + 4);
            let view = self.app.resource::<NavDock>().state.view();
            assert_eq!(view.exit, Some(crate::state::Exit::Up), "flicked");
        }

        /// Snaps `nodes` to their targets.
        fn settle(&mut self, nodes: &[NodeId]) {
            for &node in nodes {
                *self
                    .app
                    .component_mut::<LayoutAnimationSettings>(node)
                    .unwrap() = eased_layout(Duration::ZERO);
            }
            self.app.tick();
            for &node in nodes {
                let shown = self.app.component::<Layout>(node).unwrap().rect;
                let target = self.app.component::<ComputedLayout>(node).unwrap().rect;
                assert_eq!(shown, target, "settled");
            }
        }

        fn frame(&mut self) {
            self.app.signal(Frame(self.overlay()));
            self.app.flush();
        }

        /// Ends the exit motion: the row and the morph snap to their
        /// targets, and a frame is signalled for the overlay.
        fn finish_exit(&mut self) {
            let (row, morph) = (self.row(), self.morph());
            self.settle(&[row, morph]);
            self.frame();
        }

        fn settings_of(&self, node: NodeId) -> String {
            format!(
                "{:?}",
                self.app.component::<LayoutAnimationSettings>(node).unwrap()
            )
        }
    }

    #[test]
    fn overlay_outlives_hidden_until_exit_finishes() {
        let mut r = Rig::new();
        r.flick();
        let overlay = r.overlay();
        assert!(r.app.is_live(overlay), "the overlay stays for the exit");
        let mut names = r.namespaces();
        names.sort();
        assert_eq!(names, vec![OVERLAY, STRIP]);
        let row = r.row();
        let target = r.app.component::<ComputedLayout>(row).unwrap().rect;
        assert_eq!(target.y(), -1360.0, "the row is thrown off the top");
        // Mid-throw, a frame does not end it.
        r.app.signal(Frame(overlay));
        r.app.flush();
        assert!(r.app.is_live(overlay), "still flying");
        r.finish_exit();
        assert!(!r.app.is_live(overlay));
        assert_eq!(r.set().overlay, None);
        assert_eq!(r.namespaces(), vec![STRIP]);
    }

    #[test]
    fn reveal_during_exit_reuses_overlay() {
        let mut r = Rig::new();
        r.flick();
        let overlay = r.overlay();
        r.down(Surface::Strip, 306.0, 1350.0, 2000);
        r.mv(306.0, 1340.0, 2016);
        assert_eq!(r.overlay(), overlay, "the same overlay");
        let overlays = r.namespaces().iter().filter(|n| *n == OVERLAY).count();
        assert_eq!(overlays, 1);
        let target = r.app.component::<ComputedLayout>(r.row()).unwrap().rect;
        assert_eq!(target.y(), 0.0, "pulled back from the top: {target:?}");
    }

    #[test]
    fn exit_waits_for_the_morph() {
        let mut r = Rig::new();
        r.reveal();
        let overlay = r.overlay();
        let size = Size::new(612.0, 1324.0);
        r.app.emit(Resized { size }, overlay);
        r.app.tick();
        r.frame();
        r.app.tick();
        r.handle(DockEvent::Cancel);
        let row = r.row();
        let config = NavDockConfig::default();
        let pill = r.app.component::<ComputedLayout>(r.morph()).unwrap().rect;
        assert!(
            pill.y() >= size.height,
            "the expanded pill leaves fully below the edge: {pill:?}"
        );
        assert_eq!(pill.height(), config.dock_size.1);
        r.settle(&[row]);
        r.frame();
        assert!(r.app.is_live(overlay), "the morph is still moving");
        r.finish_exit();
        assert!(!r.app.is_live(overlay));
        assert_eq!(r.namespaces(), vec![STRIP]);
    }

    #[test]
    fn reveal_slides_the_row_up_from_below() {
        let mut r = Rig::new();
        r.reveal();
        r.app.tick();
        r.frame();
        r.app.tick();
        let height = r.app.resource::<NavDock>().overlay_size().height;
        let row = r.row();
        let shown = r.app.component::<Layout>(row).unwrap().rect;
        // Below the screen, a frame into the slide up at most.
        assert!(
            shown.y() > height - 20.0,
            "first resolved layout is below the screen: {}",
            shown.y()
        );
        let target = r.app.component::<ComputedLayout>(row).unwrap().rect;
        assert_eq!(target.y(), 0.0);
        let anim = format!("{:?}", eased_layout(NavDockConfig::default().anim));
        assert_eq!(r.settings_of(row), anim);
    }

    #[test]
    fn hover_change_moves_the_row_on_anim() {
        let mut r = Rig::new();
        r.reveal();
        let x = r.app.resource::<NavDock>().state.icon_center_x(0);
        r.mv(x, 1290.0, 230);
        assert_eq!(
            r.app.resource::<NavDock>().state.view().mode,
            Mode::Dragging { hover: 0 }
        );
        let anim = format!("{:?}", eased_layout(NavDockConfig::default().anim));
        assert_eq!(r.settings_of(r.row()), anim);
        let target = r.app.component::<ComputedLayout>(r.row()).unwrap().rect;
        assert_eq!((target.x(), target.y()), (0.0, 0.0));
    }

    #[test]
    fn untouched_transitions_ease_over_anim() {
        let anim = format!("{:?}", eased_layout(NavDockConfig::default().anim));
        let mut r = Rig::new();
        r.reveal();
        r.up(50);
        assert_eq!(r.settings_of(r.morph()), anim, "the commit morph");
        r.down(Surface::Bar, 306.0, 1346.0, 1000);
        r.up(1050);
        assert_eq!(
            r.app.resource::<NavDock>().state.view().mode,
            Mode::Idle { active: 2 }
        );
        assert_eq!(r.settings_of(r.morph()), anim, "a bar tap");
    }

    #[test]
    fn close_release_throws_the_row() {
        let mut r = Rig::new();
        r.reveal();
        r.up(50);
        r.down(Surface::Overlay, 306.0, 600.0, 1000);
        r.mv(306.0, 650.0, 1016);
        r.mv(306.0, 700.0, 1032);
        r.up(1040);
        let row = r.settings_of(r.row());
        assert!(row.contains("Speed("), "thrown: {row}");
    }

    impl Rig {
        /// From the committed bar, a bar drag up 300 px; `flick` ends it fast.
        fn bar_lift(&mut self, flick: bool) {
            self.reveal();
            self.up(50);
            self.down(Surface::Bar, 306.0, 1346.0, 1000);
            let mut y = 1346.0;
            let mut t = 1000;
            for _ in 0..10 {
                y -= 30.0;
                t += if flick { 8 } else { 200 };
                self.mv(306.0, y, t);
            }
            self.up(t + if flick { 4 } else { 600 });
        }
    }

    #[test]
    fn bar_drag_commit_throws_morph_back_to_bar() {
        let config = NavDockConfig::default();
        let mut r = Rig::new();
        r.bar_lift(false);
        let view = r.app.resource::<NavDock>().state.view();
        assert_eq!(view.exit, None);
        assert_eq!(r.morph_widget().look(), (0.0, 2));
        let size = r.app.resource::<NavDock>().laid_out_size(&r.app);
        let target = r.app.component::<ComputedLayout>(r.morph()).unwrap().rect;
        assert_eq!(target, Look::Bar.target_rect(&config, size));
        let morph = r.settings_of(r.morph());
        assert!(morph.contains("Speed("), "thrown: {morph}");
        let row = r.app.component::<ComputedLayout>(r.row()).unwrap().rect;
        assert_eq!(row.y(), 0.0, "the page stays put");
    }

    #[test]
    fn bar_drag_moves_the_morph_not_the_page() {
        let mut r = Rig::new();
        r.reveal();
        r.up(50);
        r.down(Surface::Bar, 306.0, 1346.0, 1000);
        r.mv(306.0, 1306.0, 1200);
        let row = r.app.component::<ComputedLayout>(r.row()).unwrap().rect;
        assert_eq!(row.y(), 0.0, "the page stays put");
        let zero = format!("{:?}", eased_layout(Duration::ZERO));
        assert_eq!(r.settings_of(r.morph()), zero);
        let morph = r.morph();
        let shown = r.app.component::<Layout>(morph).unwrap().rect;
        let target = r.app.component::<ComputedLayout>(morph).unwrap().rect;
        assert_eq!(shown, target, "the bar follows the finger exactly");
        let (t, _) = r.morph_widget().look();
        assert!(t > 0.0 && t < 1.0, "part way to the dock: {t}");
    }

    #[test]
    fn bar_flick_throws_morph_off_top() {
        let mut r = Rig::new();
        r.bar_lift(true);
        let view = r.app.resource::<NavDock>().state.view();
        assert_eq!(view.exit, Some(crate::state::Exit::Up));
        let target = r.app.component::<ComputedLayout>(r.morph()).unwrap().rect;
        assert!(target.bottom() <= 0.0, "off the top: {target:?}");
        let morph = r.settings_of(r.morph());
        assert!(morph.contains("Speed("), "thrown: {morph}");
        let row = r.settings_of(r.row());
        assert!(row.contains("Speed("), "thrown: {row}");
        let height = r.app.resource::<NavDock>().laid_out_size(&r.app).height;
        let row = r.app.component::<ComputedLayout>(r.row()).unwrap().rect;
        assert_eq!(row.y(), -height, "the row exits as ever");
    }

    #[test]
    fn strip_drag_moves_row_and_morph_unanimated() {
        let mut r = Rig::new();
        r.down(Surface::Strip, 306.0, 1350.0, 0);
        r.mv(306.0, 1340.0, 16);
        // The overlay slides up from below once its first layout resolved.
        r.app.tick();
        r.frame();
        r.mv(306.0, 1200.0, 32);
        r.app.tick();
        let morph = r.morph();
        let shown = r.app.component::<Layout>(morph).unwrap().rect;
        let target = r.app.component::<ComputedLayout>(morph).unwrap().rect;
        assert_eq!(shown, target, "the dock follows the finger exactly");
        let row = r.app.component::<ComputedLayout>(r.row()).unwrap().rect;
        assert_eq!(row.y(), 0.0, "the row targets fully up on its own");
    }

    #[test]
    fn morph_shows_hover_and_active() {
        let mut r = Rig::new();
        r.reveal();
        assert_eq!(r.morph_widget().look(), (1.0, 2));
        r.up(50);
        assert_eq!(r.morph_widget().look(), (0.0, 2));
    }
}
