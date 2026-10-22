use mecha_wayland::prelude::*;

use super::{build_label, build_tile};
use crate::services::{BluetoothState, BluetoothStateChanged, ToggleBluetooth};
use crate::util::load_icon;

#[allow(dead_code)]
mod svg {
    pub const ON: &[u8] = include_bytes!("../../assets/bluetooth/bluetooth.svg");
    pub const CONNECTED: &[u8] = include_bytes!("../../assets/bluetooth/bluetooth-connected.svg");
    pub const SLASH: &[u8] = include_bytes!("../../assets/bluetooth/bluetooth-slash.svg");
    pub const X: &[u8] = include_bytes!("../../assets/bluetooth/bluetooth-x.svg");
}

pub struct Bluetooth {
    pub icon: Handle<Icon>,
    pub name: Handle<Text>,
    pub dot: Handle<Div>,
    pub icon_on: SpriteId,
    pub icon_off: SpriteId,
    pub icon_connected: SpriteId,
    pub label: String,
}

pub struct BluetoothBuilder {
    pub font: FontId,
    pub name: String,
}

impl Build for BluetoothBuilder {
    type Widget = Bluetooth;
}

impl Widget for Bluetooth {
    type Builder = BluetoothBuilder;

    fn build(b: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let icon_on = load_icon(&mut s.resource_mut::<Atlas>(), svg::ON);
        let icon_off = load_icon(&mut s.resource_mut::<Atlas>(), svg::SLASH);
        let icon_connected = load_icon(&mut s.resource_mut::<Atlas>(), svg::CONNECTED);

        let icon = build_tile(me, s, svg::SLASH);
        let name = build_label(me, s, b.font, "Off");
        let dot = s.spawn(
            me,
            div().style(
                LayoutStyle::default()
                    .size(px(5.0), px(5.0))
                    .absolute()
                    .inset(Insets::new(px(8.0), px(10.0), auto(), auto())),
            ),
        );

        s.on::<Clicked>(me, |ctx, _| {
            ctx.signal(ToggleBluetooth);
        });

        s.on::<BluetoothStateChanged>(me, |ctx, event| {
            ctx.set_state(event.state);
        });

        Bluetooth {
            icon,
            name,
            dot,
            icon_on,
            icon_off,
            icon_connected,
            label: b.name,
        }
    }
}

pub trait BluetoothContext {
    fn set_state(&mut self, state: BluetoothState);
}

impl BluetoothContext for Context<'_, Bluetooth> {
    fn set_state(&mut self, state: BluetoothState) {
        let (icon_on, icon_off, icon_connected, default_name) = {
            let me = self.me();
            (me.icon_on, me.icon_off, me.icon_connected, me.label.clone())
        };
        let is_on = state != BluetoothState::Off;
        let bg_color = if is_on {
            self.color(ColorRole::Surface)
        } else {
            Color::TRANSPARENT
        };
        let border_color = self.color(ColorRole::SecondaryFixedDim);
        let icon_color = self.color(ColorRole::OnSurface);
        let text_color = self.color(ColorRole::OnSurfaceVariant);
        let dot_paint = if is_on {
            Paint::Quad(Quad::new(self.color(ColorRole::Primary)).radius(5.0))
        } else {
            Paint::None
        };

        self.set_paint(Paint::Quad(Quad::new(bg_color).border(1.0, border_color)));

        let (sprite, text) = match state {
            BluetoothState::Off => (icon_off, "Off".to_string()),
            BluetoothState::Connected => (
                icon_connected,
                if default_name.is_empty() {
                    "Connected".to_string()
                } else {
                    default_name
                },
            ),
            BluetoothState::On | BluetoothState::Disconnected => (icon_on, "On".to_string()),
        };

        let icon_handle = self.me().icon;
        let text_handle = self.me().name;
        let dot_handle = self.me().dot;

        if let Some(mut ic) = self.at(icon_handle) {
            ic.set_sprite(sprite);
            ic.set_color(icon_color);
        }
        if let Some(mut txt) = self.at(text_handle) {
            txt.set_text(text);
            txt.set_color(text_color);
        }
        if let Some(mut dt) = self.at(dot_handle) {
            dt.set_paint(dot_paint);
        }
    }
}
