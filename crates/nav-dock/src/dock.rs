//! `DockMorph`: the one element that is the expanded dock (a pill of
//! section icons) and the minimized bar (a black pill of dots), morphing
//! between the two inside the overlay window. Its look is a continuous
//! `t`: 0 is the bar, 1 the dock, and a bar drag holds it in between.

use std::time::Duration;

use mecha_wayland::prelude::*;

use crate::config::NavDockConfig;
use crate::state::Mode;
use crate::windows::{eased_layout, eased_paint};

/// Alpha of the expanded pill's `SurfaceContainerHighest` fill.
const PILL_ALPHA: f32 = 0.2;
/// Side of the box behind an icon. Emphasis is only the box's fill, which
/// fades through paint animation: a size change would snap while a finger
/// holds the morph.
const ICON_BOX: f32 = 44.0;
/// Alpha of the `Primary` box behind the emphasised icon.
const EMPHASIS_ALPHA: f32 = 0.24;
/// Diameter of a bar dot.
const DOT: f32 = 4.0;

/// Which of its two shapes the morph takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Look {
    /// `dock_size`, icons shown, dots at size 0.
    Expanded,
    /// `bar_size`, black, icons at size 0, dots shown.
    Bar,
}

impl Look {
    /// The look a mode shows, and the section it emphasises: the hovered
    /// one while dragging, the active one otherwise. `None` while Hidden.
    pub(crate) fn of(mode: Mode) -> Option<(Look, usize)> {
        match mode {
            Mode::Dragging { hover } => Some((Look::Expanded, hover)),
            Mode::Idle { active } => Some((Look::Expanded, active)),
            Mode::Minimized { active } => Some((Look::Bar, active)),
            Mode::Hidden => None,
        }
    }

    /// The look's `t`: 1 expanded, 0 the bar.
    pub(crate) fn t(self) -> f32 {
        match self {
            Look::Expanded => 1.0,
            Look::Bar => 0.0,
        }
    }

    /// The pill's (width, height).
    fn size(self, config: &NavDockConfig) -> (f32, f32) {
        pill_size(config, self.t())
    }

    /// Empty overlay below the pill.
    pub(crate) fn gap(self, config: &NavDockConfig) -> f32 {
        match self {
            Look::Expanded => config.dock_bottom_gap,
            Look::Bar => config.bar_bottom_gap,
        }
    }

    /// Where the pill settles in an overlay of `overlay` size, in the
    /// overlay's coordinates: horizontally centred, `gap` above the bottom.
    pub(crate) fn target_rect(self, config: &NavDockConfig, overlay: Size) -> Rect {
        let (width, height) = self.size(config);
        Rect::new(
            (overlay.width - width) / 2.0,
            overlay.height - self.gap(config) - height,
            width,
            height,
        )
    }
}

/// Show the look `t` (0 the bar, 1 the dock), emphasising section `focus`
/// (keep the current ones when `None`), with the bottom edge `bottom` above
/// an overlay of `overlay` size, reached under `settings`; `snap` when a
/// finger drives it, so its colours follow at once. Emitted at the morph
/// node.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MorphUpdate {
    pub t: Option<f32>,
    pub focus: Option<usize>,
    pub bottom: f32,
    pub overlay: Size,
    pub settings: LayoutAnimationSettings,
    pub snap: bool,
}
impl Event for MorphUpdate {}

/// Redraw each icon's sprite at its displayed box. Emitted at the morph
/// node on every overlay `Frame`, after layout animation has advanced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScaleIcons;
impl Event for ScaleIcons {}

/// The dock/bar element: one equal-width slot per section, so icon `i` is
/// centred where `NavState::icon_center_x(i)` expects it. Each slot holds
/// the icon (in its emphasis box) and the dot; the look's `t` shares size
/// between the two, so a change of look grows one and shrinks the other.
pub(crate) struct DockMorph {
    /// Per slot: the emphasis box, the icon inside it, and the dot.
    cells: Vec<Cell>,
    /// The icons' natural side.
    icon_side: f32,
    config: NavDockConfig,
    /// 0 the bar, 1 the dock.
    t: f32,
    focus: usize,
    /// Height of the bottom edge above the overlay's.
    bottom: f32,
    /// The overlay size `bottom` was set for: a resize re-applies it.
    overlay: Option<Size>,
}

#[derive(Clone, Copy)]
struct Cell {
    hl: Handle<Div>,
    icon: Handle<Icon>,
    dot: Handle<Div>,
}

#[cfg(test)]
impl DockMorph {
    pub(crate) fn look(&self) -> (f32, usize) {
        (self.t, self.focus)
    }
}

pub(crate) struct DockMorphBuilder {
    pub(crate) icons: Vec<SpriteId>,
    pub(crate) config: NavDockConfig,
    pub(crate) look: Look,
    pub(crate) focus: usize,
    pub(crate) bottom: f32,
}

impl Build for DockMorphBuilder {
    type Widget = DockMorph;
}

impl Widget for DockMorph {
    type Builder = DockMorphBuilder;

    fn build(b: DockMorphBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let t = b.look.t();
        *s.component_mut::<LayoutStyle>(me).unwrap() = pill_style(&b.config, t, b.bottom);
        *s.component_mut::<Paint>(me).unwrap() = Paint::Quad(pill_quad(s, &b.config, t));
        *s.component_mut::<LayoutAnimationSettings>(me).unwrap() = eased_layout(b.config.anim);
        *s.component_mut::<PaintAnimationSettings>(me).unwrap() = eased_paint(b.config.anim);

        let icon_side = b
            .icons
            .first()
            .map_or(0.0, |&id| s.resource::<Atlas>().sprite(id).size.width);
        let slot_width = 100.0 / b.icons.len().max(1) as f32;
        let cells = b
            .icons
            .iter()
            .enumerate()
            .map(|(i, &sprite)| {
                let l = CellLook::of(s, t, i == b.focus, icon_side);
                let slot = s.spawn(
                    me,
                    div().style(
                        LayoutStyle::default()
                            .center()
                            .width(percent(slot_width))
                            .height(percent(100.0)),
                    ),
                );
                let hl = s.spawn(slot, div().style(l.hl_style));
                *s.component_mut::<Paint>(hl).unwrap() = Paint::Quad(l.hl_quad);
                let icon = s.spawn(hl, icon(sprite).style(l.icon_style).color(l.tint));
                let dot = s.spawn(slot, div().style(l.dot_style));
                *s.component_mut::<Paint>(dot).unwrap() = Paint::Quad(l.dot_quad);
                Cell { hl, icon, dot }
            })
            .collect();

        s.on::<MorphUpdate>(me, |ctx, e| {
            // Unchanged updates are skipped, so their settings cannot cut
            // short or re-arm a running transition.
            let morph = ctx.me();
            let t = e.t.unwrap_or(morph.t);
            let focus = e.focus.unwrap_or(morph.focus);
            let next = (t, focus, e.bottom, Some(e.overlay));
            if (morph.t, morph.focus, morph.bottom, morph.overlay) == next {
                return;
            }
            (morph.t, morph.focus, morph.bottom, morph.overlay) = next;
            ctx.set_layout_animation(e.settings);
            // A finger-driven update sets the interpolated colours at once;
            // a timed one fades them over `anim`.
            let paint = if e.snap {
                Duration::ZERO
            } else {
                ctx.me().config.anim
            };
            ctx.set_paint_animation(eased_paint(paint));
            apply(ctx);
        });
        s.on_theme(me, apply);
        s.on::<ScaleIcons>(me, |ctx, _| scale_icons(ctx));

        DockMorph {
            cells,
            icon_side,
            config: b.config,
            t,
            focus: b.focus,
            bottom: b.bottom,
            overlay: None,
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: lerp(a.r, b.r, t),
        g: lerp(a.g, b.g, t),
        b: lerp(a.b, b.b, t),
        a: lerp(a.a, b.a, t),
    }
}

/// The pill's (width, height) at look `t`: the bar's at 0, the dock's at 1.
fn pill_size(config: &NavDockConfig, t: f32) -> (f32, f32) {
    (
        lerp(config.bar_size.0, config.dock_size.0, t),
        lerp(config.bar_size.1, config.dock_size.1, t),
    )
}

/// The pill's style at look `t`: absolute in the overlay, horizontally
/// centred, its bottom edge `bottom` above the overlay's.
fn pill_style(config: &NavDockConfig, t: f32, bottom: f32) -> LayoutStyle {
    let (width, height) = pill_size(config, t);
    LayoutStyle::default()
        .row()
        .align_items(Align::Center)
        .absolute()
        .size(px(width), px(height))
        .inset(Insets::new(auto(), auto(), px(bottom), percent(50.0)))
        .margin(Insets::new(px(0.0), px(0.0), px(0.0), px(-width / 2.0)))
}

fn pill_quad(theme: &impl ThemeReader, config: &NavDockConfig, t: f32) -> Quad {
    let (_, height) = pill_size(config, t);
    let dock = theme
        .color(ColorRole::SurfaceContainerHighest)
        .with_alpha(PILL_ALPHA);
    Quad::new(lerp_color(Color::BLACK, dock, t)).radius(height / 2.0)
}

/// Writes the pill and every slot for the current look and focus.
fn apply(ctx: &mut Context<'_, DockMorph>) {
    let (cells, icon_side, config, t, focus, bottom) = {
        let m = ctx.me();
        (
            m.cells.clone(),
            m.icon_side,
            m.config.clone(),
            m.t,
            m.focus,
            m.bottom,
        )
    };
    ctx.set_style(pill_style(&config, t, bottom));
    let quad = pill_quad(ctx, &config, t);
    ctx.set_paint(Paint::Quad(quad));

    for (i, cell) in cells.into_iter().enumerate() {
        let l = CellLook::of(ctx, t, i == focus, icon_side);
        if let Some(mut hl) = ctx.at(cell.hl) {
            hl.set_style(l.hl_style);
            hl.set_paint(Paint::Quad(l.hl_quad));
        }
        if let Some(mut ic) = ctx.at(cell.icon) {
            ic.set_style(l.icon_style);
            ic.set_color(l.tint);
        }
        if let Some(mut dot) = ctx.at(cell.dot) {
            dot.set_style(l.dot_style);
            dot.set_paint(Paint::Quad(l.dot_quad));
        }
    }
}

/// Sizes every icon's sprite to its displayed box, so it shrinks and grows
/// with the layout animation instead of drawing full size until its box
/// is empty. `set_size` does nothing when the size is unchanged.
fn scale_icons(ctx: &mut Context<'_, DockMorph>) {
    let cells = ctx.me().cells.clone();
    for cell in cells {
        let Some(mut icon) = ctx.at(cell.icon) else {
            continue;
        };
        let Some(box_) = icon.component::<Layout>().map(Layout::content) else {
            continue;
        };
        let side = box_.width().min(box_.height()).max(0.0);
        icon.set_size(Size::new(side, side));
    }
}

/// One slot at look `t`. At 1 (expanded): the icon box and icon at size,
/// the dot at 0. At 0 (the bar): the other way round; in between, both
/// scale linearly. Sprite paint cannot fade, so icons go
/// and dots come through their layout size; an icon's sprite follows its
/// displayed box through `ScaleIcons`, since a sprite draws at its own
/// size whatever its box.
struct CellLook {
    hl_style: LayoutStyle,
    hl_quad: Quad,
    icon_style: LayoutStyle,
    tint: Color,
    dot_style: LayoutStyle,
    dot_quad: Quad,
}

impl CellLook {
    fn of(theme: &impl ThemeReader, t: f32, emphasised: bool, icon_side: f32) -> Self {
        let mut icon = IconLook::of(theme, emphasised);
        icon.side *= t;
        let (icon_side, dot_side) = (icon_side * t, DOT * (1.0 - t));
        Self {
            hl_style: icon.box_style(),
            hl_quad: icon.box_quad(),
            icon_style: square(icon_side),
            tint: icon.tint,
            dot_style: square(dot_side),
            dot_quad: dot_quad(theme, emphasised),
        }
    }
}

fn square(side: f32) -> LayoutStyle {
    LayoutStyle::default().size(px(side), px(side)).shrink(0.0)
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
                side: ICON_BOX,
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
        square(self.side).center()
    }

    fn box_quad(&self) -> Quad {
        Quad::new(self.fill).radius(self.side / 2.0)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halfway_morph_is_halfway_wide() {
        let config = NavDockConfig::default();
        let (width, height) = pill_size(&config, 0.5);
        assert_eq!(width, (config.bar_size.0 + config.dock_size.0) / 2.0);
        assert_eq!(height, (config.bar_size.1 + config.dock_size.1) / 2.0);
        assert_eq!(pill_size(&config, 0.0), config.bar_size);
        assert_eq!(pill_size(&config, 1.0), config.dock_size);
    }
}
