use mecha_wayland::mechanix_widgets::prelude::FontBook;
use mecha_wayland::prelude::*;
use status_bar::{DEFAULT_ICON_SIZE, GEIST_MONO_BYTES, StatusBarModule, StatusIcons};

fn main() {
    let mut app = App::new();
    app.add_module(LayoutModule)
        .add_module(PaintModule)
        .add_module(WindowModule)
        .add_module(InteractivityModule)
        .add_module(AnimationModule)
        .add_module(RenderModule::default())
        .insert_resource(Atlas::new());

    app.add_module(MechanixTheme::dark());

    let font = app
        .resource_mut::<Atlas>()
        .add_font(GEIST_MONO_BYTES)
        .expect("GeistMono font loads");
    app.insert_resource(FontBook::new(font));

    let icons = StatusIcons::load(&mut app.resource_mut::<Atlas>(), DEFAULT_ICON_SIZE)
        .expect("Status bar icons load");
    app.insert_resource(icons);

    app.add_module(RingModule::default())
        .add_module(DBusModule::new().system())
        .add_module(
            WaylandModule::new()
                .bind::<WlCompositor>()
                .bind::<ZwpLinuxDmabufV1>()
                .bind::<XdgWmBase>()
                .bind::<WlSeat>()
                .bind::<ZwlrLayerShellV1>(),
        )
        .add_module(PresentationModule {
            app_id: "mechanix.launcher".into(),
            budget: Budget::default(),
        });

    app.add_module(StatusBarModule);

    app.run();
}
