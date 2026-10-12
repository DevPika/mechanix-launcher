use crate::notify::{Categories, Notification};

fn notification(app_name: &str, summary: &str, body: &str) -> Notification {
    Notification {
        app_name: app_name.into(),
        summary: summary.into(),
        body: body.into(),
        actions: vec![("default".into(), "Open".into())],
        hints: Categories::NORMAL,
        ..Default::default()
    }
}

pub fn notifications() -> Vec<Notification> {
    vec![
        Notification {
            desktop_entry: "org.gnome.Calls".into(),
            app_icon: "call-start-symbolic".into(),
            actions: vec![
                ("default".into(), "Open".into()),
                ("call-back".into(), "Call back".into()),
                ("message".into(), "Message".into()),
            ],
            ..notification("Calls", "Pritam", "Missed call")
        },
        notification("Messages", "Phoebe Bridgers", "I know the end"),
        notification("pacman", "pacman", "3 packages updated · kernel 6.14.6"),
        notification("Mail", "James Rivera", "Re: Project timeline update"),
        notification("X", "@musicfan", "Started following you"),
        notification("X", "@indieheads", "Retweeted your post"),
        notification("X", "@techcrunch", "Liked your reply"),
    ]
}
