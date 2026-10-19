use mecha_wayland::prelude::*;

use super::build_tile;

pub struct DisplayTile {
    pub icon: Handle<Icon>,
}

pub struct DisplayTileBuilder;

impl Build for DisplayTileBuilder {
    type Widget = DisplayTile;
}

impl Widget for DisplayTile {
    type Builder = DisplayTileBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        // Display artwork is not available yet.
        let icon = build_tile(me, s, include_bytes!("../../assets/airplane.svg"));
        DisplayTile { icon }
    }
}
