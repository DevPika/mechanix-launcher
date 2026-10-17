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
use crate::dock::Look;
use crate::state::{Exit, Mode, Surface, View};
use crate::windows::eased_layout;

/// Target positions in the overlay's coordinates: the page row's left and
/// top edges, and the morph's bottom edge above the overlay's bottom.
///
/// The row's edges are percentages of the window while they rest or exit,
/// so they always match the real window, and px only while a finger offsets
/// them (Taffy cannot add a percentage to px).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Targets {
    pub(crate) row_left: Val,
    pub(crate) row_top: Val,
    pub(crate) morph_bottom: f32,
}

/// The targets for `view` in an overlay of `overlay` size, which only the
/// finger-coupled px values and the morph depend on.
pub(crate) fn targets(view: &View, config: &NavDockConfig, overlay: Size) -> Targets {
    let (width, height) = (overlay.width, overlay.height);
    let (page, dx, dy) = view
        .overlay
        .map_or((view.last_page, 0.0, 0.0), |o| (o.page, o.dx, o.dy));
    let (row_top, morph_bottom) = match (view.mode, view.reveal_travel) {
        // A bar drag grows the bar into the dock under the finger; the
        // page stays put.
        (Mode::Dragging { .. }, None) if view.lift.is_some() => (
            percent(0.0),
            lifted_bottom(view.lift.unwrap_or(0.0), config),
        ),
        (Mode::Hidden, _) => match view.exit {
            Some(Exit::Up) => (percent(-100.0), height),
            // Below the edge in either look: the pill may still be expanded.
            Some(Exit::Down) | None => (
                percent(100.0),
                -(config.dock_size.1.max(config.bar_size.1) + config.bar_bottom_gap),
            ),
        },
        (_, Some(travel)) => {
            let dock_height = config.dock_size.1;
            let rest = dock_height + config.dock_bottom_gap;
            let risen = travel.min(rest) + (travel - rest).max(0.0) * config.dock_rubber;
            (percent(0.0), -dock_height + risen)
        }
        (Mode::Dragging { .. }, None) => (percent(0.0), config.dock_bottom_gap),
        (Mode::Idle { .. }, None) => (rest_or_px(0.0, height, dy), config.dock_bottom_gap - dy),
        (Mode::Minimized { .. }, None) => (rest_or_px(0.0, height, dy), config.bar_bottom_gap - dy),
    };
    Targets {
        row_left: rest_or_px(-100.0 * page as f32, width, dx),
        row_top,
        morph_bottom,
    }
}

/// The extra height between the bar's and the dock's resting tops: the
/// lift over which a bar drag grows the bar into the dock.
fn lift_rest(config: &NavDockConfig) -> f32 {
    config.dock_size.1 + config.dock_bottom_gap - (config.bar_size.1 + config.bar_bottom_gap)
}

/// The morph's bottom `lift` px into a bar drag: its top follows the finger
/// 1:1 up to the dock's rest, then resists as a strip drag's does.
fn lifted_bottom(lift: f32, config: &NavDockConfig) -> f32 {
    let rest = lift_rest(config);
    let t = (lift / rest).clamp(0.0, 1.0);
    config.bar_bottom_gap
        + t * (config.dock_bottom_gap - config.bar_bottom_gap)
        + (lift - rest).max(0.0) * config.dock_rubber
}

/// The morph's look for `view`, from 0 (the bar) to 1 (the dock): the
/// lift's share of `lift_rest` during a bar drag, the mode's look
/// otherwise. `None` while Hidden: the morph keeps the look it has.
pub(crate) fn morph_t(view: &View, config: &NavDockConfig) -> Option<f32> {
    if let Some(lift) = view.lift {
        return Some((lift / lift_rest(config)).clamp(0.0, 1.0));
    }
    Look::of(view.mode).map(|(look, _)| look.t())
}

/// `rest` percent of a window side of `extent` px, moved by the finger's
/// `d` px: a percentage while `d` is zero, px once it is not.
fn rest_or_px(rest: f32, extent: f32, d: f32) -> Val {
    if d == 0.0 {
        percent(rest)
    } else {
        px(rest / 100.0 * extent + d)
    }
}

/// `val` in px of an `extent` long window side.
pub(crate) fn resolve(val: Val, extent: f32) -> f32 {
    match val {
        Val::Px(v) => v,
        Val::Percent(p) => p / 100.0 * extent,
        Val::Auto => 0.0,
    }
}

/// The largest absolute delta between the displayed row position
/// `row_xy`, the displayed morph bottom `morph_bottom` and `targets` in an
/// `overlay`: how far the farthest edge has to travel.
pub(crate) fn travel(
    targets: Targets,
    row_xy: (f32, f32),
    morph_bottom: f32,
    overlay: Size,
) -> f32 {
    (resolve(targets.row_left, overlay.width) - row_xy.0)
        .abs()
        .max((resolve(targets.row_top, overlay.height) - row_xy.1).abs())
        .max((targets.morph_bottom - morph_bottom).abs())
}

/// Whether a held finger moves the morph (and, but for the strip's, the
/// row): any held contact but the bar's, whose tap moves nothing.
pub(crate) fn coupled(view: &View) -> bool {
    view.held_surface.is_some_and(|s| s != Surface::Bar)
}

/// Whether a held finger drives the morph: a coupled contact, or a bar
/// drag, whose lift grows the bar into the dock.
pub(crate) fn morph_coupled(view: &View) -> bool {
    coupled(view) || view.lift.is_some()
}

/// Whether a held finger moves the row: a coupled contact but the strip's,
/// whose row slides up on its own while only the dock follows the finger.
pub(crate) fn row_coupled(view: &View) -> bool {
    coupled(view) && view.held_surface != Some(Surface::Strip)
}

/// The throw for `view.release` when it is newer than `prev_release_seq`
/// and ended a drag that moved along `axis`: its speed along that axis.
fn throw(
    view: &View,
    prev_release_seq: u64,
    axis: Option<Axis>,
    distance: f32,
    config: &NavDockConfig,
) -> Option<LayoutAnimationSettings> {
    let r = view.release.filter(|r| r.seq != prev_release_seq)?;
    let v = match axis? {
        Axis::Horizontal => r.vx,
        Axis::Vertical => r.vy,
    };
    // A slow release would take `3 * distance / v`: raise it so the
    // farthest edge arrives within `max_throw_time`.
    let speed = throw_speed(v, config.min_throw_speed, config.throw_easing_slope)
        .max(distance / config.max_throw_time.as_secs_f32());
    Some(LayoutAnimationSettings::new(
        AnimationTime::Speed(speed),
        Easing::EaseOutCubic,
    ))
}

/// How the row moves to its next targets: snapped while a finger drags
/// it (not a strip drag's), thrown at the release speed when a new release ended a drag along
/// `axis` (at least fast enough to cover `distance` within
/// `max_throw_time`), eased over `anim` otherwise.
pub(crate) fn settings(
    view: &View,
    prev_release_seq: u64,
    axis: Option<Axis>,
    distance: f32,
    config: &NavDockConfig,
) -> LayoutAnimationSettings {
    if row_coupled(view) {
        return eased_layout(Duration::ZERO);
    }
    throw(view, prev_release_seq, axis, distance, config).unwrap_or(eased_layout(config.anim))
}

/// How the morph moves to its next targets: snapped while a finger drives
/// it, thrown as the row when it is thrown off screen or when the release
/// ended a bar drag (`lifted`) that drove it, otherwise eased over `anim`,
/// since its look change (commit, tap) is not the finger's.
pub(crate) fn morph_settings(
    view: &View,
    prev_release_seq: u64,
    axis: Option<Axis>,
    distance: f32,
    config: &NavDockConfig,
    lifted: bool,
) -> LayoutAnimationSettings {
    if morph_coupled(view) {
        return eased_layout(Duration::ZERO);
    }
    if view.exit.is_some() || lifted {
        return throw(view, prev_release_seq, axis, distance, config)
            .unwrap_or(eased_layout(config.anim));
    }
    eased_layout(config.anim)
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
            lift: None,
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

    fn bar_drag(lift: f32) -> View {
        View {
            held_surface: Some(Surface::Bar),
            lift: Some(lift),
            ..view(Mode::Dragging { hover: 1 })
        }
    }

    /// The extra height between the bar's and the dock's rest tops.
    fn rest(config: &NavDockConfig) -> f32 {
        config.dock_size.1 + config.dock_bottom_gap - (config.bar_size.1 + config.bar_bottom_gap)
    }

    #[test]
    fn bar_drag_does_not_move_the_page() {
        let config = NavDockConfig::default();
        let v = bar_drag(200.0);
        let t = at(v);
        assert_eq!(t.row_top, percent(0.0));
        assert_eq!(t.row_left, percent(-100.0), "on the current page");
        same(settings(&v, 0, V, 0.0, &config), eased_layout(config.anim));
    }

    #[test]
    fn bar_drag_morph_follows_lift() {
        let config = NavDockConfig::default();
        let v = bar_drag(rest(&config) / 2.0);
        assert_eq!(morph_t(&v, &config), Some(0.5));
        let halfway = (config.bar_bottom_gap + config.dock_bottom_gap) / 2.0;
        assert!(
            (at(v).morph_bottom - halfway).abs() < 1e-3,
            "{}",
            at(v).morph_bottom
        );
        same(
            morph_settings(&v, 0, V, 0.0, &config, false),
            eased_layout(Duration::ZERO),
        );
        assert_eq!(morph_t(&bar_drag(0.0), &config), Some(0.0));
        assert_eq!(at(bar_drag(0.0)).morph_bottom, config.bar_bottom_gap);
    }

    #[test]
    fn bar_drag_morph_resists_past_rest() {
        let config = NavDockConfig::default();
        let v = bar_drag(rest(&config) + 100.0);
        assert_eq!(morph_t(&v, &config), Some(1.0));
        let want = config.dock_bottom_gap + 100.0 * config.dock_rubber;
        assert!(
            (at(v).morph_bottom - want).abs() < 1e-3,
            "{}",
            at(v).morph_bottom
        );
    }

    #[test]
    fn bar_drag_commit_throws_morph_back_to_bar() {
        let config = NavDockConfig::default();
        let v = released(0.0, -0.5);
        assert_eq!(morph_t(&v, &config), Some(0.0));
        assert_eq!(at(v).morph_bottom, config.bar_bottom_gap);
        let s = morph_settings(&v, 2, V, 0.0, &config, true);
        same(s, settings(&v, 2, V, 0.0, &config));
        assert!(format!("{s:?}").contains("Speed("), "{s:?}");
        // A release that did not lift the morph still eases.
        same(
            morph_settings(&v, 2, V, 0.0, &config, false),
            eased_layout(config.anim),
        );
    }

    #[test]
    fn reveal_moves_only_the_morph_with_finger() {
        let t = at(strip_drag(150.0));
        assert_eq!(
            t.row_top,
            percent(0.0),
            "the row is fully up from the first pixel"
        );
        assert_eq!(at(strip_drag(1.0)).row_top, percent(0.0));
        assert_eq!(t.row_left, percent(-200.0));
        // Past the rest point (73 + 26) the dock resists: 26 + 51 * 0.3.
        assert!((t.morph_bottom - 41.3).abs() < 1e-3, "{}", t.morph_bottom);
        assert_eq!(at(strip_drag(99.0)).morph_bottom, 26.0, "at rest");
        assert_eq!(at(strip_drag(50.0)).morph_bottom, -23.0);
        assert_eq!(at(strip_drag(0.0)).morph_bottom, -73.0, "below the edge");
    }

    #[test]
    fn strip_drag_row_slides_up_on_anim_while_dock_follows() {
        let config = NavDockConfig::default();
        let v = strip_drag(50.0);
        assert_eq!(at(v).row_top, percent(0.0));
        assert_eq!(at(v).morph_bottom, -23.0, "the dock follows travel");
        same(settings(&v, 0, V, 0.0, &config), eased_layout(config.anim));
        same(
            morph_settings(&v, 0, V, 0.0, &config, false),
            eased_layout(Duration::ZERO),
        );
    }

    #[test]
    fn strip_release_does_not_throw_the_row() {
        let config = NavDockConfig::default();
        same(
            settings(&released(0.0, -3.0), 2, None, 0.0, &config),
            eased_layout(config.anim),
        );
    }

    #[test]
    fn dock_resists_past_rest() {
        assert!((at(strip_drag(199.0)).morph_bottom - 56.0).abs() < 1e-3);
    }

    #[test]
    fn rest_targets_without_travel() {
        for mode in [Mode::Dragging { hover: 1 }, Mode::Idle { active: 1 }] {
            let t = at(view(mode));
            assert_eq!(
                (t.row_top, t.morph_bottom),
                (percent(0.0), 26.0),
                "{mode:?}"
            );
            assert_eq!(t.row_left, percent(-100.0));
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
        assert_eq!(
            t.row_left,
            px(-3.0 * 612.0 - 40.0),
            "px while a finger offsets it"
        );
        assert_eq!(t.row_top, px(30.0));
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
        assert_eq!(t.row_top, percent(-100.0));
        assert_eq!(t.morph_bottom, 1324.0);
        assert_eq!(t.row_left, percent(-400.0), "stays on the last page");
    }

    #[test]
    fn exit_down_targets_off_bottom() {
        let v = View {
            exit: Some(Exit::Down),
            last_page: 1,
            ..view(Mode::Hidden)
        };
        let t = at(v);
        assert_eq!(t.row_top, percent(100.0));
        assert_eq!(t.morph_bottom, -(73.0 + 8.0));
        assert_eq!(t.row_left, percent(-100.0));
    }

    #[test]
    fn exit_down_from_expanded_is_fully_below() {
        let config = NavDockConfig::default();
        let v = View {
            exit: Some(Exit::Down),
            ..view(Mode::Hidden)
        };
        // The pill's top edge is its bottom plus the taller look's height.
        let top = at(v).morph_bottom + config.dock_size.1;
        assert!(top <= 0.0, "the expanded pill's top is at {top}");
    }

    #[test]
    fn idle_follows_close_drag() {
        let mut v = view(Mode::Idle { active: 1 });
        v.overlay = Some(OverlayView {
            page: 1,
            dx: 0.0,
            dy: 40.0,
        });
        let t = at(v);
        assert_eq!(t.row_top, px(40.0));
        assert_eq!(t.morph_bottom, 26.0 - 40.0);
    }

    #[test]
    fn travel_is_the_largest_delta() {
        let t = Targets {
            row_left: percent(-100.0),
            row_top: percent(0.0),
            morph_bottom: 26.0,
        };
        assert_eq!(travel(t, (0.0, 100.0), 26.0, OVERLAY), 612.0);
        assert_eq!(travel(t, (-612.0, 1324.0), -73.0, OVERLAY), 1324.0);
        assert_eq!(travel(t, (-612.0, 0.0), 26.0, OVERLAY), 0.0);
    }

    const V: Option<Axis> = Some(Axis::Vertical);

    #[test]
    fn held_contact_uses_zero_duration() {
        let config = NavDockConfig::default();
        let zero = eased_layout(Duration::ZERO);
        let v = strip_drag(150.0);
        same(morph_settings(&v, 0, V, 0.0, &config, false), zero);
        // A held drag snaps even with an unapplied release on record.
        let mut v = released(0.0, -3.0);
        v.held_surface = Some(Surface::Overlay);
        same(settings(&v, 0, V, 0.0, &config), zero);
        same(morph_settings(&v, 0, V, 0.0, &config, false), zero);
    }

    #[test]
    fn bar_drag_eases_over_anim() {
        // A bar contact without lift (a tap) moves nothing with the finger:
        // the morph opens over `anim`.
        let config = NavDockConfig::default();
        let v = View {
            held_surface: Some(Surface::Bar),
            ..view(Mode::Dragging { hover: 1 })
        };
        let anim = eased_layout(config.anim);
        same(settings(&v, 0, None, 0.0, &config), anim);
        same(morph_settings(&v, 0, None, 0.0, &config, false), anim);
    }

    #[test]
    fn new_release_uses_throw_speed() {
        let config = NavDockConfig::default();
        same(
            settings(&released(0.2, -3.0), 2, V, 0.0, &config),
            throw(1000.0),
        );
    }

    #[test]
    fn long_slow_throw_finishes_within_max_throw_time() {
        let config = NavDockConfig::default();
        let s = settings(&released(0.0, -0.01), 2, V, 2000.0, &config);
        let want = 2000.0 / config.max_throw_time.as_secs_f32();
        // 0.01 px/ms alone would give 1000 px/s: the cap raises it.
        assert!(want > 1000.0);
        same(s, throw(want));
    }

    #[test]
    fn page_switch_throws_along_x() {
        let config = NavDockConfig::default();
        let h = Some(Axis::Horizontal);
        same(
            settings(&released(-6.0, 0.1), 2, h, 0.0, &config),
            throw(2000.0),
        );
    }

    #[test]
    fn slow_release_uses_min_throw_speed() {
        let config = NavDockConfig::default();
        same(
            settings(&released(0.0, -0.01), 2, V, 0.0, &config),
            throw(1000.0),
        );
    }

    #[test]
    fn uncoupled_release_eases_over_anim() {
        // A tap moved nothing: there is no throw to carry.
        let config = NavDockConfig::default();
        let anim = eased_layout(config.anim);
        same(settings(&released(0.0, -3.0), 2, None, 0.0, &config), anim);
        same(
            morph_settings(&released(0.0, -3.0), 2, None, 0.0, &config, false),
            anim,
        );
    }

    #[test]
    fn morph_throws_only_on_exit() {
        let config = NavDockConfig::default();
        // A commit: the row is thrown, the morph's look change eases.
        let commit = released(0.0, -3.0);
        same(
            morph_settings(&commit, 2, V, 0.0, &config, false),
            eased_layout(config.anim),
        );
        let flick = View {
            exit: Some(Exit::Up),
            ..released(0.0, -3.0)
        };
        same(
            morph_settings(&flick, 2, V, 0.0, &config, false),
            throw(1000.0),
        );
    }

    #[test]
    fn applied_release_eases_over_anim() {
        let config = NavDockConfig::default();
        let anim = eased_layout(config.anim);
        same(settings(&released(0.0, -3.0), 3, V, 0.0, &config), anim);
        same(
            settings(&view(Mode::Idle { active: 0 }), 0, V, 0.0, &config),
            anim,
        );
    }
}
