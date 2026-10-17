//! The overlay: a full-screen Top-layer window holding every section page
//! in one row, placed by `motion::targets` (slid sideways to page, up and
//! down to reveal, close and exit), and the dock/bar morph drawn above it.

use mecha_wayland::prelude::*;
use nav_dock_api::Section;

use crate::config::NavDockConfig;
use crate::dock::{DockMorphBuilder, Look};
use crate::motion::Targets;

pub(crate) const NAMESPACE: &str = "mechanix-nav-overlay";

/// New position for the row, in an overlay of `size`, reached under
/// `settings`. Emitted at the row node.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OverlayUpdate {
    pub left: f32,
    pub top: f32,
    pub size: Size,
    pub settings: LayoutAnimationSettings,
}
impl Event for OverlayUpdate {}

/// Spawns the overlay window and builds every page once, each into a
/// fixed-size cell of `size` (the screen, until the window's own
/// `Resized` says otherwise), at `targets`. The morph, spawned after the
/// row so it draws above it, starts in `look` emphasising `focus`.
pub(crate) fn spawn_overlay(
    app: &mut App,
    config: &NavDockConfig,
    sections: &[Box<dyn Section>],
    icons: &[SpriteId],
    size: Size,
    targets: Targets,
    (look, focus): (Look, usize),
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
            size,
            left: targets.row_left,
            top: targets.row_top,
        },
    );
    let cells = sections
        .iter()
        .map(|section| {
            let cell = app.spawn(row, div().style(cell_style(size)));
            section.spawn_page(app, cell.id());
            cell
        })
        .collect::<Vec<_>>();
    *app.component_mut::<LayoutStyle>(row).unwrap() =
        row_style(size, targets.row_left, targets.row_top, cells.len());
    app.widget_mut::<OverlayRow>(row)
        .expect("row was just spawned")
        .cells = cells;
    app.spawn(
        win,
        DockMorphBuilder {
            icons: icons.to_vec(),
            config: config.clone(),
            look,
            focus,
            bottom: targets.morph_bottom,
        },
    );
    win
}

/// The overlay's page row and its morph, in that order.
pub(crate) fn parts(app: &App, overlay: NodeId) -> Option<(NodeId, NodeId)> {
    match app.children(overlay)? {
        [row, morph] => Some((*row, *morph)),
        _ => None,
    }
}

/// The row of pages, `left` and `top` in the overlay; the window edge
/// clips whatever is off screen.
pub(crate) struct OverlayRow {
    cells: Vec<Handle<Div>>,
    size: Size,
    left: f32,
    top: f32,
}

pub(crate) struct OverlayRowBuilder {
    size: Size,
    left: f32,
    top: f32,
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
            // event; skip those, so their settings cannot cut short or
            // re-arm a running transition.
            let row = ctx.me();
            if (row.left, row.top, row.size) == (e.left, e.top, e.size) {
                return;
            }
            (row.left, row.top, row.size) = (e.left, e.top, e.size);
            apply(ctx, e.settings);
        });

        OverlayRow {
            cells: Vec::new(),
            size: b.size,
            left: b.left,
            top: b.top,
        }
    }
}

fn row_style(size: Size, left: f32, top: f32, cells: usize) -> LayoutStyle {
    let mut style = LayoutStyle::default().row().absolute();
    style.inset = Insets::new(px(top), auto(), auto(), px(left));
    style.width = px(size.width * cells as f32);
    style.height = px(size.height);
    style
}

fn cell_style(size: Size) -> LayoutStyle {
    LayoutStyle::default()
        .size(px(size.width), px(size.height))
        .shrink(0.0)
}

/// Writes the row's animation, then its position and size, and every
/// cell's size.
fn apply(ctx: &mut Context<'_, OverlayRow>, settings: LayoutAnimationSettings) {
    let (cells, size, left, top) = {
        let row = ctx.me();
        (row.cells.clone(), row.size, row.left, row.top)
    };
    ctx.set_layout_animation(settings);
    ctx.set_style(row_style(size, left, top, cells.len()));
    for cell in cells {
        if let Some(mut cell) = ctx.at(cell) {
            cell.set_style(cell_style(size));
        }
    }
}
