use mecha_wayland::mechanix_widgets::FontBook;
use mecha_wayland::prelude::*;

pub mod card;
pub mod card_list;
pub mod dbus;
pub mod dbus_spec;
pub mod expiry_timeout;
pub mod markup;
pub mod notification_store;
pub mod notify;
pub mod notify_signal;
pub mod panel;
pub mod rich_text;
pub mod style;

use crate::{
    dbus::{NotificationServer, on_call},
    dbus_spec::register_notifications_interface,
    expiry_timeout::{ExpiryTimers, notification_time_expired},
    notification_store::NotificationStore,
    notify_signal::notify_signal,
    panel::PanelBuilder,
};

fn main() {
    let mut app = App::new();
    app.add_module(LayoutModule)
        .add_module(PaintModule)
        .add_module(WindowModule)
        .add_module(InteractivityModule)
        .add_module(RenderModule::default())
        .add_module(MechanixTheme::dark())
        .insert_resource(Atlas::new());
    app.add_module(RingModule::default())
        .add_module(DBusModule::new().session())
        .add_module(
            WaylandModule::new()
                .bind::<WlCompositor>()
                .bind::<ZwpLinuxDmabufV1>()
                .bind::<XdgWmBase>()
                .bind::<WlSeat>(),
        )
        .add_module(AnimationModule)
        .add_module(PresentationModule {
            app_id: "Mecha Notifications".into(),
            budget: Budget::default(),
        });

    // TODO: Match the figma's font
    // This is just a workaround for now
    let (font, semi_bold, bold) = {
        let mut atlas = app.resource_mut::<Atlas>();
        let mut load = |bytes: &'static [u8]| atlas.add_font(bytes).expect("Open Sans loads");
        (
            load(include_bytes!(
                "/usr/share/fonts/open-sans/OpenSans-Regular.ttf"
            )),
            load(include_bytes!(
                "/usr/share/fonts/open-sans/OpenSans-Semibold.ttf"
            )),
            load(include_bytes!(
                "/usr/share/fonts/open-sans/OpenSans-Bold.ttf"
            )),
        )
    };

    let export_id = register_notifications_interface(&mut app);

    // this will register the id for the interface
    app.insert_resource(NotificationServer { export_id });
    app.insert_resource(FontBook::new(font).semi_bold(semi_bold).bold(bold));
    app.system(notify_signal);
    app.system(on_call);

    let root = app.root();
    app.spawn(root, PanelBuilder { root });
    app.init_resource::<NotificationStore>();
    app.init_resource::<ExpiryTimers>();
    app.system(notification_time_expired);
    app.run();
}
