use mecha_wayland::prelude::*;

use super::build_slider;
use crate::util::load_icon;

mod svg {
    pub const MEDIUM: &[u8] = include_bytes!("../../assets/brightness/brightness-medium.svg");
}

const TOP: f32 = 133.0;
const LEVEL_PERCENT: f32 = 70.0;

pub struct BrightnessSlider;

pub struct BrightnessSliderBuilder;

impl Build for BrightnessSliderBuilder {
    type Widget = BrightnessSlider;
}

impl Widget for BrightnessSlider {
    type Builder = BrightnessSliderBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let icon = load_icon(&mut s.resource_mut::<Atlas>(), svg::MEDIUM);
        build_slider(me, s, TOP, LEVEL_PERCENT, icon);
        BrightnessSlider
    }
}
