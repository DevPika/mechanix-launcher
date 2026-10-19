use mecha_wayland::prelude::*;

use super::build_tile;

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
        let icon = build_tile(
            me,
            s,
            include_bytes!("../../assets/cellular/cell-signal-full.svg"),
        );
        Cellular { icon }
    }
}
