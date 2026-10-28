use mecha_wayland::prelude::*;

use super::{build_label, build_tile};

#[allow(dead_code)] // Other signal states are ready for service-driven icon changes.
mod svg {
    pub const HIGH: &[u8] = include_bytes!("../../assets/wifi/wifi-high.svg");
    pub const LOW: &[u8] = include_bytes!("../../assets/wifi/wifi-low.svg");
    pub const NONE: &[u8] = include_bytes!("../../assets/wifi/wifi-none.svg");
    pub const SLASH: &[u8] = include_bytes!("../../assets/wifi/wifi-slash.svg");
    pub const X: &[u8] = include_bytes!("../../assets/wifi/wifi-x.svg");
}

pub struct Wifi;

pub struct WifiBuilder {
    pub name: String,
}

impl Build for WifiBuilder {
    type Widget = Wifi;
}

impl Widget for Wifi {
    type Builder = WifiBuilder;

    fn build(b: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        build_tile(me, s, svg::HIGH);
        build_label(me, s, b.name);
        Wifi
    }
}
