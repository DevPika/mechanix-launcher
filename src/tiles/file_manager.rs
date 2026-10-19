use mecha_wayland::prelude::*;

use super::build_tile;

pub struct FileManager {
    pub icon: Handle<Icon>,
}

pub struct FileManagerBuilder;

impl Build for FileManagerBuilder {
    type Widget = FileManager;
}

impl Widget for FileManager {
    type Builder = FileManagerBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let icon = build_tile(me, s, include_bytes!("../../assets/file-manager.svg"));
        FileManager { icon }
    }
}
