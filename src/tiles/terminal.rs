use mecha_wayland::prelude::*;

use super::build_tile;

mod svg {
    pub const ICON: &[u8] = include_bytes!("../../assets/terminal.svg");
}

pub struct Terminal {
    pub icon: Handle<Icon>,
}

pub struct TerminalBuilder;

impl Build for TerminalBuilder {
    type Widget = Terminal;
}

impl Widget for Terminal {
    type Builder = TerminalBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let icon = build_tile(me, s, svg::ICON);
        Terminal { icon }
    }
}
