use mecha_wayland::prelude::*;

use super::build_tile;

mod svg {
    pub const ICON: &[u8] = include_bytes!("../../assets/microphone.svg");
}

pub struct Microphone {
    pub icon: Handle<Icon>,
}

pub struct MicrophoneBuilder;

impl Build for MicrophoneBuilder {
    type Widget = Microphone;
}

impl Widget for Microphone {
    type Builder = MicrophoneBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        // Microphone artwork is not available yet.
        let icon = build_tile(me, s, svg::ICON);
        Microphone { icon }
    }
}
