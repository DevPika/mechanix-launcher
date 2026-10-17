//! Raw contact input into the state machine: each `ContactInput` on a dock
//! window becomes a screen-space `DockEvent`, then the windows reconcile.

use std::time::Instant;

use mecha_wayland::prelude::*;

use crate::state::{DockEvent, Surface};
use crate::windows::{NavDock, reconcile, surface_of};

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
                let Some(surface) = surface_of(&nav, c.window).filter(|_| live) else {
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

/// Screen position of `surface`'s window's top-left corner. The strip, bar
/// and dock sit bottom-centre at the sizes they are spawned with; the
/// overlay spans the width and ends at the bottom edge, its top pushed
/// down by the status bar's exclusive zone.
fn origin(nav: &NavDock, surface: Surface) -> Point {
    let config = nav.state.config();
    let screen = nav.state.screen();
    let (width, height) = match surface {
        Surface::Strip => config.strip_size,
        Surface::Dock => (
            config.dock_size.0,
            config.dock_size.1 + config.dock_bottom_gap,
        ),
        Surface::Bar => (config.bar_size.0, config.bar_size.1 + config.bar_bottom_gap),
        Surface::Overlay => {
            let height = nav.overlay_height.unwrap_or(screen.height);
            return Point::new(0.0, screen.height - height);
        }
    };
    Point::new(screen.width / 2.0 - width / 2.0, screen.height - height)
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
        let dock = app.resource::<NavDock>().windows.dock;
        assert!(
            dock.is_some_and(|d| app.is_live(d)),
            "the dock window exists"
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
        // release is no flick: it commits, and the bar is up.
        let bar = nav.windows.bar.expect("bar is up");
        let after = mode(&app);
        assert!(matches!(after, Mode::Minimized { .. }));

        contact(
            &mut app,
            bar,
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
        assert_eq!(origin(&nav, Surface::Dock), Point::new(74.0, 1261.0));
        assert_eq!(origin(&nav, Surface::Bar), Point::new(226.0, 1340.0));
        assert_eq!(
            origin(&nav, Surface::Overlay),
            Point::ZERO,
            "height unknown"
        );
        nav.overlay_height = Some(1324.0);
        assert_eq!(origin(&nav, Surface::Overlay), Point::new(0.0, 36.0));
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
