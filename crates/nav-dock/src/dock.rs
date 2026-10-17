//! The expanded dock (a pill of section icons) and the minimized bar (a
//! black pill of dots), each in its own bottom-centre window.

use std::time::Duration;

use mecha_wayland::prelude::*;

use crate::config::NavDockConfig;
use crate::windows::{bottom_centre_role, eased_layout, eased_paint};

pub(crate) const DOCK_NAMESPACE: &str = "mechanix-nav-dock";
pub(crate) const BAR_NAMESPACE: &str = "mechanix-nav-bar";

/// Alpha of the dock pill's `SurfaceContainerHighest` fill.
const PILL_ALPHA: f32 = 0.2;
/// Side of the box behind a plain icon, and behind the emphasised one.
const ICON_BOX: f32 = 44.0;
const ICON_BOX_EMPHASIS: f32 = 56.0;
/// Alpha of the `Primary` box behind the emphasised icon.
const EMPHASIS_ALPHA: f32 = 0.24;
/// Diameter of a bar dot.
const DOT: f32 = 4.0;

/// Emphasise icon `.0` of the dock row. Emitted at the row node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DockFocus(pub usize);
impl Event for DockFocus {}

/// Highlight dot `.0` of the bar. Emitted at the bar node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BarActive(pub usize);
impl Event for BarActive {}

/// Spawns the dock window, `dock_size` with `dock_bottom_gap` of empty
/// window below the pill, and the row in it with `emphasis` highlighted.
pub(crate) fn spawn_dock(
    app: &mut App,
    config: &NavDockConfig,
    icons: &[SpriteId],
    emphasis: usize,
) -> NodeId {
    let (width, height) = config.dock_size;
    let win = spawn_bottom_window(app, DOCK_NAMESPACE, width, height + config.dock_bottom_gap);
    app.spawn(
        win,
        DockRowBuilder {
            icons: icons.to_vec(),
            height,
            emphasis,
            anim: config.anim,
        },
    );
    win
}

/// Spawns the bar window, `bar_size` with `bar_bottom_gap` below, and the
/// bar in it with dot `active` highlighted.
pub(crate) fn spawn_bar(
    app: &mut App,
    config: &NavDockConfig,
    count: usize,
    active: usize,
) -> NodeId {
    let (width, height) = config.bar_size;
    let win = spawn_bottom_window(app, BAR_NAMESPACE, width, height + config.bar_bottom_gap);
    app.spawn(
        win,
        BarBuilder {
            count,
            height,
            active,
            anim: config.anim,
        },
    );
    win
}

/// A column window of a fixed size. Windows are opaque, so the area around
/// the pill clears to `Surface`, the colour of the page underneath.
fn spawn_bottom_window(app: &mut App, namespace: &str, width: f32, height: f32) -> NodeId {
    let clear = app.color(ColorRole::Surface);
    let root = app.root();
    app.spawn_with(
        root,
        window()
            .title(namespace)
            .layout(LayoutStyle::default().column().size(px(width), px(height)))
            .clear(clear),
        (bottom_centre_role(namespace),),
    )
    .id()
}

// ---------------------------------------------------------------------------
// Dock row
// ---------------------------------------------------------------------------

/// The expanded dock: one equal-width slot per section, so icon `i` is
/// centred where `NavState::icon_center_x(i)` expects it.
pub(crate) struct DockRow {
    /// Per slot: the emphasis box and the icon inside it.
    cells: Vec<(Handle<Div>, Handle<Icon>)>,
    emphasis: usize,
}

#[cfg(test)]
impl DockRow {
    pub(crate) fn emphasis(&self) -> usize {
        self.emphasis
    }
}

pub(crate) struct DockRowBuilder {
    icons: Vec<SpriteId>,
    height: f32,
    emphasis: usize,
    anim: Duration,
}

impl Build for DockRowBuilder {
    type Widget = DockRow;
}

impl Widget for DockRow {
    type Builder = DockRowBuilder;

    fn build(b: DockRowBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = LayoutStyle::default()
            .row()
            .align_items(Align::Center)
            .width(percent(100.0))
            .height(px(b.height));
        *s.component_mut::<Paint>(me).unwrap() = Paint::Quad(pill(s, b.height));
        *s.component_mut::<LayoutAnimationSettings>(me).unwrap() = eased_layout(b.anim);
        *s.component_mut::<PaintAnimationSettings>(me).unwrap() = eased_paint(b.anim);

        let slot_width = 100.0 / b.icons.len().max(1) as f32;
        let mut cells = Vec::with_capacity(b.icons.len());
        for (i, sprite) in b.icons.iter().enumerate() {
            let look = IconLook::of(s, i == b.emphasis);
            let slot = s.spawn(
                me,
                div().style(
                    LayoutStyle::default()
                        .center()
                        .width(percent(slot_width))
                        .height(percent(100.0)),
                ),
            );
            let hl = s.spawn(slot, div().style(look.box_style()));
            *s.component_mut::<Paint>(hl).unwrap() = Paint::Quad(look.box_quad());
            let ic = s.spawn(hl, icon(*sprite).color(look.tint));
            cells.push((hl, ic));
        }

        s.on::<DockFocus>(me, |ctx, e| {
            ctx.me().emphasis = e.0;
            apply_emphasis(ctx);
        });
        let height = b.height;
        s.on_theme(me, move |ctx| {
            let quad = pill(ctx, height);
            ctx.set_paint(Paint::Quad(quad));
            apply_emphasis(ctx);
        });

        DockRow {
            cells,
            emphasis: b.emphasis,
        }
    }
}

fn pill(theme: &impl ThemeReader, height: f32) -> Quad {
    let fill = theme
        .color(ColorRole::SurfaceContainerHighest)
        .with_alpha(PILL_ALPHA);
    Quad::new(fill).radius(height / 2.0)
}

fn apply_emphasis(ctx: &mut Context<'_, DockRow>) {
    let emphasis = ctx.me().emphasis;
    let cells = ctx.me().cells.clone();
    for (i, (hl, ic)) in cells.into_iter().enumerate() {
        let look = IconLook::of(ctx, i == emphasis);
        if let Some(mut hl) = ctx.at(hl) {
            hl.set_style(look.box_style());
            hl.set_paint(Paint::Quad(look.box_quad()));
        }
        if let Some(mut ic) = ctx.at(ic) {
            ic.set_color(look.tint);
        }
    }
}

/// How one icon looks, plain or emphasised. The box behind is always a
/// `Primary` quad, transparent when plain, so the change animates.
struct IconLook {
    side: f32,
    fill: Color,
    tint: Color,
}

impl IconLook {
    fn of(theme: &impl ThemeReader, emphasised: bool) -> Self {
        let primary = theme.color(ColorRole::Primary);
        if emphasised {
            Self {
                side: ICON_BOX_EMPHASIS,
                fill: primary.with_alpha(EMPHASIS_ALPHA),
                tint: primary,
            }
        } else {
            Self {
                side: ICON_BOX,
                fill: primary.with_alpha(0.0),
                tint: theme.color(ColorRole::OnSurfaceVariant),
            }
        }
    }

    fn box_style(&self) -> LayoutStyle {
        LayoutStyle::default()
            .center()
            .size(px(self.side), px(self.side))
    }

    fn box_quad(&self) -> Quad {
        Quad::new(self.fill).radius(self.side / 2.0)
    }
}

// ---------------------------------------------------------------------------
// Bar
// ---------------------------------------------------------------------------

/// The minimized dock: a black pill with one dot per section.
pub(crate) struct Bar {
    dots: Vec<Handle<Div>>,
    active: usize,
}

#[cfg(test)]
impl Bar {
    pub(crate) fn active(&self) -> usize {
        self.active
    }
}

pub(crate) struct BarBuilder {
    count: usize,
    height: f32,
    active: usize,
    anim: Duration,
}

impl Build for BarBuilder {
    type Widget = Bar;
}

impl Widget for Bar {
    type Builder = BarBuilder;

    fn build(b: BarBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = LayoutStyle::default()
            .row()
            .center()
            .justify(Justify::SpaceEvenly)
            .width(percent(100.0))
            .height(px(b.height));
        *s.component_mut::<Paint>(me).unwrap() =
            Paint::Quad(Quad::new(Color::BLACK).radius(b.height / 2.0));
        *s.component_mut::<PaintAnimationSettings>(me).unwrap() = eased_paint(b.anim);

        let dots = (0..b.count)
            .map(|i| {
                let dot = s.spawn(
                    me,
                    div().style(LayoutStyle::default().size(px(DOT), px(DOT))),
                );
                *s.component_mut::<Paint>(dot).unwrap() = Paint::Quad(dot_quad(s, i == b.active));
                dot
            })
            .collect();

        s.on::<BarActive>(me, |ctx, e| {
            ctx.me().active = e.0;
            apply_active(ctx);
        });
        s.on_theme(me, apply_active);

        Bar {
            dots,
            active: b.active,
        }
    }
}

fn dot_quad(theme: &impl ThemeReader, active: bool) -> Quad {
    let role = if active {
        ColorRole::Primary
    } else {
        ColorRole::OnSurfaceVariant
    };
    Quad::new(theme.color(role)).radius(DOT / 2.0)
}

fn apply_active(ctx: &mut Context<'_, Bar>) {
    let active = ctx.me().active;
    let dots = ctx.me().dots.clone();
    for (i, dot) in dots.into_iter().enumerate() {
        let quad = dot_quad(ctx, i == active);
        if let Some(mut dot) = ctx.at(dot) {
            dot.set_paint(Paint::Quad(quad));
        }
    }
}
