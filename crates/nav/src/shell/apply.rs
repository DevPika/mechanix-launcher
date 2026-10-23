//! The `Nav` resource, and pushing the stack's state onto nodes and windows.

use std::collections::HashMap;

use mecha_wayland::prelude::*;

use crate::core::{AreaId, ContainerId, Stack};

/// A nav window, in spawn order.
pub(crate) struct NavWindow {
    pub(crate) node: NodeId,
    /// The area its touches are tagged with; read by the touch input.
    #[allow(dead_code)]
    pub(crate) area: AreaId,
    /// Every container in this window, children included, in z order.
    pub(crate) containers: Vec<ContainerId>,
    /// Takes no input: gets an empty input region on its first `Resized`.
    pub(crate) no_input: bool,
}

/// The nodes a container was built into.
pub(crate) struct ContainerNodes {
    pub(crate) id: ContainerId,
    /// Index into [`Nav::windows`].
    pub(crate) window: usize,
    /// Holds the screen and any child containers' nodes.
    pub(crate) node: NodeId,
    /// The black cover, the container node's last child.
    pub(crate) cover: NodeId,
}

/// Everything the nav shell owns at run time.
pub(crate) struct Nav {
    pub(crate) stack: Stack,
    pub(crate) windows: Vec<NavWindow>,
    /// In z order.
    pub(crate) containers: Vec<ContainerNodes>,
    /// Each nav window's last `Resized` size.
    pub(crate) sizes: HashMap<NodeId, Size>,
}
impl Resource for Nav {}

impl Nav {
    pub(crate) fn container(&self, id: ContainerId) -> &ContainerNodes {
        self.containers
            .iter()
            .find(|c| c.id == id)
            .unwrap_or_else(|| panic!("unknown container {id:?}"))
    }
}

/// The cover's paint at `alpha`: blended, so the content shows through.
pub(crate) fn cover_paint(alpha: f32) -> Paint {
    Paint::Quad(Quad {
        is_opaque: false,
        ..Quad::new(Color::BLACK.with_alpha(alpha))
    })
}

/// Pushes the stack's state onto every container node, cover and window,
/// and asks for a frame on each window with an animating container.
pub(crate) fn apply(app: &mut App) {
    let (containers, windows, frames) = {
        let nav = app.resource::<Nav>();
        let containers: Vec<_> = nav
            .containers
            .iter()
            .map(|c| {
                let display = if nav.stack.state(c.id).visible() {
                    Display::Flex
                } else {
                    Display::Hidden
                };
                (c.node, c.cover, display, nav.stack.visual(c.id).cover)
            })
            .collect();
        // Windows without containers (the strips) are never touched.
        let windows: Vec<_> = nav
            .windows
            .iter()
            .filter(|w| !w.containers.is_empty())
            .map(|w| {
                let mapped = w.containers.iter().any(|id| nav.stack.state(*id).visible());
                (w.node, mapped)
            })
            .collect();
        let mut frames: Vec<NodeId> = Vec::new();
        for id in nav.stack.animating() {
            let window = nav.windows[nav.container(id).window].node;
            if !frames.contains(&window) {
                frames.push(window);
            }
        }
        (containers, windows, frames)
    };

    for (node, cover, display, alpha) in containers {
        if app.component::<LayoutStyle>(node).unwrap().display != display {
            app.component_mut::<LayoutStyle>(node).unwrap().display = display;
        }
        let paint = cover_paint(alpha);
        if *app.component::<Paint>(cover).unwrap() != paint {
            *app.component_mut::<Paint>(cover).unwrap() = paint;
        }
    }

    if app.has_resource::<Surfaces>() {
        let (mut surfaces, mut wl) = app.query::<(ResMut<Surfaces>, ResMut<Wayland>)>();
        for (window, mapped) in windows {
            // A window spawned this tick has no entry yet and reads as
            // unmapped, so hiding is always sent (a no-op once hidden):
            // that is what makes a fresh window start hidden.
            if !mapped || !surfaces.is_mapped(window) {
                surfaces.set_mapped(window, mapped, &mut wl);
            }
        }
    }

    for window in frames {
        app.signal(RequestFrame(window));
    }
}

/// Records a nav window's size and, on a `no_input` window's first
/// resize, gives it an empty input region.
pub(crate) fn on_resized(app: &mut App, e: &Emitted<Resized>) {
    let mut fresh_no_input = Vec::new();
    {
        let mut nav = app.resource_mut::<Nav>();
        for &target in e.targets.iter() {
            let Some(no_input) = nav
                .windows
                .iter()
                .find(|w| w.node == target)
                .map(|w| w.no_input)
            else {
                continue;
            };
            let first = nav.sizes.insert(target, e.event.size).is_none();
            if first && no_input {
                fresh_no_input.push(target);
            }
        }
    }
    if fresh_no_input.is_empty() || !app.has_resource::<Surfaces>() {
        return;
    }
    let (mut surfaces, mut wl) = app.query::<(ResMut<Surfaces>, ResMut<Wayland>)>();
    for window in fresh_no_input {
        surfaces.set_input_region(window, Some(&[]), &mut wl);
    }
}
