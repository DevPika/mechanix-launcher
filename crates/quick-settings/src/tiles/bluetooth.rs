use mecha_wayland::mechanix_widgets::prelude::{Text, TextContextExt, TextOverflow, text};
use mecha_wayland::prelude::*;

use super::build_tile;
use crate::services::{BluetoothState, BluetoothStateChanged, ToggleBluetooth};
use crate::util::load_icon;

mod svg {
    pub const ON: &[u8] = include_bytes!("../../assets/bluetooth/bluetooth.svg");
    pub const CONNECTED: &[u8] = include_bytes!("../../assets/bluetooth/bluetooth-connected.svg");
    pub const SLASH: &[u8] = include_bytes!("../../assets/bluetooth/bluetooth-slash.svg");
}

pub struct Bluetooth {
    pub icon: Handle<Icon>,
    pub name: Handle<Text>,
    pub dot: Handle<Div>,
    pub icon_on: SpriteId,
    pub icon_off: SpriteId,
    pub icon_connected: SpriteId,
}

pub struct BluetoothBuilder;

impl Build for BluetoothBuilder {
    type Widget = Bluetooth;
}

impl Widget for Bluetooth {
    type Builder = BluetoothBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let icon_on = load_icon(&mut s.resource_mut::<Atlas>(), svg::ON);
        let icon_off = load_icon(&mut s.resource_mut::<Atlas>(), svg::SLASH);
        let icon_connected = load_icon(&mut s.resource_mut::<Atlas>(), svg::CONNECTED);

        let icon = build_tile(me, s, svg::SLASH);
        // `short_name` abbreviates device names; ellipsis overflow is a fallback for narrow tiles.
        let name = s.spawn(
            me,
            text("Off")
                .size(18.0)
                .max_lines(1)
                .overflow(TextOverflow::Ellipsis)
                .style(LayoutStyle::default().max_width(percent(100.0)))
                .color(ColorRole::OnSurfaceVariant),
        );
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
            ctx.set_state(event.state.clone());
        });

        Bluetooth {
            icon,
            name,
            dot,
            icon_on,
            icon_off,
            icon_connected,
        }
    }
}

/// Keep short device names intact; abbreviate longer names for the tile.
fn short_name(name: &str) -> String {
    if name.chars().count() <= 5 {
        name.to_string()
    } else {
        format!("{}...", name.chars().take(3).collect::<String>())
    }
}

pub trait BluetoothContext {
    fn set_state(&mut self, state: BluetoothState);
}

impl BluetoothContext for Context<'_, Bluetooth> {
    fn set_state(&mut self, state: BluetoothState) {
        let (icon_on, icon_off, icon_connected) = {
            let me = self.me();
            (me.icon_on, me.icon_off, me.icon_connected)
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
            BluetoothState::Connected { count, name } => (
                icon_connected,
                match name {
                    Some(name) if count == 1 => short_name(&name),
                    _ if count <= 1 => "Connected".to_string(),
                    _ => format!("{count} devices"),
                },
            ),
            BluetoothState::On => (icon_on, "On".to_string()),
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

#[cfg(test)]
mod tests {
    use super::short_name;

    #[test]
    fn device_names_are_shortened_after_five_characters() {
        assert_eq!(short_name("Buds"), "Buds");
        assert_eq!(short_name("Pixel"), "Pixel");
        assert_eq!(short_name("Headphones"), "Hea...");
        assert_eq!(short_name("Ünïcødé"), "Ünï...");
    }
}
