use mecha_wayland::prelude::*;

use super::build_tile;

pub struct Rotate {
    pub icon: Handle<Icon>,
}

pub struct RotateBuilder;

impl Build for RotateBuilder {
    type Widget = Rotate;
}

impl Widget for Rotate {
    type Builder = RotateBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let icon = build_tile(me, s, include_bytes!("../../assets/rotate.svg"));
        Rotate { icon }
    }
}
