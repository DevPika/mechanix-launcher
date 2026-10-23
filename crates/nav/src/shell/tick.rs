//! Frame ticks: advance the stack by the time since the last frame.

use std::time::Instant;

use mecha_wayland::prelude::*;

use super::apply::{Nav, apply};
use crate::core::Input;

/// Ticks the stack on a nav window's `Frame` while anything animates.
/// The first frame after idling ticks by zero, so a fade that starts
/// late does not skip ahead.
pub(crate) fn on_frame(app: &mut App, f: &Frame) {
    let now = Instant::now();
    {
        let mut nav = app.resource_mut::<Nav>();
        if !nav.windows.iter().any(|w| w.node == f.0) {
            return;
        }
        if nav.stack.animating().is_empty() {
            nav.last_tick = None;
            return;
        }
        let dt = nav.last_tick.map_or_else(Default::default, |t| now - t);
        nav.last_tick = Some(now);
        nav.stack.handle(Input::Tick { dt });
    }
    apply(app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::module::tests::{app_with, module};

    #[test]
    fn idle_frames_leave_last_tick_unset() {
        let mut app = app_with(module(true));
        let window = app.resource::<Nav>().windows[0].node;
        app.signal(Frame(window));
        app.flush();
        assert!(app.resource::<Nav>().last_tick.is_none());
    }

    #[test]
    fn frames_while_animating_tick() {
        let mut app = app_with(module(false));
        let strip = app.resource::<Nav>().windows[1].node;
        app.emit(
            Resized {
                size: Size::new(600.0, 20.0),
            },
            strip,
        );
        app.flush();
        for (phase, y) in [
            (ContactPhase::Pressed, 10.0),
            (ContactPhase::Moved, 15.0),
            (ContactPhase::Moved, -70.0),
            (ContactPhase::Released, -70.0),
        ] {
            app.signal(ContactInput {
                window: strip,
                contact: ContactId::Touch(0),
                phase,
                position: Point::new(300.0, y),
            });
            app.flush();
        }
        assert!(!app.resource::<Nav>().stack.animating().is_empty());
        let window = app.resource::<Nav>().windows[0].node;
        app.signal(Frame(window));
        app.flush();
        assert!(app.resource::<Nav>().last_tick.is_some());
    }
}
