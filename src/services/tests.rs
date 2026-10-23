use super::*;

#[test]
fn get_all_distinguishes_enabled_disabled_and_unknown_networking() {
    for (value, expected) in [
        (Some(Value::Bool(true)), Some(true)),
        (Some(Value::Bool(false)), Some(false)),
        (None, None),
        (Some(Value::Str("true".into())), None),
    ] {
        let call = CallId {
            bus: BusId::System,
            serial: 42,
        };
        let mut app = App::new();
        app.insert_resource(NMState::default());
        app.insert_resource(NMBookKeeping {
            pending: HashMap::from([(call, CallType::GetAll)]),
        });
        app.system(on_method_reply);
        let mut properties = PropMap::new();
        if let Some(value) = value {
            properties.insert("NetworkingEnabled".into(), value);
        }
        app.signal(MethodReply {
            call,
            result: Ok(Message::method_return(call.serial).with_body(&(properties,))),
        });
        app.flush();

        assert_eq!(app.resource::<NMState>().networking_enabled, expected);
        assert!(app.resource::<NMBookKeeping>().pending.is_empty());
    }
}
