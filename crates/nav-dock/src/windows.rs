//! The dock's windows and the reconciler that keeps them in step with
//! `NavState::view()`.
//!
//! A window never resizes (see the crate's constraints): each surface has
//! its own window, spawned when its mode needs it and removed when it does
//! not, except the window holding the live touch, which a touch stays bound
//! to; it goes on the first reconcile after that touch ends.

use std::time::Duration;

use mecha_wayland::prelude::*;
use nav_dock_api::Section;

use crate::dock::{BarActive, DockFocus, spawn_bar, spawn_dock};
use crate::overlay::{OverlayUpdate, spawn_overlay};
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
}
impl Resource for NavDock {}

/// The live dock windows, one slot per surface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct WindowSet {
    pub(crate) strip: Option<NodeId>,
    pub(crate) bar: Option<NodeId>,
    pub(crate) dock: Option<NodeId>,
    pub(crate) overlay: Option<NodeId>,
}

impl WindowSet {
    fn slot(&mut self, surface: Surface) -> &mut Option<NodeId> {
        match surface {
            Surface::Strip => &mut self.strip,
            Surface::Bar => &mut self.bar,
            Surface::Dock => &mut self.dock,
            Surface::Overlay => &mut self.overlay,
        }
    }
}

/// Which dock surface `window` is, if it is one.
pub(crate) fn surface_of(nav: &NavDock, window: NodeId) -> Option<Surface> {
    let w = nav.windows;
    [
        (w.strip, Surface::Strip),
        (w.bar, Surface::Bar),
        (w.dock, Surface::Dock),
        (w.overlay, Surface::Overlay),
    ]
    .into_iter()
    .find_map(|(id, surface)| (id == Some(window)).then_some(surface))
}

/// Brings the windows in line with the state's view: removes what the mode
/// no longer wants (unless it holds the live touch), spawns what is missing,
/// and pushes hover, active and paging into the widgets.
pub(crate) fn reconcile(app: &mut App) {
    const ALL: [Surface; 4] = [
        Surface::Strip,
        Surface::Bar,
        Surface::Dock,
        Surface::Overlay,
    ];
    let (view, mut set) = {
        let nav = app.resource::<NavDock>();
        (nav.state.view(), nav.windows)
    };

    for surface in ALL {
        let slot = set.slot(surface);
        let Some(window) = *slot else { continue };
        if !app.is_live(window) {
            *slot = None;
        } else if !wanted(view, surface) && view.held_surface != Some(surface) {
            app.remove(window);
            *slot = None;
        }
    }
    for surface in ALL {
        if wanted(view, surface) && set.slot(surface).is_none() {
            *set.slot(surface) = Some(spawn(app, surface, view));
        }
    }
    app.resource_mut::<NavDock>().windows = set;

    if let (Some(dock), Some(i)) = (set.dock, emphasis(view.mode)) {
        emit_at_content(app, dock, DockFocus(i));
    }
    if let (Some(bar), Mode::Minimized { active }) = (set.bar, view.mode) {
        emit_at_content(app, bar, BarActive(active));
    }
    if let (Some(overlay), Some(o)) = (set.overlay, view.overlay) {
        let dragging = view.held_surface == Some(Surface::Overlay);
        emit_at_content(app, overlay, OverlayUpdate { view: o, dragging });
    }
}

/// The window table: what each mode puts on screen.
fn wanted(view: View, surface: Surface) -> bool {
    match surface {
        Surface::Strip => view.strip,
        Surface::Dock => matches!(view.mode, Mode::Dragging { .. } | Mode::Idle { .. }),
        Surface::Bar => matches!(view.mode, Mode::Minimized { .. }),
        Surface::Overlay => view.overlay.is_some(),
    }
}

/// The dock icon to emphasise: the hovered one while dragging, the active
/// one while idle.
fn emphasis(mode: Mode) -> Option<usize> {
    match mode {
        Mode::Dragging { hover } => Some(hover),
        Mode::Idle { active } => Some(active),
        Mode::Hidden | Mode::Minimized { .. } => None,
    }
}

fn spawn(app: &mut App, surface: Surface, view: View) -> NodeId {
    let (config, icons, screen) = {
        let nav = app.resource::<NavDock>();
        (
            nav.state.config().clone(),
            nav.icons.clone(),
            nav.state.screen(),
        )
    };
    match surface {
        Surface::Strip => spawn_strip(app, &config),
        Surface::Dock => spawn_dock(app, &config, &icons, emphasis(view.mode).unwrap_or(0)),
        Surface::Bar => {
            let active = match view.mode {
                Mode::Minimized { active } => active,
                _ => 0,
            };
            spawn_bar(app, &config, icons.len(), active)
        }
        Surface::Overlay => {
            let overlay = view.overlay.expect("overlay is only wanted with a view");
            // `spawn_page` needs the whole app, so the sections step out
            // of the resource while their pages are built.
            let sections = std::mem::take(&mut app.resource_mut::<NavDock>().sections);
            let window = spawn_overlay(app, &config, &sections, screen, overlay);
            app.resource_mut::<NavDock>().sections = sections;
            window
        }
    }
}

/// Every dock window holds one content widget, its first child.
fn emit_at_content<E: Event>(app: &mut App, window: NodeId, event: E) {
    if let Some(&content) = app.children(window).and_then(|c| c.first()) {
        app.emit(event, content);
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

/// An Overlay-layer role anchored to the bottom edge only, so the
/// compositor centres the surface horizontally. The strip, dock and bar.
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
    use crate::module::NavDockModule;
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

        let mut module = NavDockModule::new(NavDockConfig::default());
        for id in IDS {
            module = module.section(Fake(id));
        }
        app.add_module(module);
        app.tick();
        app
    }

    impl Rig {
        fn new() -> Self {
            Self {
                app: dock_app(),
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

        /// Every live window's surface, in spawn order.
        fn surfaces(&self) -> Vec<Surface> {
            let nav = self.app.resource::<NavDock>();
            self.app
                .resource::<Windows>()
                .iter()
                .map(|w| surface_of(nav, w).expect("only dock windows exist"))
                .collect()
        }

        fn content(&self, window: NodeId) -> NodeId {
            self.app.children(window).expect("window is live")[0]
        }
    }

    #[test]
    fn hidden_spawns_only_strip() {
        let r = Rig::new();
        assert_eq!(r.app.resource::<Windows>().len(), 1);
        assert_eq!(r.surfaces(), vec![Surface::Strip]);
    }

    #[test]
    fn reveal_spawns_dock_and_overlay_keeps_strip_while_held() {
        let mut r = Rig::new();
        r.reveal();
        let mut surfaces = r.surfaces();
        surfaces.sort_by_key(|s| *s as u8);
        assert_eq!(
            surfaces,
            vec![Surface::Strip, Surface::Dock, Surface::Overlay]
        );
    }

    #[test]
    fn commit_removes_strip_and_dock_spawns_bar() {
        let mut r = Rig::new();
        r.reveal();
        let (strip, dock) = (r.set().strip.unwrap(), r.set().dock.unwrap());
        r.up(50);
        let set = r.set();
        assert!(set.bar.is_some() && set.overlay.is_some());
        assert_eq!((set.strip, set.dock), (None, None));
        assert!(!r.app.is_live(strip) && !r.app.is_live(dock));
        assert_eq!(r.app.resource::<Windows>().len(), 2);
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
        assert_eq!(r.surfaces(), vec![Surface::Strip]);
        assert_eq!(r.app.resource::<Windows>().len(), 1);
    }

    #[test]
    fn bar_drag_keeps_bar_while_held() {
        let mut r = Rig::new();
        r.reveal();
        r.up(50);
        let bar = r.set().bar.unwrap();
        r.down(Surface::Bar, 306.0, 1346.0, 1000);
        r.mv(400.0, 1346.0, 1016);
        assert_eq!(r.set().bar, Some(bar), "held bar survives Dragging");
        assert!(r.set().dock.is_some());
        r.up(1200);
        assert_eq!(r.set().bar, Some(bar));
        assert_eq!(r.set().dock, None);
    }

    #[test]
    fn overlay_has_one_page_per_section() {
        let mut r = Rig::new();
        r.reveal();
        let row = r.content(r.set().overlay.unwrap());
        assert_eq!(r.app.children(row).unwrap().len(), 5);
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
        let row = r.content(r.set().overlay.unwrap());
        let style = r.app.component::<LayoutStyle>(row).unwrap();
        assert_eq!(style.inset.left, px(-2.0 * 612.0));
        assert_eq!(style.width, px(5.0 * 612.0));
    }

    #[test]
    fn pages_follow_overlay_size_and_snap() {
        let mut r = Rig::new();
        r.reveal();
        let overlay = r.set().overlay.unwrap();
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
        let row = r.content(overlay);
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
    fn dock_and_bar_show_hover_and_active() {
        let mut r = Rig::new();
        r.reveal();
        let dock = r.content(r.set().dock.unwrap());
        assert_eq!(
            r.app
                .widget::<crate::dock::DockRow>(dock)
                .unwrap()
                .emphasis(),
            2
        );
        r.up(50);
        let bar = r.content(r.set().bar.unwrap());
        assert_eq!(r.app.widget::<crate::dock::Bar>(bar).unwrap().active(), 2);
    }
}
