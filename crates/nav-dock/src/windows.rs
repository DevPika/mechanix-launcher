//! The dock's windows and the reconciler that keeps them in step with
//! `NavState::view()`.
//!
//! A window never resizes (see the crate's constraints). There are two: the
//! swipe strip while Hidden, and the overlay, which holds the page row and
//! the dock/bar morph, in every other mode. Each is spawned when its mode
//! needs it and removed when it does not, except the window holding the
//! live touch, which a touch stays bound to; it goes on the first reconcile
//! after that touch ends.

use std::time::Duration;

use mecha_wayland::prelude::*;
use nav_dock_api::Section;

use crate::dock::{Look, MorphUpdate, ScaleIcons};
use crate::overlay::{OverlayUpdate, parts, spawn_overlay};
use crate::state::{NavState, Surface, View};
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
}
impl Resource for NavDock {}

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
/// no longer wants (unless it holds the live touch), spawns what is missing,
/// and pushes paging, look and emphasis into the widgets.
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
        } else if !wanted(view, kind) && held != Some(kind) {
            app.remove(window);
            *slot = None;
        }
    }
    for kind in Kind::ALL {
        if wanted(view, kind) && set.slot(kind).is_none() {
            *set.slot(kind) = Some(spawn(app, kind, view));
        }
    }
    app.resource_mut::<NavDock>().windows = set;

    if let Some((row, morph)) = set.overlay.and_then(|o| parts(app, o)) {
        if let Some(o) = view.overlay {
            let dragging = view.held_surface == Some(Surface::Overlay);
            app.emit(OverlayUpdate { view: o, dragging }, row);
        }
        if let Some((look, focus)) = Look::of(view.mode) {
            app.emit(MorphUpdate { look, focus }, morph);
        }
    }
}

/// The window table: the strip while Hidden, the overlay otherwise.
fn wanted(view: View, kind: Kind) -> bool {
    match kind {
        Kind::Strip => view.strip,
        Kind::Overlay => view.overlay.is_some(),
    }
}

fn spawn(app: &mut App, kind: Kind, view: View) -> NodeId {
    let (config, icons, screen, overlay_height) = {
        let nav = app.resource::<NavDock>();
        (
            nav.state.config().clone(),
            nav.icons.clone(),
            nav.state.screen(),
            nav.overlay_height,
        )
    };
    match kind {
        Kind::Strip => spawn_strip(app, &config),
        Kind::Overlay => {
            let overlay = view.overlay.expect("overlay is only wanted with a view");
            let look = Look::of(view.mode).expect("overlay is only wanted while shown");
            // `spawn_page` needs the whole app, so the sections step out
            // of the resource while their pages are built.
            let sections = std::mem::take(&mut app.resource_mut::<NavDock>().sections);
            // Start at the last overlay height, so the first frame does not
            // jump from the full screen height to the overlay's own.
            let size = Size::new(screen.width, overlay_height.unwrap_or(screen.height));
            let window = spawn_overlay(app, &config, &sections, &icons, size, overlay, look);
            app.resource_mut::<NavDock>().sections = sections;
            window
        }
    }
}

/// Every window's settled size feeds the state's screen (icon centres,
/// paging and window origins). Windows other than home are smaller than the
/// screen, so the screen only grows, to the component-wise max of all of
/// them. The overlay's height is also kept for its own origin.
pub(crate) fn on_resized(app: &mut App, e: &Emitted<Resized>) {
    let mut nav = app.resource_mut::<NavDock>();
    let old = nav.state.screen();
    let size = Size::new(
        old.width.max(e.event.size.width),
        old.height.max(e.event.size.height),
    );
    nav.state.set_screen(size);
    if nav.windows.overlay.is_some_and(|w| e.targets.contains(&w)) {
        nav.overlay_height = Some(e.event.size.height);
    }
}

/// On each overlay frame, after layout animation has advanced, the morph's
/// icons are redrawn at their displayed size. The frame drawn in the same
/// signal may still show the previous size: about one frame of lag.
pub(crate) fn on_frame(app: &mut App, frame: &Frame) {
    let overlay = app.resource::<NavDock>().windows.overlay;
    if overlay != Some(frame.0) {
        return;
    }
    if let Some((_, morph)) = parts(app, frame.0) {
        app.emit(ScaleIcons, morph);
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
        assert_eq!(
            r.morph_width(),
            config.dock_size.0,
            "expanded while dragging"
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
        r.reveal();
        let overlay = r.overlay();
        let size = Size::new(612.0, 1324.0);
        r.app.emit(Resized { size }, overlay);
        r.app.tick();
        let target = |r: &Rig| r.app.component::<ComputedLayout>(r.morph()).unwrap().rect;
        assert_eq!(target(&r), Look::Expanded.target_rect(&config, size));
        r.up(50);
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
        assert_eq!(style.inset.left, px(-2.0 * 612.0));
        assert_eq!(style.width, px(5.0 * 612.0));
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
        assert_eq!(style.inset.left, px(-2.0 * 700.0), "default page 2");
        assert_eq!(style.width, px(5.0 * 700.0));
        let shown = r.app.component::<Layout>(row).unwrap().rect;
        let target = r.app.component::<ComputedLayout>(row).unwrap().rect;
        assert_eq!(shown, target, "a resize snaps, it does not slide");
        let cell = r.app.children(row).unwrap()[0];
        let cell = r.app.component::<LayoutStyle>(cell).unwrap();
        assert_eq!((cell.width, cell.height), (px(700.0), px(1324.0)));
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
        assert_eq!(r.set().overlay, None);
        r.down(Surface::Strip, 306.0, 1350.0, 2000);
        r.mv(306.0, 1340.0, 2016);
        let cell = r.app.children(r.row()).unwrap()[0];
        let cell = r.app.component::<LayoutStyle>(cell).unwrap();
        assert_eq!((cell.width, cell.height), (px(612.0), px(1324.0)));
    }

    #[test]
    fn morph_shows_hover_and_active() {
        let mut r = Rig::new();
        r.reveal();
        assert_eq!(r.morph_widget().look(), (Look::Expanded, 2));
        r.up(50);
        assert_eq!(r.morph_widget().look(), (Look::Bar, 2));
    }
}
