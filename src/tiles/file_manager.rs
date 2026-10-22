use mecha_wayland::prelude::*;

use super::build_tile;

mod svg {
    pub const ICON: &[u8] = include_bytes!("../../assets/file-manager.svg");
}

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
        let icon = build_tile(me, s, svg::ICON);
        FileManager { icon }
    }
}
