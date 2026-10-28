use mecha_wayland::prelude::*;

mod brightness;
mod volume;

use brightness::BrightnessSliderBuilder;
use volume::VolumeSliderBuilder;

pub const SLIDERS_HEIGHT: f32 = 203.0;

pub struct Sliders;

pub struct SlidersBuilder;

impl Build for SlidersBuilder {
    type Widget = Sliders;
}

impl Widget for Sliders {
    type Builder = SlidersBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let col = s.spawn(
            me,
            div().style(
                LayoutStyle::default()
                    .column()
                    .center()
                    .gap(px(Spacing::Space200.dp())),
            ),
        );
        s.spawn(col, VolumeSliderBuilder);
        s.spawn(col, BrightnessSliderBuilder);

        Sliders
    }
}
