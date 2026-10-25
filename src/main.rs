use mecha_wayland::prelude::*;

mod bottom_bar;
mod header;
mod quick_settings;
mod services;
mod sliders;
mod tiles;
mod util;

use bottom_bar::BottomBarBuilder;
use header::HeaderBuilder;
use quick_settings::QuickSettingsBuilder;
use services::{BluetoothModule, DisplayModule, UpowerModule};
use sliders::SlidersBuilder;

const PANEL_HEIGHT: f32 = 620.0;

struct Shell;

struct ShellBuilder {
    root: NodeId,
    header: HeaderBuilder,
    bottom_bar: BottomBarBuilder,
    sliders: SlidersBuilder,
}

impl Build for ShellBuilder {
    type Widget = Shell;
}

impl Widget for Shell {
    type Builder = ShellBuilder;

    fn build(b: Self::Builder, _me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let role = Role::Layer(LayerRole {
            layer: Layer::Overlay,
            anchor: Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
            exclusive_zone: 0,
            namespace: "mechanix-quicksettings".into(),
            keyboard_interactivity: KeyboardInteractivity::None,
        });

        let win = s.spawn_with(
            b.root,
            window()
                .title("Mechanix Quick Settings")
                .clear(s.color(ColorRole::Background))
                .layout(LayoutStyle::default().height(px(PANEL_HEIGHT))),
            (role,),
        );
        s.on::<CloseRequested>(win, |ctx, _| ctx.signal(Stop));
        s.spawn(
            win,
            QuickSettingsBuilder {
                header: b.header,
                bottom_bar: b.bottom_bar,
                sliders: b.sliders,
            },
        );
        Shell
    }
}

fn main() {
    let mut atlas = Atlas::new();
    let font = atlas
        .add_font(include_bytes!("../assets/fonts/GeistMono-Regular.ttf"))
        .expect("bundled Geist Mono font should load");
    let mut app = App::new();
    app.add_module(LayoutModule)
        .add_module(AnimationModule)
        .add_module(PaintModule)
        .add_module(WindowModule)
        .add_module(MechanixTheme::dark())
        .add_module(InteractivityModule)
        .add_module(RenderModule::default())
        .insert_resource(atlas);
    app.add_module(RingModule::default())
        .add_module(DBusModule::new().system())
        .add_module(UpowerModule)
        .add_module(BluetoothModule)
        .add_module(DisplayModule)
        .add_module(
            WaylandModule::new()
                .bind::<WlCompositor>()
                .bind::<ZwpLinuxDmabufV1>()
                .bind::<XdgWmBase>()
                .bind::<WlSeat>()
                .bind::<ZwlrLayerShellV1>(),
        )
        .add_module(PresentationModule {
            app_id: "mechanix.quicksettings".into(),
            budget: Budget::default(),
        });

    let root = app.root();
    app.spawn(
        root,
        ShellBuilder {
            root,
            header: HeaderBuilder { font },
            bottom_bar: BottomBarBuilder { font },
            sliders: SlidersBuilder,
        },
    );

    app.run();
}
