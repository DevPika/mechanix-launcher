use mecha_wayland::prelude::*;
use std::collections::HashMap;

const NM: &str = "org.freedesktop.NetworkManager";
const NM_MAIN_OBJ: &str = "/org/freedesktop/NetworkManager";

enum CallType {
    GetAll, // GetAll on NM_MAIN_OBJ
}
struct NMBookKeeping {
    pending: HashMap<CallId, CallType>,
}
impl Resource for NMBookKeeping {}

pub struct NetworkManagerModule;
impl Module for NetworkManagerModule {
    fn install(self, app: &mut App) {
        app.insert_resource(NMBookKeeping {
            pending: HashMap::new(),
        });

        let mut dbus = app.resource_mut::<DBus>();
        let call = dbus
            .system()
            .object(NM, NM_MAIN_OBJ)
            .get_all(NM);

        app.resource_mut::<NMBookKeeping>()
            .pending
            .insert(call, CallType::GetAll);
    }
}
