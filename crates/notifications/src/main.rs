use mecha_wayland::mechanix_widgets::FontBook;
use mecha_wayland::prelude::*;

pub mod card;
pub mod card_list;
pub mod dbus;
pub mod dbus_spec;
pub mod notification_store;
pub mod notify;
pub mod panel;
pub mod style;

use crate::{
    card_list::notifications_changed, dbus::{NotificationServer, on_call}, dbus_spec::register_notifications_interface, notification_store::NotificationStore, notify::append_notifications, panel::PanelBuilder,
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
        .add_module(PresentationModule {
            app_id: "Mecha Notifications".into(),
            budget: Budget::default(),
        });

    // TODO: Match the figma's font
    let font = app
        .resource_mut::<Atlas>()
        .add_font(include_bytes!(
            "../../../crates/atlas/tests/fixtures/Inter-Regular.ttf"
        ))
        .expect("Inter loads");

    let export_id = register_notifications_interface(&mut app);

    // this will register the id for the interface
    app.insert_resource(NotificationServer { export_id });
    app.insert_resource(FontBook::new(font));
    app.system(on_call);

    let root = app.root();
    app.spawn(root, PanelBuilder { root });
    app.init_resource::<NotificationStore>();
    app.system(append_notifications);
    app.system(notifications_changed);
    app.run();
}
