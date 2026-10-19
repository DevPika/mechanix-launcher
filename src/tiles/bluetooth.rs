use mecha_wayland::prelude::*;

use super::{build_label, build_tile};

pub struct Bluetooth {
    pub icon: Handle<Icon>,
    pub name: Handle<Text>,
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
        let icon = build_tile(
            me,
            s,
            include_bytes!("../../assets/bluetooth/bluetooth.svg"),
        );
        let name = build_label(me, s, b.font, b.name);
        Bluetooth { icon, name }
    }
}
