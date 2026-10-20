use mecha_wayland::prelude::*;

use crate::{full_width, load_icon};

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
pub use camera::{Camera, CameraBuilder};
pub use cellular::{Cellular, CellularBuilder};
pub use display::{DisplayTile, DisplayTileBuilder};
pub use file_manager::{FileManager, FileManagerBuilder};
pub use microphone::{Microphone, MicrophoneBuilder};
pub use record::{Record, RecordBuilder};
pub use rotate::{Rotate, RotateBuilder};
pub use settings::{Settings, SettingsBuilder};
pub use terminal::{Terminal, TerminalBuilder};
pub use wifi::{Wifi, WifiBuilder};

const TILE_HEIGHT: f32 = 139.0;
const TILE_COLUMNS: usize = 5;
pub const GRID_HEIGHT: f32 = TILE_HEIGHT * 2.0;

/// Individually addressable tiles, in the reference layout's order.
pub struct Tiles {
    pub wifi: Handle<Wifi>,
    pub bluetooth: Handle<Bluetooth>,
    pub display: Handle<DisplayTile>,
    pub terminal: Handle<Terminal>,
    pub microphone: Handle<Microphone>,
    pub record: Handle<Record>,
    pub settings: Handle<Settings>,
    pub camera: Handle<Camera>,
    pub rotate: Handle<Rotate>,
    pub battery: Handle<Battery>,
}

pub struct TilesBuilder {
    pub font: FontId,
}

impl Build for TilesBuilder {
    type Widget = Tiles;
}

impl Widget for Tiles {
    type Builder = TilesBuilder;

    fn build(b: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = full_width(GRID_HEIGHT).column();

        let top = s.spawn(me, div().style(full_width(TILE_HEIGHT).row()));
        let wifi = s.spawn(
            top,
            WifiBuilder {
                font: b.font,
                name: "Office 01".into(),
            },
        );
        let bluetooth = s.spawn(
            top,
            BluetoothBuilder {
                font: b.font,
                name: "Comet".into(),
            },
        );
        let display = s.spawn(top, DisplayTileBuilder);
        let terminal = s.spawn(top, TerminalBuilder);
        let microphone = s.spawn(top, MicrophoneBuilder);

        let bottom = s.spawn(me, div().style(full_width(TILE_HEIGHT).row()));
        let record = s.spawn(bottom, RecordBuilder);
        let settings = s.spawn(bottom, SettingsBuilder);
        let camera = s.spawn(bottom, CameraBuilder);
        let rotate = s.spawn(bottom, RotateBuilder);
        let battery = s.spawn(
            bottom,
            BatteryBuilder {
                font: b.font,
            },
        );
        // let secondary_display = s.spawn(bottom, DisplayTileBuilder);

        Tiles {
            wifi,
            bluetooth,
            display,
            terminal,
            microphone,
            record,
            settings,
            camera,
            rotate,
            battery,
        }
    }
}

/// Shared appearance only; each tile owns its children and future behavior.
fn build_tile<W: Widget>(me: Handle<W>, s: &mut Spawner<'_, W>, svg: &[u8]) -> Handle<Icon> {
    *s.component_mut::<LayoutStyle>(me).unwrap() = full_width(TILE_HEIGHT)
        .width(percent(100.0 / TILE_COLUMNS as f32))
        .column()
        .center()
        .gap(px(12.0));
    *s.component_mut::<Paint>(me).unwrap() = Paint::Quad(
        Quad::new(s.color(ColorRole::Surface)).border(1.0, s.color(ColorRole::OutlineVariant)),
    );
    let sprite = load_icon(&mut s.resource_mut::<Atlas>(), svg);
    s.spawn(
        me,
        icon(sprite)
            .style(LayoutStyle::default().shrink(0.0))
            .color(s.color(ColorRole::OnSurface)),
    )
}

fn build_label<W: Widget>(
    me: Handle<W>,
    s: &mut Spawner<'_, W>,
    font: FontId,
    value: impl Into<String>,
) -> Handle<Text> {
    s.spawn(
        me,
        text(font, value)
            .size(18)
            .style(LayoutStyle::default().shrink(0.0))
            .color(s.color(ColorRole::OnSurfaceVariant)),
    )
}
