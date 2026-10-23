use mecha_wayland::prelude::*;
use std::collections::HashMap;

const NM: &str = "org.freedesktop.NetworkManager";
const NM_MAIN_OBJ: &str = "/org/freedesktop/NetworkManager";

enum CallType {
    GetAll, // GetAll on NM_MAIN_OBJ
}

#[derive(Default)]
pub struct NMState {
    /// is networking enabled
    pub networking_enabled: bool,
    /// if wireless hardware is enabled (useful for kill switch)
    pub wireless_hardware_enabled: bool,
    pending: HashMap<CallId, CallType>,
}
impl Resource for NMState {}

pub struct NetworkManagerModule;
impl Module for NetworkManagerModule {
    fn install(self, app: &mut App) {
        app.insert_resource(NMState::default());

        let mut dbus = app.resource_mut::<DBus>();
        let call = dbus.system().object(NM, NM_MAIN_OBJ).get_all(NM);

        app.resource_mut::<NMState>()
            .pending
            .insert(call, CallType::GetAll);
        app.system(on_method_reply);
    }
}

fn on_method_reply(app: &mut App, event: &MethodReply) {
    let mut state = app.resource_mut::<NMState>();
    let Some(call_type) = state.pending.remove(&event.call) else {
        return; // another module's DBus call
    };

    let message = match &event.result {
        Ok(message) => message,
        // cleanly handle network manager not being available
        Err(error)
            if error.is("org.freedesktop.DBus.Error.ServiceUnknown")
                || error.is("org.freedesktop.DBus.Error.NameHasNoOwner") =>
        {
            eprintln!("NetworkManager unavailable, clearing all pending calls");
            state.pending.clear();
            return;
        }
        Err(error) => {
            eprintln!("NetworkManager request failed: {error:?}");
            return;
        }
    };

    // handle messages
    match call_type {
        CallType::GetAll => {
            let (properties,) = match message.body::<(PropMap,)>() {
                Ok(body) => body,
                Err(_) => {
                    println!("NetworkManager: invalid GetAll reply");
                    return;
                }
            };

            app.resource_mut::<NMState>().networking_enabled =
                match properties.get_as::<bool>("NetworkingEnabled") {
                    Some(Ok(enabled)) => enabled,
                    Some(Err(error)) => {
                        eprintln!("NetworkManager: invalid NetworkingEnabled: {error:?}");
                        false
                    }
                    None => {
                        eprintln!("NetworkManager: missing NetworkingEnabled");
                        false
                    }
                };

            app.resource_mut::<NMState>().wireless_hardware_enabled =
                match properties.get_as::<bool>("WirelessHardwareEnabled") {
                    Some(Ok(enabled)) => enabled,
                    Some(Err(error)) => {
                        eprintln!("NetworkManager: invalid WirelessHardwareEnabled: {error:?}");
                        false
                    }
                    None => {
                        eprintln!("NetworkManager: missing WirelessHardwareEnabled");
                        false
                    }
                };
        }
    }
}

#[cfg(test)]
mod tests;
