//! Where the finger-coupled elements go, and how they get there: pure
//! functions of the state's `View`. The windows apply them.
//!
//! Every visible node draws at its own displayed rect, in window
//! coordinates, under its own (or inherited) layout animation settings, so
//! a node and everything inside it move under one timing.

use std::time::Duration;

use gestures::{Axis, throw_speed};
use mecha_wayland::prelude::*;

use crate::config::NavDockConfig;
use crate::state::{Exit, Mode, Surface, View};
use crate::windows::eased_layout;

/// Target positions in the overlay's coordinates: the page row's left and
/// top edges, and the morph's bottom edge above the overlay's bottom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Targets {
    pub(crate) row_left: f32,
    pub(crate) row_top: f32,
    pub(crate) morph_bottom: f32,
}

/// The targets for `view` in an overlay of `overlay` size.
pub(crate) fn targets(view: &View, config: &NavDockConfig, overlay: Size) -> Targets {
    let (width, height) = (overlay.width, overlay.height);
    let (page, dx, dy) = view
        .overlay
        .map_or((view.last_page, 0.0, 0.0), |o| (o.page, o.dx, o.dy));
    let (row_top, morph_bottom) = match (view.mode, view.reveal_travel) {
        (Mode::Hidden, _) => match view.exit {
            Some(Exit::Up) => (-height, height),
            Some(Exit::Down) | None => (height, -(config.bar_size.1 + config.bar_bottom_gap)),
        },
        (_, Some(travel)) => {
            let dock_height = config.dock_size.1;
            let rest = dock_height + config.dock_bottom_gap;
            let risen = travel.min(rest) + (travel - rest).max(0.0) * config.dock_rubber;
            (
                height * (1.0 - (travel / config.reveal_full_px).min(1.0)),
                -dock_height + risen,
            )
        }
        (Mode::Dragging { .. } | Mode::Idle { .. }, None) => (0.0, config.dock_bottom_gap),
        (Mode::Minimized { .. }, None) => (dy, config.bar_bottom_gap - dy),
    };
    Targets {
        row_left: -(page as f32 * width) + dx,
        row_top,
        morph_bottom,
    }
}

/// How the elements move to their next targets: snapped while a finger
/// drags them, thrown at the release speed along `axis` (the axis the
/// released drag moved along) when `view.release` is newer than
/// `prev_release_seq`, eased over `anim` otherwise.
pub(crate) fn settings(
    view: &View,
    prev_release_seq: u64,
    axis: Axis,
    config: &NavDockConfig,
) -> LayoutAnimationSettings {
    // A bar drag moves nothing with the finger, so its look change eases.
    if view.held_surface.is_some_and(|s| s != Surface::Bar) {
        return eased_layout(Duration::ZERO);
    }
    match view.release {
        Some(r) if r.seq != prev_release_seq => {
            let v = match axis {
                Axis::Horizontal => r.vx,
                Axis::Vertical => r.vy,
            };
            let speed = throw_speed(v, config.min_throw_speed, config.throw_easing_slope);
            LayoutAnimationSettings::new(AnimationTime::Speed(speed), Easing::EaseOutCubic)
        }
        _ => eased_layout(config.anim),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{OverlayView, Release};

    const OVERLAY: Size = Size::new(612.0, 1324.0);

    fn view(mode: Mode) -> View {
        let page = match mode {
            Mode::Hidden => None,
            Mode::Dragging { hover: p }
            | Mode::Idle { active: p }
            | Mode::Minimized { active: p } => Some(p),
        };
        View {
            mode,
            strip: mode == Mode::Hidden,
            overlay: page.map(|page| OverlayView {
                page,
                dx: 0.0,
                dy: 0.0,
            }),
            held_surface: None,
            reveal_travel: None,
            release: None,
            exit: None,
            last_page: 0,
        }
    }

    fn strip_drag(travel: f32) -> View {
        View {
            held_surface: Some(Surface::Strip),
            reveal_travel: Some(travel),
            ..view(Mode::Dragging { hover: 2 })
        }
    }

    fn at(view: View) -> Targets {
        targets(&view, &NavDockConfig::default(), OVERLAY)
    }

    /// Settings carry no `PartialEq`; their `Debug` shows time and easing.
    fn same(a: LayoutAnimationSettings, b: LayoutAnimationSettings) {
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
    }

    fn throw(speed: f32) -> LayoutAnimationSettings {
        LayoutAnimationSettings::new(AnimationTime::Speed(speed), Easing::EaseOutCubic)
    }

    fn released(vx: f32, vy: f32) -> View {
        View {
            release: Some(Release { seq: 3, vx, vy }),
            ..view(Mode::Minimized { active: 2 })
        }
    }

    #[test]
    fn reveal_moves_row_and_morph_with_finger() {
        let t = at(strip_drag(150.0));
        assert_eq!(t.row_top, 662.0);
        assert_eq!(t.row_left, -2.0 * 612.0);
        // Past the rest point (73 + 26) the dock resists: 26 + 51 * 0.3.
        assert!((t.morph_bottom - 41.3).abs() < 1e-3, "{}", t.morph_bottom);
        assert_eq!(at(strip_drag(99.0)).morph_bottom, 26.0, "at rest");
        assert_eq!(at(strip_drag(50.0)).morph_bottom, -23.0);
        assert_eq!(at(strip_drag(0.0)).morph_bottom, -73.0, "below the edge");
        assert_eq!(at(strip_drag(400.0)).row_top, 0.0, "fully up");
    }

    #[test]
    fn dock_resists_past_rest() {
        assert!((at(strip_drag(199.0)).morph_bottom - 56.0).abs() < 1e-3);
    }

    #[test]
    fn rest_targets_without_travel() {
        for mode in [Mode::Dragging { hover: 1 }, Mode::Idle { active: 1 }] {
            let t = at(view(mode));
            assert_eq!((t.row_top, t.morph_bottom), (0.0, 26.0), "{mode:?}");
            assert_eq!(t.row_left, -612.0);
        }
    }

    #[test]
    fn minimized_follows_overlay_drag() {
        let mut v = view(Mode::Minimized { active: 3 });
        v.overlay = Some(OverlayView {
            page: 3,
            dx: -40.0,
            dy: 30.0,
        });
        let t = at(v);
        assert_eq!(t.row_left, -3.0 * 612.0 - 40.0);
        assert_eq!(t.row_top, 30.0);
        assert_eq!(t.morph_bottom, 8.0 - 30.0);
    }

    #[test]
    fn exit_up_targets_off_top() {
        let v = View {
            exit: Some(Exit::Up),
            last_page: 4,
            ..view(Mode::Hidden)
        };
        let t = at(v);
        assert_eq!(t.row_top, -1324.0);
        assert_eq!(t.morph_bottom, 1324.0);
        assert_eq!(t.row_left, -4.0 * 612.0, "stays on the last page");
    }

    #[test]
    fn exit_down_targets_off_bottom() {
        let v = View {
            exit: Some(Exit::Down),
            last_page: 1,
            ..view(Mode::Hidden)
        };
        let t = at(v);
        assert_eq!(t.row_top, 1324.0);
        assert_eq!(t.morph_bottom, -(12.0 + 8.0));
        assert_eq!(t.row_left, -612.0);
    }

    #[test]
    fn held_contact_uses_zero_duration() {
        let config = NavDockConfig::default();
        let zero = eased_layout(Duration::ZERO);
        same(
            settings(&strip_drag(150.0), 0, Axis::Vertical, &config),
            zero,
        );
        // A held drag snaps even with an unapplied release on record.
        let mut v = released(0.0, -3.0);
        v.held_surface = Some(Surface::Overlay);
        same(settings(&v, 0, Axis::Vertical, &config), zero);
    }

    #[test]
    fn bar_drag_eases_over_anim() {
        // The bar drag moves nothing with the finger: the morph opens over
        // `anim` while the hover follows.
        let config = NavDockConfig::default();
        let v = View {
            held_surface: Some(Surface::Bar),
            ..view(Mode::Dragging { hover: 1 })
        };
        same(
            settings(&v, 0, Axis::Vertical, &config),
            eased_layout(config.anim),
        );
    }

    #[test]
    fn new_release_uses_throw_speed() {
        let config = NavDockConfig::default();
        same(
            settings(&released(0.2, -3.0), 2, Axis::Vertical, &config),
            throw(1000.0),
        );
    }

    #[test]
    fn page_switch_throws_along_x() {
        let config = NavDockConfig::default();
        same(
            settings(&released(-6.0, 0.1), 2, Axis::Horizontal, &config),
            throw(2000.0),
        );
    }

    #[test]
    fn slow_release_uses_min_throw_speed() {
        let config = NavDockConfig::default();
        same(
            settings(&released(0.0, -0.01), 2, Axis::Vertical, &config),
            throw(500.0),
        );
    }

    #[test]
    fn applied_release_eases_over_anim() {
        let config = NavDockConfig::default();
        same(
            settings(&released(0.0, -3.0), 3, Axis::Vertical, &config),
            eased_layout(config.anim),
        );
        same(
            settings(&view(Mode::Idle { active: 0 }), 0, Axis::Vertical, &config),
            eased_layout(config.anim),
        );
    }
}
