use mecha_wayland::prelude::*;

mod header;
mod services;
mod tiles;

use header::{HEADER_HEIGHT, HeaderBuilder};
use services::UpowerModule;
use tiles::{GRID_HEIGHT, TilesBuilder};

// Initial window size only; the panel fills the actual window dimensions.
const INITIAL_WIDTH: f32 = 696.0;
const ICON_SIZE: u32 = 32;
const SLIDERS_HEIGHT: f32 = 203.0;
const HEADER_SLIDER_GAP: f32 = 56.0;
const TOP_HEIGHT: f32 = HEADER_HEIGHT + HEADER_SLIDER_GAP + SLIDERS_HEIGHT;
const SECTION_GAP: f32 = 64.0;
const INITIAL_HEIGHT: f32 = TOP_HEIGHT + SECTION_GAP + GRID_HEIGHT;

const SLIDER_HEIGHT: f32 = 40.0;
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
    header: HeaderBuilder,
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
                    .gap(px(SECTION_GAP))
                    .justify(Justify::SpaceBetween),
            ),
        );

        let top = s.spawn(
            panel,
            div().style(full_width(TOP_HEIGHT).column().gap(px(HEADER_SLIDER_GAP))),
        );
        s.spawn(top, b.header);

        // Static preview levels: volume first, then brightness.
        let sliders = s.spawn(
            top,
            placeholder(
                full_width(SLIDERS_HEIGHT),
                s,
                ColorRole::SurfaceContainerLow,
            ),
        );
        for (top, (level, sprite)) in SLIDER_TOPS.into_iter().zip([
            (VOLUME_PERCENT, b.volume_icon),
            (BRIGHTNESS_PERCENT, b.brightness_icon),
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

        s.spawn(panel, b.tiles);

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
    let mut app = App::new();
    app.add_module(LayoutModule)
        .add_module(PaintModule)
        .add_module(WindowModule)
        .add_module(MechanixTheme::dark())
        .add_module(InteractivityModule)
        .add_module(RenderModule::default())
        .insert_resource(atlas);
    app.add_module(RingModule::default())
        .add_module(DBusModule::new().system())
        .add_module(UpowerModule)
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
            header: HeaderBuilder { font },
            tiles: TilesBuilder { font },
            brightness_icon,
            volume_icon,
        },
    );

    app.run();
}
