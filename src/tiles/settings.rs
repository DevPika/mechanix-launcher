use mecha_wayland::prelude::*;

use super::build_tile;

pub struct Settings {
    pub icon: Handle<Icon>,
}

pub struct SettingsBuilder;

impl Build for SettingsBuilder {
    type Widget = Settings;
}

impl Widget for Settings {
    type Builder = SettingsBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let icon = build_tile(me, s, include_bytes!("../../assets/settings.svg"));
        Settings { icon }
    }
}
