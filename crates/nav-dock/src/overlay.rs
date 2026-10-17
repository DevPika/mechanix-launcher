//! The overlay: a full-screen Top-layer window holding every section page
//! in one row, slid sideways to page and down to close.

use std::time::Duration;

use mecha_wayland::prelude::*;
use nav_dock_api::Section;

use crate::config::NavDockConfig;
use crate::state::OverlayView;
use crate::windows::eased_layout;

pub(crate) const NAMESPACE: &str = "mechanix-nav-overlay";

/// New paging position for the row. Emitted at the row node. `dragging`
/// means a finger is on the overlay: the row then follows it unanimated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OverlayUpdate {
    pub view: OverlayView,
    pub dragging: bool,
}
impl Event for OverlayUpdate {}

/// Spawns the overlay window and builds every page once, each into a
/// fixed-size cell of `size` (the screen, until the window's own
/// `Resized` says otherwise).
pub(crate) fn spawn_overlay(
    app: &mut App,
    config: &NavDockConfig,
    sections: &[Box<dyn Section>],
    size: Size,
    view: OverlayView,
) -> NodeId {
    let role = Role::Layer(LayerRole {
        layer: Layer::Top,
        anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
        exclusive_zone: 0,
        namespace: NAMESPACE.into(),
        keyboard_interactivity: KeyboardInteractivity::None,
    });
    let root = app.root();
    let win = app
        .spawn_with(
            root,
            window()
                .title(NAMESPACE)
                .layout(LayoutStyle::default())
                .clear(Color::BLACK),
            (role,),
        )
        .id();
    let row = app.spawn(
        win,
        OverlayRowBuilder {
            window: win,
            size,
            view,
            anim: config.anim,
        },
    );
    let cells = sections
        .iter()
        .map(|section| {
            let cell = app.spawn(row, div().style(cell_style(size)));
            section.spawn_page(app, cell.id());
            cell
        })
        .collect();
    app.widget_mut::<OverlayRow>(row)
        .expect("row was just spawned")
        .cells = cells;
    win
}

/// The row of pages. Its `left` is `-(page * width) + dx` and its `top`
/// is `dy`; the window edge clips whatever is off screen.
pub(crate) struct OverlayRow {
    cells: Vec<Handle<Div>>,
    size: Size,
    view: OverlayView,
    /// The last update applied.
    applied: Option<OverlayUpdate>,
    anim: Duration,
}

pub(crate) struct OverlayRowBuilder {
    window: NodeId,
    size: Size,
    view: OverlayView,
    anim: Duration,
}

impl Build for OverlayRowBuilder {
    type Widget = OverlayRow;
}

impl Widget for OverlayRow {
    type Builder = OverlayRowBuilder;

    fn build(b: OverlayRowBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = LayoutStyle::default().row().absolute();

        s.on::<OverlayUpdate>(me, |ctx, e| {
            // Reconcile re-sends an unchanged position after every touch
            // event; skip those so they cannot re-arm easing.
            let row = ctx.me();
            if row.applied == Some(*e) {
                return;
            }
            row.applied = Some(*e);
            row.view = e.view;
            apply(ctx, !e.dragging);
        });
        // A size change never slides the pages; only paging animates.
        s.on::<Resized>(b.window, |ctx, r| {
            ctx.me().size = r.size;
            apply(ctx, false);
        });

        OverlayRow {
            cells: Vec::new(),
            size: b.size,
            view: b.view,
            applied: None,
            anim: b.anim,
        }
    }
}

fn cell_style(size: Size) -> LayoutStyle {
    LayoutStyle::default()
        .size(px(size.width), px(size.height))
        .shrink(0.0)
}

/// Writes the row's position, size and animation (eased when `animate`,
/// snapped otherwise), and every cell's size.
fn apply(ctx: &mut Context<'_, OverlayRow>, animate: bool) {
    let (cells, size, view, anim) = {
        let row = ctx.me();
        (row.cells.clone(), row.size, row.view, row.anim)
    };
    let left = -(view.page as f32 * size.width) + view.dx;
    let mut style = ctx.style().clone();
    style.inset = Insets::new(px(view.dy), auto(), auto(), px(left));
    style.width = px(size.width * cells.len() as f32);
    style.height = px(size.height);
    ctx.set_style(style);
    ctx.set_layout_animation(eased_layout(if animate { anim } else { Duration::ZERO }));
    for cell in cells {
        if let Some(mut cell) = ctx.at(cell) {
            cell.set_style(cell_style(size));
        }
    }
}
