//! Touch input: each `ContactInput` on a nav window becomes a stack input.

use std::time::Instant;

use mecha_wayland::prelude::*;

use super::apply::{Nav, apply};
use crate::core::Input;

/// The stack's contact id for the mouse pointer, so nav can be driven
/// from a desktop.
const MOUSE_CONTACT: u32 = u32::MAX;

/// Routes one contact report into the stack. A `Moved` only counts while
/// its contact is pressed (a touch's first `Moved` comes just before its
/// `Pressed`, and the mouse hovers). A touch stays bound to the window it
/// went down on, so releases and cancels need no window check beyond
/// the contact being pressed.
pub(crate) fn on_contact(app: &mut App, c: &ContactInput) {
    let contact = match c.contact {
        ContactId::Mouse => MOUSE_CONTACT,
        ContactId::Touch(id) => id,
    };
    let t = Instant::now();
    let live = app.is_live(c.window);
    {
        let mut nav = app.resource_mut::<Nav>();
        let pressed = nav.pressed.contains(&contact);
        let input = match c.phase {
            ContactPhase::Pressed => {
                let Some(w) = nav.windows.iter().find(|w| w.node == c.window) else {
                    return;
                };
                if !live {
                    return;
                }
                let area = w.area;
                let area_size = nav.sizes.get(&c.window).copied().unwrap_or(Size::ZERO);
                if !pressed {
                    nav.pressed.push(contact);
                }
                Input::Down {
                    contact,
                    area,
                    area_size,
                    pos: c.position,
                    t,
                }
            }
            ContactPhase::Moved => {
                if !pressed {
                    return;
                }
                Input::Move {
                    contact,
                    pos: c.position,
                    t,
                }
            }
            ContactPhase::Released | ContactPhase::Cancelled => {
                if !pressed {
                    return;
                }
                nav.pressed.retain(|&p| p != contact);
                if c.phase == ContactPhase::Released {
                    Input::Up { contact, t }
                } else {
                    Input::Cancel
                }
            }
        };
        nav.stack.handle(input);
    }
    apply(app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::BOTTOM_EDGE;
    use crate::shell::module::tests::{HOME, app_with, display, module, node_of};

    fn contact(app: &mut App, window: NodeId, phase: ContactPhase, x: f32, y: f32) {
        app.signal(ContactInput {
            window,
            contact: ContactId::Touch(0),
            phase,
            position: Point::new(x, y),
        });
        app.flush();
    }

    fn window_of(app: &App, area: crate::core::AreaId) -> NodeId {
        app.resource::<Nav>()
            .windows
            .iter()
            .find(|w| w.area == area)
            .unwrap()
            .node
    }

    #[test]
    fn bottom_strip_quick_swipe_opens_home() {
        let mut app = app_with(module(false));
        let strip = window_of(&app, BOTTOM_EDGE);
        app.emit(
            Resized {
                size: Size::new(600.0, 20.0),
            },
            strip,
        );
        app.flush();
        assert_eq!(display(&app, node_of(&app, HOME)), Display::Hidden);
        contact(&mut app, strip, ContactPhase::Pressed, 300.0, 10.0);
        contact(&mut app, strip, ContactPhase::Moved, 300.0, 15.0);
        contact(&mut app, strip, ContactPhase::Moved, 300.0, -70.0);
        contact(&mut app, strip, ContactPhase::Released, 300.0, -70.0);
        app.signal(Frame(window_of(&app, crate::core::AreaId("home"))));
        app.flush();
        assert_eq!(display(&app, node_of(&app, HOME)), Display::Flex);
    }

    #[test]
    fn contact_on_unknown_window_is_ignored() {
        let mut app = app_with(module(false));
        let root = app.root();
        let strip = window_of(&app, BOTTOM_EDGE);
        for w in [root, strip] {
            if w == strip {
                app.remove(strip);
            }
            contact(&mut app, w, ContactPhase::Pressed, 300.0, 10.0);
            contact(&mut app, w, ContactPhase::Moved, 300.0, -70.0);
            contact(&mut app, w, ContactPhase::Released, 300.0, -70.0);
            assert_eq!(display(&app, node_of(&app, HOME)), Display::Hidden);
            assert!(app.resource::<Nav>().pressed.is_empty());
        }
    }

    #[test]
    fn moved_without_press_is_ignored() {
        let mut app = app_with(module(false));
        let strip = window_of(&app, BOTTOM_EDGE);
        contact(&mut app, strip, ContactPhase::Moved, 300.0, 15.0);
        contact(&mut app, strip, ContactPhase::Moved, 300.0, -70.0);
        contact(&mut app, strip, ContactPhase::Released, 300.0, -70.0);
        assert_eq!(display(&app, node_of(&app, HOME)), Display::Hidden);
        assert!(app.resource::<Nav>().pressed.is_empty());
    }
}
