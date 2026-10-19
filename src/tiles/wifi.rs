use mecha_wayland::prelude::*;

use super::{build_label, build_tile};

pub struct Wifi {
    pub icon: Handle<Icon>,
    pub name: Handle<Text>,
}

pub struct WifiBuilder {
    pub font: FontId,
    pub name: String,
}

impl Build for WifiBuilder {
    type Widget = Wifi;
}

impl Widget for Wifi {
    type Builder = WifiBuilder;

    fn build(b: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let icon = build_tile(me, s, include_bytes!("../../assets/wifi/wifi-high.svg"));
        let name = build_label(me, s, b.font, b.name);
        Wifi { icon, name }
    }
}
