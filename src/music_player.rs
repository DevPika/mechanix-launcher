use mecha_wayland::prelude::*;

use crate::{MEDIA_HEIGHT, full_width, placeholder};

const ALBUM_SIZE: f32 = 112.0;
const SONG_TITLE: &str = "Faisal";
const ARTIST_NAME: &str = "Fred again..";
const CONTROL_SIZE: f32 = 40.0;
const PLAY_SIZE: f32 = 64.0;
const PROGRESS_HEIGHT: f32 = 3.0;
const PROGRESS_PERCENT: f32 = 42.0;
const THUMB_SIZE: f32 = 14.0;

pub struct MusicPlayer;

pub struct MusicPlayerBuilder {
    pub font: FontId,
    pub placeholder_icon: SpriteId,
}

impl Build for MusicPlayerBuilder {
    type Widget = MusicPlayer;
}

impl Widget for MusicPlayer {
    type Builder = MusicPlayerBuilder;

    fn build(b: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = full_width(MEDIA_HEIGHT)
            .row()
            .align_items(Align::Center)
            .padding(Insets::new(px(24.0), percent(5.0), px(24.0), percent(5.0)))
            .gap(px(24.0));
        *s.component_mut::<Paint>(me).unwrap() = Paint::Quad(
            Quad::new(s.color(ColorRole::SurfaceContainer))
                .border(1.0, s.color(ColorRole::OutlineVariant)),
        );

        let album = s.spawn(
            me,
            placeholder(
                LayoutStyle::default()
                    .size(px(ALBUM_SIZE), px(ALBUM_SIZE))
                    .shrink(0.0)
                    .center(),
                s,
                ColorRole::SurfaceContainerHigh,
            )
            .radius(4.0),
        );
        s.spawn(
            album,
            icon(b.placeholder_icon).color(s.color(ColorRole::OnSurfaceVariant)),
        );

        let details = s.spawn(
            me,
            div().style(
                LayoutStyle::default()
                    .grow(1.0)
                    .min_width(px(0.0))
                    .column()
                    .gap(px(22.0)),
            ),
        );
        let header = s.spawn(
            details,
            div().style(
                full_width(PLAY_SIZE)
                    .row()
                    .align_items(Align::Center)
                    .gap(px(16.0)),
            ),
        );
        let metadata = s.spawn(
            header,
            div().style(
                LayoutStyle::default()
                    .column()
                    .grow(1.0)
                    .min_width(px(0.0))
                    .gap(px(8.0)),
            ),
        );
        s.spawn(
            metadata,
            text(b.font, SONG_TITLE)
                .size(28)
                .color(s.color(ColorRole::OnSurface)),
        );
        s.spawn(
            metadata,
            text(b.font, ARTIST_NAME)
                .size(18)
                .color(s.color(ColorRole::OnSurfaceVariant)),
        );

        let controls = s.spawn(
            header,
            div().style(
                LayoutStyle::default()
                    .height(px(PLAY_SIZE))
                    .shrink(0.0)
                    .row()
                    .center()
                    .gap(px(18.0)),
            ),
        );

        // Previous, play, next. The larger circular button marks play.
        for is_play in [false, true, false] {
            let size = if is_play { PLAY_SIZE } else { CONTROL_SIZE };
            let control = s.spawn(
                controls,
                button()
                    .style(
                        LayoutStyle::default()
                            .size(px(size), px(size))
                            .shrink(0.0)
                            .center(),
                    )
                    .radius(Corners::all(size / 2.0))
                    .border(
                        Insets::all(if is_play { 1.0 } else { 0.0 }),
                        s.color(ColorRole::Outline),
                    )
                    .hover_background(s.color(ColorRole::SurfaceContainerHigh))
                    .pressed_background(s.color(ColorRole::SurfaceContainerHighest)),
            );
            s.spawn(
                control,
                icon(b.placeholder_icon).color(s.color(if is_play {
                    ColorRole::Primary
                } else {
                    ColorRole::OnSurface
                })),
            );
        }

        let progress = s.spawn(
            details,
            div()
                .style(full_width(PROGRESS_HEIGHT))
                .background(s.color(ColorRole::OutlineVariant)),
        );
        s.spawn(
            progress,
            div()
                .style(LayoutStyle::default().size(percent(PROGRESS_PERCENT), percent(100.0)))
                .background(s.color(ColorRole::Primary)),
        );
        s.spawn(
            progress,
            div()
                .style(
                    LayoutStyle::default()
                        .size(px(THUMB_SIZE), px(THUMB_SIZE))
                        .absolute()
                        .inset(Insets::new(
                            px((PROGRESS_HEIGHT - THUMB_SIZE) / 2.0),
                            auto(),
                            auto(),
                            percent(PROGRESS_PERCENT),
                        ))
                        .margin(Insets::new(
                            px(0.0),
                            px(0.0),
                            px(0.0),
                            px(-THUMB_SIZE / 2.0),
                        )),
                )
                .background(s.color(ColorRole::Primary))
                .radius(THUMB_SIZE / 2.0),
        );

        MusicPlayer
    }
}
