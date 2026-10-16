use mecha_wayland::prelude::*;

mod music_player;

use music_player::MusicPlayerBuilder;

// Initial window size only; all content widths follow the actual window width.
const INITIAL_WIDTH: f32 = 696.0;
const MEDIA_HEIGHT: f32 = 167.0;
const TILE_HEIGHT: f32 = 139.0;
const TILE_COLUMNS: usize = 5;
const TILE_ROWS: usize = 2;
const ICON_SIZE: u32 = 32;
const SLIDERS_HEIGHT: f32 = 203.0;
const HEIGHT: f32 = MEDIA_HEIGHT + TILE_HEIGHT * TILE_ROWS as f32 + SLIDERS_HEIGHT;

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
    font: FontId,
    tile_icons: [SpriteId; TILE_COLUMNS * TILE_ROWS],
    placeholder_icon: SpriteId,
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
                        .size(px(INITIAL_WIDTH), px(HEIGHT))
                        .column(),
                ),
        );
        s.on::<CloseRequested>(win, |ctx, _| ctx.signal(Stop));

        let panel = s.spawn(win, div().style(full_width(HEIGHT).column()));

        s.spawn(
            panel,
            MusicPlayerBuilder {
                font: b.font,
                placeholder_icon: b.placeholder_icon,
            },
        );

        // Two rows of five equally sized tiles.
        for icons in b.tile_icons.chunks_exact(TILE_COLUMNS) {
            let row = s.spawn(panel, div().style(full_width(TILE_HEIGHT).row()));
            for &sprite in icons {
                let tile = s.spawn(
                    row,
                    placeholder(
                        full_width(TILE_HEIGHT)
                            .width(percent(100.0 / TILE_COLUMNS as f32))
                            .center(),
                        s,
                        ColorRole::Surface,
                    ),
                );
                s.spawn(
                    tile,
                    icon(sprite)
                        .style(LayoutStyle::default().shrink(0.0))
                        .color(s.color(ColorRole::OnSurface)),
                );
            }
        }

        // Static preview levels: brightness first, then volume.
        let sliders = s.spawn(
            panel,
            placeholder(
                full_width(SLIDERS_HEIGHT),
                s,
                ColorRole::SurfaceContainerLow,
            ),
        );
        for (top, level) in SLIDER_TOPS
            .into_iter()
            .zip([BRIGHTNESS_PERCENT, VOLUME_PERCENT])
        {
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
                icon(b.placeholder_icon)
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
    let airplane = load_icon(&mut atlas, include_bytes!("../assets/airplane.svg"));
    let wifi = load_icon(&mut atlas, include_bytes!("../assets/wifi/wifi-high.svg"));
    let bluetooth = load_icon(
        &mut atlas,
        include_bytes!("../assets/bluetooth/bluetooth.svg"),
    );
    let cellular = load_icon(
        &mut atlas,
        include_bytes!("../assets/cellular/cell-signal-full.svg"),
    );
    // Row-major reference order, with airplane placeholders for missing assets.
    let tile_icons = [
        wifi,      // Wi-Fi
        bluetooth, // Bluetooth
        airplane,  // Sync
        airplane,  // Terminal
        airplane,  // Files
        cellular,  // Cellular
        airplane,  // Battery
        airplane,  // Recording
        airplane,  // Camera
        airplane,  // Settings
    ];

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
            font,
            tile_icons,
            placeholder_icon: airplane,
        },
    );
    app.run();
}
