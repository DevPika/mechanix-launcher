use mecha_wayland::prelude::*;

use crate::util::full_width;

mod brightness;
mod volume;

use brightness::BrightnessSliderBuilder;
use volume::VolumeSliderBuilder;

pub const SLIDERS_HEIGHT: f32 = 203.0;
const SLIDER_HEIGHT: f32 = 40.0;
const SLIDER_LEFT_PERCENT: f32 = 15.0;
const SLIDER_RIGHT_PERCENT: f32 = 6.0;

pub struct Sliders;

pub struct SlidersBuilder;

impl Build for SlidersBuilder {
    type Widget = Sliders;
}

impl Widget for Sliders {
    type Builder = SlidersBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = full_width(SLIDERS_HEIGHT);
        *s.component_mut::<Paint>(me).unwrap() = Paint::Quad(
            Quad::new(s.color(ColorRole::SurfaceContainerLow))
                .border(1.0, s.color(ColorRole::OutlineVariant)),
        );

        s.spawn(me, VolumeSliderBuilder);
        s.spawn(me, BrightnessSliderBuilder);

        Sliders
    }
}

fn build_slider<W: Widget>(
    me: Handle<W>,
    s: &mut Spawner<'_, W>,
    top: f32,
    level: f32,
    sprite: SpriteId,
) {
    *s.component_mut::<LayoutStyle>(me).unwrap() = full_width(SLIDER_HEIGHT)
        .absolute()
        .inset(Insets::new(px(top), auto(), auto(), px(0.0)));

    let icon_box = s.spawn(
        me,
        div().style(
            LayoutStyle::default()
                .size(percent(SLIDER_LEFT_PERCENT), px(SLIDER_HEIGHT))
                .absolute()
                .inset(Insets::new(px(0.0), auto(), auto(), px(0.0)))
                .center(),
        ),
    );
    s.spawn(
        icon_box,
        icon(sprite)
            .style(LayoutStyle::default().shrink(0.0))
            .color(s.color(ColorRole::OnSurface)),
    );

    let track = s.spawn(
        me,
        div()
            .style(
                LayoutStyle::default()
                    .height(px(SLIDER_HEIGHT))
                    .padding_all(px(1.0))
                    .absolute()
                    .inset(Insets::new(
                        px(0.0),
                        percent(SLIDER_RIGHT_PERCENT),
                        auto(),
                        percent(SLIDER_LEFT_PERCENT),
                    )),
            )
            .background(s.color(ColorRole::SurfaceContainerHigh))
            .border(1.0, s.color(ColorRole::OutlineVariant)),
    );
    s.spawn(
        track,
        div()
            .style(
                LayoutStyle::default()
                    .size(percent(level), percent(100.0))
                    .shrink(0.0),
            )
            .background(s.color(ColorRole::Primary)),
    );
}
