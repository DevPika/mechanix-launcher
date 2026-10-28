use mecha_wayland::mechanix_widgets::prelude::{Text, text};
use mecha_wayland::prelude::*;

use crate::util::load_icon;

pub mod battery;
pub mod bluetooth;
pub mod camera;
pub mod cellular;
pub mod display;
pub mod file_manager;
pub mod microphone;
pub mod record;
pub mod rotate;
pub mod settings;
pub mod terminal;
pub mod wifi;

pub use battery::{Battery, BatteryBuilder};
pub use bluetooth::{Bluetooth, BluetoothBuilder};
pub use camera::CameraBuilder;
pub use display::DisplayTileBuilder;
pub use microphone::MicrophoneBuilder;
pub use record::RecordBuilder;
pub use rotate::RotateBuilder;
pub use settings::SettingsBuilder;
pub use terminal::TerminalBuilder;
pub use wifi::WifiBuilder;

pub(crate) const TILE_HEIGHT: f32 = 106.0;

/// Shared appearance only; each tile owns its children and future behavior.
pub(super) fn build_tile<W: Widget>(
    me: Handle<W>,
    s: &mut Spawner<'_, W>,
    svg: &[u8],
) -> Handle<Icon> {
    *s.component_mut::<LayoutStyle>(me).unwrap() = LayoutStyle::default()
        .height(px(TILE_HEIGHT))
        .flex_basis(px(0.0))
        .grow(1.0)
        .shrink(0.0)
        .min_width(px(0.0))
        .max_width(percent(20.0))
        .column()
        .center()
        .gap(px(12.0));
    *s.component_mut::<Paint>(me).unwrap() = Paint::Quad(
        Quad::new(Color::TRANSPARENT).border(1.0, s.color(ColorRole::SecondaryFixedDim)),
    );
    let sprite = load_icon(&mut s.resource_mut::<Atlas>(), svg);
    s.spawn(
        me,
        icon(sprite)
            .style(LayoutStyle::default().shrink(0.0))
            .color(s.color(ColorRole::OnSurface)),
    )
}

pub(super) fn build_label<W: Widget>(
    me: Handle<W>,
    s: &mut Spawner<'_, W>,
    value: impl Into<String>,
) -> Handle<Text> {
    s.spawn(
        me,
        text(value)
            .size(18.0)
            .style(LayoutStyle::default().shrink(0.0))
            .color(ColorRole::OnSurfaceVariant),
    )
}
