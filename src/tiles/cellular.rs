use mecha_wayland::prelude::*;

use super::build_tile;

mod svg {
    pub const FULL: &[u8] = include_bytes!("../../assets/cellular/cell-signal-full.svg");
    pub const HIGH: &[u8] = include_bytes!("../../assets/cellular/cell-signal-high.svg");
    pub const MEDIUM: &[u8] = include_bytes!("../../assets/cellular/cell-signal-medium.svg");
    pub const LOW: &[u8] = include_bytes!("../../assets/cellular/cell-signal-low.svg");
    pub const NONE: &[u8] = include_bytes!("../../assets/cellular/cell-signal-none.svg");
    pub const SLASH: &[u8] = include_bytes!("../../assets/cellular/cell-signal-slash.svg");
    pub const X: &[u8] = include_bytes!("../../assets/cellular/cell-signal-x.svg");
}

pub struct Cellular {
    pub icon: Handle<Icon>,
}

pub struct CellularBuilder;

impl Build for CellularBuilder {
    type Widget = Cellular;
}

impl Widget for Cellular {
    type Builder = CellularBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let icon = build_tile(me, s, svg::FULL);
        Cellular { icon }
    }
}
