use super::*;

#[test]
fn get_all_reads_networking_hardware_and_wireless_flags_independently() {
    let cases = [
        (Some(Value::Bool(true)), true),
        (Some(Value::Bool(false)), false),
        (None, false),
        (Some(Value::Str("true".into())), false),
    ];
    for (networking, networking_expected) in cases.clone() {
        for (hardware, hardware_expected) in cases.clone() {
            for (wireless, wireless_expected) in cases.clone() {
                let call = CallId {
                    bus: BusId::System,
                    serial: 42,
                };
                let mut app = App::new();
                app.insert_resource(NMState {
                    pending: HashMap::from([(call, CallType::GetAll)]),
                    ..NMState::default()
                });
                app.system(on_method_reply);
                let mut properties = PropMap::new();
                if let Some(value) = networking.clone() {
                    properties.insert("NetworkingEnabled".into(), value);
                }
                if let Some(value) = hardware.clone() {
                    properties.insert("WirelessHardwareEnabled".into(), value);
                }
                if let Some(value) = wireless {
                    properties.insert("WirelessEnabled".into(), value);
                }
                app.signal(MethodReply {
                    call,
                    result: Ok(Message::method_return(call.serial).with_body(&(properties,))),
                });
                app.flush();

                let state = app.resource::<NMState>();
                assert_eq!(state.networking_enabled, networking_expected);
                assert_eq!(state.wireless_hardware_enabled, hardware_expected);
                assert_eq!(state.wireless_enabled, wireless_expected);
                assert!(state.pending.is_empty());
            }
        }
    }
}
