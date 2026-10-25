use mecha_wayland::mechanix_widgets::prelude::{SliderChange, slider};
use mecha_wayland::prelude::*;

use crate::util::load_icon;

mod svg {
    pub const HIGH: &[u8] = include_bytes!("../../assets/speaker/speaker-high.svg");
}

const LEVEL_PERCENT: f32 = 70.0;
const PADDING_VERTICAL: f32 = 12.0;
const PADDING_HORIZONTAL: f32 = 32.0;

pub struct VolumeSlider;

pub struct VolumeSliderBuilder;

impl Build for VolumeSliderBuilder {
    type Widget = VolumeSlider;
}

impl Widget for VolumeSlider {
    type Builder = VolumeSliderBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let icon_sprite = load_icon(&mut s.resource_mut::<Atlas>(), svg::HIGH);

        let row = s.spawn(
            me,
            div().style(
                LayoutStyle::default()
                    .row()
                    .center()
                    .gap(px(Spacing::Space200.dp()))
                    .padding(Insets::symmetric(
                        px(PADDING_HORIZONTAL),
                        px(PADDING_VERTICAL),
                    )),
            ),
        );

        s.spawn(
            row,
            icon(icon_sprite)
                .style(LayoutStyle::default().shrink(0.0))
                .color(s.color(ColorRole::OnSurface)),
        );

        let slider_handle = s.spawn(
            row,
            slider()
                .value(LEVEL_PERCENT)
                .range(0.0, 100.0)
                .steps(68)
                .tick_height(40.0)
                .tick_gap(4.0),
        );

        s.on::<SliderChange>(slider_handle, |_ctx, e| {
            println!(
                "[volume] slider changed: value = {:.2}, step_index = {}",
                e.value, e.step_index
            );
        });

        VolumeSlider
    }
}
