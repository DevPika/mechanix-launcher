//! The keyboard's exclusive zone: off, the shown keyboard sits over apps; on,
//! it reserves its height so apps shrink above it. Flipped by a
//! `toggle_exclusive_zone` key, which shows the on state in its latched
//! colours.

use mecha_wayland::prelude::*;

use crate::latch::repaint_function_key;
use crate::widget::keyboard::Keyboard;
use crate::widget::surface::KeyboardSurface;

/// Whether the keyboard reserves its height. Starts off.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ExclusiveZone {
    pub(crate) on: bool,
}
impl Resource for ExclusiveZone {}

impl ExclusiveZone {
    /// The layer surface's exclusive zone for a keyboard `height` tall: `-1`
    /// (overlap everything) when off.
    pub(crate) fn zone(self, height: f32) -> i32 {
        if self.on { height as i32 } else { -1 }
    }
}

/// Flip the zone, send it to the surface and repaint every zone key.
pub(crate) fn toggle_exclusive_zone(ctx: &mut Context<'_, Keyboard>) {
    let on = !ctx.resource::<ExclusiveZone>().on;
    ctx.resource_mut::<ExclusiveZone>().on = on;
    let surface = *ctx.resource::<KeyboardSurface>();
    {
        let (mut surfaces, mut wl) = ctx.fetch::<(ResMut<Surfaces>, ResMut<Wayland>)>();
        let zone = ExclusiveZone { on }.zone(surface.height);
        surfaces.set_exclusive_zone(surface.window, zone, &mut wl);
    }
    let radius = ctx.me().radius;
    for k in ctx.me().zone_keys.clone() {
        repaint_function_key(ctx, k, on, radius);
    }
    tracing::info!(on, "keyboard exclusive zone");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_overlaps_and_on_reserves_the_height() {
        assert_eq!(ExclusiveZone { on: false }.zone(252.0), -1);
        assert_eq!(ExclusiveZone { on: true }.zone(252.0), 252);
    }
}
