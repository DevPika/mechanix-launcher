use mecha_wayland::prelude::*;

use super::build_tile;

pub struct Record {
    pub icon: Handle<Icon>,
}

pub struct RecordBuilder;

impl Build for RecordBuilder {
    type Widget = Record;
}

impl Widget for Record {
    type Builder = RecordBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let icon = build_tile(me, s, include_bytes!("../../assets/record.svg"));
        Record { icon }
    }
}
