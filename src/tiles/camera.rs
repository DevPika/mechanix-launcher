use mecha_wayland::prelude::*;

use super::build_tile;

pub struct Camera {
    pub icon: Handle<Icon>,
}

pub struct CameraBuilder;

impl Build for CameraBuilder {
    type Widget = Camera;
}

impl Widget for Camera {
    type Builder = CameraBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let icon = build_tile(me, s, include_bytes!("../../assets/camera.svg"));
        Camera { icon }
    }
}
