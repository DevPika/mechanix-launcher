use mecha_wayland::prelude::*;

use super::build_slider;
use crate::util::load_icon;

mod svg {
    pub const HIGH: &[u8] = include_bytes!("../../assets/speaker/speaker-high.svg");
}

const TOP: f32 = 47.0;
const LEVEL_PERCENT: f32 = 45.0;

pub struct VolumeSlider;

pub struct VolumeSliderBuilder;

impl Build for VolumeSliderBuilder {
    type Widget = VolumeSlider;
}

impl Widget for VolumeSlider {
    type Builder = VolumeSliderBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let icon = load_icon(&mut s.resource_mut::<Atlas>(), svg::HIGH);
        build_slider(me, s, TOP, LEVEL_PERCENT, icon);
        VolumeSlider
    }
}
