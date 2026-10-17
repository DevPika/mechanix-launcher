//! Raw contact input into the state machine: each `ContactInput` on a dock
//! window becomes a screen-space `DockEvent`, then the windows reconcile.

use std::time::Instant;

use mecha_wayland::prelude::*;

use crate::dock::Look;
use crate::state::{DockEvent, Surface};
use crate::windows::{NavDock, reconcile};

/// The state machine's contact id for the mouse pointer, so the dock can
/// be driven from a desktop.
const MOUSE_CONTACT: u32 = u32::MAX;

/// Routes one contact report into `NavState`. `Pressed` and `Moved` need a
/// live dock window to place them on screen; a `Moved` only counts while
/// its contact is pressed (a touch's first `Moved` comes just before its
/// `Pressed`, and the mouse hovers). `Released` and `Cancelled` end a
/// contact pressed here even if its window is gone, so no gesture is left
/// hanging.
pub(crate) fn on_contact(app: &mut App, c: &ContactInput) {
    let contact = match c.contact {
        ContactId::Mouse => MOUSE_CONTACT,
        ContactId::Touch(id) => id,
    };
    let t = Instant::now();
    let live = app.is_live(c.window);
    let event = {
        let mut nav = app.resource_mut::<NavDock>();
        let pressed = nav.pressed.contains(&contact);
        match c.phase {
            ContactPhase::Pressed | ContactPhase::Moved => {
                let Some(surface) = hit(&nav, c.window, c.position).filter(|_| live) else {
                    return;
                };
                let pos = origin(&nav, surface) + c.position;
                if c.phase == ContactPhase::Moved {
                    if !pressed {
                        return;
                    }
                    DockEvent::Move { contact, pos, t }
                } else {
                    if !pressed {
                        nav.pressed.push(contact);
                    }
                    DockEvent::Down {
                        contact,
                        surface,
                        pos,
                        t,
                    }
                }
            }
            ContactPhase::Released | ContactPhase::Cancelled => {
                if !pressed {
                    return;
                }
                nav.pressed.retain(|&p| p != contact);
                if c.phase == ContactPhase::Released {
                    DockEvent::Up { contact, t }
                } else {
                    DockEvent::Cancel
                }
            }
        }
    };
    app.resource_mut::<NavDock>().state.handle(event);
    reconcile(app);
}

/// What a touch at `pos` (in `window`'s coordinates) went down on, if
/// `window` is a dock window. In the overlay it is the dock or the bar when
/// it lands on the morph, the page otherwise. The morph's rect is where its
/// look settles, not where it is drawn mid-morph, so a touch classifies the
/// same however far the animation has got.
pub(crate) fn hit(nav: &NavDock, window: NodeId, pos: Point) -> Option<Surface> {
    let w = nav.windows;
    if w.strip == Some(window) {
        return Some(Surface::Strip);
    }
    if w.overlay != Some(window) {
        return None;
    }
    let config = nav.state.config();
    let size = Size::new(nav.state.screen().width, overlay_height(nav));
    let surface = match Look::of(nav.state.view().mode) {
        Some((Look::Expanded, _)) => Look::Expanded
            .target_rect(config, size)
            .contains(pos)
            .then_some(Surface::Dock),
        Some((Look::Bar, _)) => {
            let bar = Look::Bar.target_rect(config, size);
            let gap = Look::Bar.gap(config);
            Rect::new(bar.x(), bar.y(), bar.width(), bar.height() + gap)
                .contains(pos)
                .then_some(Surface::Bar)
        }
        None => None,
    };
    Some(surface.unwrap_or(Surface::Overlay))
}

/// The overlay's height: its last `Resized`, the screen's until then.
fn overlay_height(nav: &NavDock) -> f32 {
    nav.overlay_height.unwrap_or(nav.state.screen().height)
}

/// Screen position of the top-left corner of the window a touch on
/// `surface` lands in. The strip sits bottom-centre at `strip_size`; the
/// overlay, which holds the dock and the bar, spans the width and ends at
/// the bottom edge, its top pushed down by the status bar's exclusive zone.
fn origin(nav: &NavDock, surface: Surface) -> Point {
    let screen = nav.state.screen();
    match surface {
        Surface::Strip => {
            let (width, height) = nav.state.config().strip_size;
            Point::new(screen.width / 2.0 - width / 2.0, screen.height - height)
        }
        Surface::Bar | Surface::Dock | Surface::Overlay => {
            Point::new(0.0, screen.height - overlay_height(nav))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Mode;
    use crate::windows::tests::dock_app;

    fn contact(app: &mut App, window: NodeId, id: ContactId, phase: ContactPhase, x: f32, y: f32) {
        app.signal(ContactInput {
            window,
            contact: id,
            phase,
            position: Point::new(x, y),
        });
        app.flush();
    }

    /// A touch going down the way `wl_touch.down` arrives: `Moved`, then
    /// `Pressed`, at the same position.
    fn touch_down(app: &mut App, window: NodeId, x: f32, y: f32) {
        contact(app, window, ContactId::Touch(0), ContactPhase::Moved, x, y);
        contact(
            app,
            window,
            ContactId::Touch(0),
            ContactPhase::Pressed,
            x,
            y,
        );
    }

    fn mode(app: &App) -> Mode {
        app.resource::<NavDock>().state.view().mode
    }

    fn strip(app: &App) -> NodeId {
        app.resource::<NavDock>()
            .windows
            .strip
            .expect("strip is up")
    }

    #[test]
    fn strip_press_and_move_reveals() {
        let mut app = dock_app();
        let strip = strip(&app);
        touch_down(&mut app, strip, 60.0, 10.0);
        contact(
            &mut app,
            strip,
            ContactId::Touch(0),
            ContactPhase::Moved,
            60.0,
            -10.0,
        );
        assert!(matches!(mode(&app), Mode::Dragging { .. }));
        let overlay = app.resource::<NavDock>().windows.overlay;
        assert!(
            overlay.is_some_and(|o| app.is_live(o)),
            "the overlay window exists"
        );
    }

    #[test]
    fn contact_on_unknown_window_is_ignored() {
        let mut app = dock_app();
        let root = app.root();
        touch_down(&mut app, root, 60.0, 10.0);
        contact(
            &mut app,
            root,
            ContactId::Touch(0),
            ContactPhase::Moved,
            60.0,
            -10.0,
        );
        assert_eq!(mode(&app), Mode::Hidden, "a live non-dock node");

        let strip = strip(&app);
        app.remove(strip);
        touch_down(&mut app, strip, 60.0, 10.0);
        contact(
            &mut app,
            strip,
            ContactId::Touch(0),
            ContactPhase::Moved,
            60.0,
            -10.0,
        );
        assert_eq!(mode(&app), Mode::Hidden, "a removed dock window");
    }

    #[test]
    fn moves_count_only_while_pressed() {
        let mut app = dock_app();
        let strip = strip(&app);
        let mouse = |app: &mut App, phase, y| contact(app, strip, ContactId::Mouse, phase, 60.0, y);
        mouse(&mut app, ContactPhase::Moved, 10.0);
        mouse(&mut app, ContactPhase::Moved, -10.0);
        assert_eq!(mode(&app), Mode::Hidden, "hover does not reveal");

        mouse(&mut app, ContactPhase::Pressed, 10.0);
        mouse(&mut app, ContactPhase::Moved, -10.0);
        assert!(matches!(mode(&app), Mode::Dragging { .. }));
        mouse(&mut app, ContactPhase::Released, -10.0);
        let nav = app.resource::<NavDock>();
        assert_eq!(
            nav.state.view().held_surface,
            None,
            "release ends the gesture"
        );
        assert!(nav.pressed.is_empty());
        // Real-time samples span less than `velocity_min_span`, so that
        // release is no flick: it commits, and the overlay is up.
        let overlay = nav.windows.overlay.expect("overlay is up");
        let after = mode(&app);
        assert!(matches!(after, Mode::Minimized { .. }));

        contact(
            &mut app,
            overlay,
            ContactId::Mouse,
            ContactPhase::Moved,
            60.0,
            -100.0,
        );
        assert_eq!(mode(&app), after, "moves after release do nothing");
    }

    #[test]
    fn unpressed_cancel_leaves_a_touch_alone() {
        let mut app = dock_app();
        let strip = strip(&app);
        touch_down(&mut app, strip, 60.0, 10.0);
        contact(
            &mut app,
            strip,
            ContactId::Mouse,
            ContactPhase::Cancelled,
            0.0,
            0.0,
        );
        contact(
            &mut app,
            strip,
            ContactId::Touch(0),
            ContactPhase::Moved,
            60.0,
            -10.0,
        );
        assert!(matches!(mode(&app), Mode::Dragging { .. }));
    }

    #[test]
    fn origins_are_bottom_centre_and_below_the_bar() {
        let mut app = dock_app();
        let mut nav = app.resource_mut::<NavDock>();
        assert_eq!(origin(&nav, Surface::Strip), Point::new(246.0, 1340.0));
        for surface in [Surface::Overlay, Surface::Dock, Surface::Bar] {
            assert_eq!(origin(&nav, surface), Point::ZERO, "height unknown");
        }
        nav.overlay_height = Some(1324.0);
        for surface in [Surface::Overlay, Surface::Dock, Surface::Bar] {
            assert_eq!(
                origin(&nav, surface),
                Point::new(0.0, 36.0),
                "dock and bar live in the overlay"
            );
        }
    }

    /// Strip press, 20 px up, release: Minimized on the default section.
    fn commit(app: &mut App) -> NodeId {
        let strip = strip(app);
        touch_down(app, strip, 60.0, 10.0);
        contact(
            app,
            strip,
            ContactId::Touch(0),
            ContactPhase::Moved,
            60.0,
            -10.0,
        );
        contact(
            app,
            strip,
            ContactId::Touch(0),
            ContactPhase::Released,
            60.0,
            -10.0,
        );
        assert!(matches!(mode(app), Mode::Minimized { .. }));
        app.resource::<NavDock>()
            .windows
            .overlay
            .expect("overlay is up")
    }

    #[test]
    fn bar_hit_uses_target_rect() {
        let mut app = dock_app();
        let overlay = commit(&mut app);
        let nav = app.resource::<NavDock>();
        // Overlay height unknown: the overlay is the screen, 612 x 1360.
        // The bar is 160 x 12 with 8 below it, so it centres at y = 1346.
        assert_eq!(
            hit(nav, overlay, Point::new(306.0, 1346.0)),
            Some(Surface::Bar)
        );
        assert_eq!(
            hit(nav, overlay, Point::new(306.0, 1358.0)),
            Some(Surface::Bar),
            "the gap below the bar counts"
        );
        assert_eq!(
            hit(nav, overlay, Point::new(306.0, 600.0)),
            Some(Surface::Overlay)
        );
        assert_eq!(
            hit(nav, overlay, Point::new(100.0, 1346.0)),
            Some(Surface::Overlay),
            "beside the bar"
        );
        let root = app.root();
        assert_eq!(hit(nav, root, Point::ZERO), None, "not a dock window");
    }

    #[test]
    fn dock_hit_in_idle() {
        let mut app = dock_app();
        let overlay = commit(&mut app);
        app.resource_mut::<NavDock>().overlay_height = Some(1324.0);
        // A bar tap, in overlay coordinates: the bar centres 14 px above
        // the overlay's bottom edge.
        touch_down(&mut app, overlay, 306.0, 1324.0 - 14.0);
        contact(
            &mut app,
            overlay,
            ContactId::Touch(0),
            ContactPhase::Released,
            306.0,
            1310.0,
        );
        assert!(
            matches!(mode(&app), Mode::Idle { .. }),
            "the tap hit the bar"
        );
        let nav = app.resource::<NavDock>();
        let x = nav.state.icon_center_x(3);
        // The dock is 73 high with 26 below it.
        let y = 1324.0 - 26.0 - 73.0 / 2.0;
        assert_eq!(hit(nav, overlay, Point::new(x, y)), Some(Surface::Dock));
        assert_eq!(
            hit(nav, overlay, Point::new(x, 600.0)),
            Some(Surface::Overlay)
        );
    }

    #[test]
    fn any_window_resize_raises_the_screen() {
        let mut app = dock_app();
        let root = app.root();
        let home = app.spawn(root, window().title("home")).id();
        app.emit(
            Resized {
                size: Size::new(720.0, 1600.0),
            },
            home,
        );
        app.flush();
        let strip = strip(&app);
        app.emit(
            Resized {
                size: Size::new(120.0, 20.0),
            },
            strip,
        );
        app.flush();
        assert_eq!(
            app.resource::<NavDock>().state.screen(),
            Size::new(720.0, 1600.0)
        );
    }
}
