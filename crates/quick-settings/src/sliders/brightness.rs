use mecha_wayland::mechanix_widgets::prelude::{Slider, SliderChange, SliderContextExt, slider};
use mecha_wayland::prelude::*;

use crate::util::load_icon;
use services::{BrightnessChanged, MAX_BRIGHTNESS, MIN_BRIGHTNESS, SetBrightness};

mod svg {
    pub const MEDIUM: &[u8] = include_bytes!("../../assets/brightness/brightness-medium.svg");
}

const PADDING_VERTICAL: f32 = 12.0;
const PADDING_HORIZONTAL: f32 = 32.0;

pub struct BrightnessSlider {
    pub slider: Handle<Slider>,
}

pub struct BrightnessSliderBuilder;

impl Build for BrightnessSliderBuilder {
    type Widget = BrightnessSlider;
}

impl Widget for BrightnessSlider {
    type Builder = BrightnessSliderBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let icon_sprite = load_icon(&mut s.resource_mut::<Atlas>(), svg::MEDIUM);

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
                .value(MIN_BRIGHTNESS)
                .range(MIN_BRIGHTNESS, MAX_BRIGHTNESS)
                .steps(68)
                .tick_height(40.0)
                .tick_gap(4.0),
        );

        s.on::<SliderChange>(slider_handle, |ctx, event| {
            ctx.signal(SetBrightness(event.value));
        });

        s.on::<BrightnessChanged>(me, |ctx, event| {
            let slider = ctx.me().slider;
            if let Some(mut slider) = ctx.at(slider) {
                slider.set_value(event.value);
            }
        });

        BrightnessSlider {
            slider: slider_handle,
        }
    }
}
