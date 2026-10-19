use mecha_wayland::prelude::*;

mod music_player;
pub mod tiles;

use music_player::MusicPlayerBuilder;
use tiles::{GRID_HEIGHT, TilesBuilder};

// Initial window size only; the panel fills the actual window dimensions.
const INITIAL_WIDTH: f32 = 696.0;
const MEDIA_HEIGHT: f32 = 167.0;
const ICON_SIZE: u32 = 32;
const SLIDERS_HEIGHT: f32 = 203.0;
const INITIAL_HEIGHT: f32 = MEDIA_HEIGHT + GRID_HEIGHT + SLIDERS_HEIGHT;

const SLIDER_HEIGHT: f32 = 25.0;
const SLIDER_LEFT_PERCENT: f32 = 15.0;
const SLIDER_RIGHT_PERCENT: f32 = 6.0;
const SLIDER_TOPS: [f32; 2] = [47.0, 133.0];
const BRIGHTNESS_PERCENT: f32 = 70.0;
const VOLUME_PERCENT: f32 = 45.0;

fn full_width(height: f32) -> LayoutStyle {
    LayoutStyle::default()
        .size(percent(100.0), px(height))
        .shrink(0.0)
}

fn placeholder(style: LayoutStyle, theme: &impl ThemeReader, surface: ColorRole) -> DivBuilder {
    div()
        .style(style)
        .background(theme.color(surface))
        .border(1.0, theme.color(ColorRole::OutlineVariant))
}

fn load_icon(atlas: &mut Atlas, svg: &[u8]) -> SpriteId {
    let bitmap = Bitmap::from_svg(svg, ICON_SIZE).expect("embedded icon SVG should render");
    atlas
        .insert(Class::Icon, &bitmap)
        .expect("icon should fit in the atlas")
}

struct QuickSettings;

struct QuickSettingsBuilder {
    root: NodeId,
    music_player: MusicPlayerBuilder,
    tiles: TilesBuilder,
    brightness_icon: SpriteId,
    volume_icon: SpriteId,
}

impl Build for QuickSettingsBuilder {
    type Widget = QuickSettings;
}

impl Widget for QuickSettings {
    type Builder = QuickSettingsBuilder;

    fn build(b: Self::Builder, _me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let win = s.spawn(
            b.root,
            window()
                .title("Mechanix Quick Settings")
                .clear(s.color(ColorRole::Background))
                .layout(
                    LayoutStyle::default()
                        .size(px(INITIAL_WIDTH), px(INITIAL_HEIGHT))
                        .column(),
                ),
        );
        s.on::<CloseRequested>(win, |ctx, _| ctx.signal(Stop));

        let panel = s.spawn(
            win,
            div().style(
                LayoutStyle::default()
                    .fill()
                    .column()
                    .justify(Justify::SpaceBetween),
            ),
        );

        s.spawn(panel, b.music_player);

        s.spawn(panel, b.tiles);

        // Static preview levels: brightness first, then volume.
        let sliders = s.spawn(
            panel,
            placeholder(
                full_width(SLIDERS_HEIGHT),
                s,
                ColorRole::SurfaceContainerLow,
            ),
        );
        for (top, (level, sprite)) in SLIDER_TOPS.into_iter().zip([
            (BRIGHTNESS_PERCENT, b.brightness_icon),
            (VOLUME_PERCENT, b.volume_icon),
        ]) {
            let icon_box = s.spawn(
                sliders,
                div().style(
                    LayoutStyle::default()
                        .size(percent(SLIDER_LEFT_PERCENT), px(SLIDER_HEIGHT))
                        .absolute()
                        .inset(Insets::new(px(top), auto(), auto(), px(0.0)))
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
                sliders,
                placeholder(
                    LayoutStyle::default()
                        .height(px(SLIDER_HEIGHT))
                        .padding_all(px(1.0))
                        .absolute()
                        .inset(Insets::new(
                            px(top),
                            percent(SLIDER_RIGHT_PERCENT),
                            auto(),
                            percent(SLIDER_LEFT_PERCENT),
                        )),
                    s,
                    ColorRole::SurfaceContainerHigh,
                ),
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

        QuickSettings
    }
}

fn main() {
    let mut atlas = Atlas::new();
    let font = atlas
        .add_font(include_bytes!("../assets/fonts/GeistMono-Regular.ttf"))
        .expect("bundled Geist Mono font should load");
    let brightness_icon = load_icon(
        &mut atlas,
        include_bytes!("../assets/brightness/brightness-medium.svg"),
    );
    let volume_icon = load_icon(
        &mut atlas,
        include_bytes!("../assets/speaker/speaker-high.svg"),
    );
    let music_player = MusicPlayerBuilder {
        font,
        previous_icon: load_icon(
            &mut atlas,
            include_bytes!("../assets/music-player/skip-back.svg"),
        ),
        play_icon: load_icon(
            &mut atlas,
            include_bytes!("../assets/music-player/play.svg"),
        ),
        next_icon: load_icon(
            &mut atlas,
            include_bytes!("../assets/music-player/skip-forward.svg"),
        ),
    };

    let mut app = App::new();
    app.add_module(LayoutModule)
        .add_module(PaintModule)
        .add_module(WindowModule)
        .add_module(MechanixTheme::dark())
        .add_module(InteractivityModule)
        .add_module(RenderModule::default())
        .insert_resource(atlas);
    app.add_module(RingModule::default())
        .add_module(
            WaylandModule::new()
                .bind::<WlCompositor>()
                .bind::<ZwpLinuxDmabufV1>()
                .bind::<XdgWmBase>()
                .bind::<WlSeat>(),
        )
        .add_module(PresentationModule {
            app_id: "mechanix.quicksettings".into(),
            budget: Budget::default(),
        });

    let root = app.root();
    app.spawn(
        root,
        QuickSettingsBuilder {
            root,
            music_player,
            tiles: TilesBuilder { font },
            brightness_icon,
            volume_icon,
        },
    );
    app.run();
}
