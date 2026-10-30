use mecha_wayland::prelude::*;

use crate::indicators::{indicator_layout, load_svg_icon};

use assets::icons::wifi as svg;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WifiState {
    #[default]
    High,
    Medium,
    Low,
    None,
    Disabled,
    Disconnected,
}

impl WifiState {
    /// Icon for a NetworkManager state. `strength` (0-100, zero when unknown)
    /// only matters while connected.
    pub fn from_service(state: ::services::WifiState, strength: u8) -> Self {
        use ::services::WifiState as Service;
        match state {
            Service::Connected => match strength {
                0..=24 => Self::None,
                25..=49 => Self::Low,
                50..=74 => Self::Medium,
                _ => Self::High,
            },
            Service::On | Service::Disconnected | Service::Connecting | Service::Disconnecting => {
                Self::Disconnected
            }
            Service::Unavailable | Service::Off => Self::Disabled,
        }
    }

    #[inline]
    pub fn is_visible(&self) -> bool {
        !matches!(self, Self::Disabled)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WifiIcons {
    pub high: SpriteId,
    pub medium: SpriteId,
    pub low: SpriteId,
    pub none: SpriteId,
    pub slash: SpriteId,
    pub x: SpriteId,
}

impl WifiIcons {
    pub fn load(atlas: &mut Atlas, master: u32) -> Result<Self, Error> {
        Ok(Self {
            high: load_svg_icon(atlas, svg::HIGH, master)?,
            medium: load_svg_icon(atlas, svg::MEDIUM, master)?,
            low: load_svg_icon(atlas, svg::LOW, master)?,
            none: load_svg_icon(atlas, svg::NONE, master)?,
            slash: load_svg_icon(atlas, svg::SLASH, master)?,
            x: load_svg_icon(atlas, svg::X, master)?,
        })
    }

    pub fn sprite(&self, state: WifiState) -> SpriteId {
        match state {
            WifiState::High => self.high,
            WifiState::Medium => self.medium,
            WifiState::Low => self.low,
            WifiState::None => self.none,
            WifiState::Disabled => self.slash,
            WifiState::Disconnected => self.x,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WifiIndicator {
    pub icons: WifiIcons,
    pub state: WifiState,
    pub icon_node: Handle<Icon>,
    /// Latest service updates; `StateUpdate` and `APUpdate` arrive separately.
    connection: ::services::WifiState,
    strength: u8,
}

pub fn wifi_indicator(icons: WifiIcons) -> WifiIndicatorBuilder {
    WifiIndicatorBuilder {
        icons,
        state: WifiState::default(),
    }
}

pub struct WifiIndicatorBuilder {
    pub icons: WifiIcons,
    pub state: WifiState,
}

impl WifiIndicatorBuilder {
    pub fn state(mut self, state: WifiState) -> Self {
        self.state = state;
        self
    }
}

impl Build for WifiIndicatorBuilder {
    type Widget = WifiIndicator;
}

impl Widget for WifiIndicator {
    type Builder = WifiIndicatorBuilder;

    fn build(b: WifiIndicatorBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = indicator_layout(b.state.is_visible());

        let sprite = b.icons.sprite(b.state);
        let color = s.color(ColorRole::OnSurface);
        let icon_node = s.spawn(me, icon(sprite).color(color));

        s.on_theme(me, move |ctx| {
            let color = ctx.color(ColorRole::OnSurface);
            ctx.at(icon_node).unwrap().set_color(color);
        });

        WifiIndicator {
            icons: b.icons,
            state: b.state,
            icon_node,
            connection: ::services::WifiState::Unavailable,
            strength: 0,
        }
    }
}

impl WifiIndicator {
    #[inline]
    pub fn state(&self) -> WifiState {
        self.state
    }
}

pub trait WifiIndicatorContext {
    fn state(&mut self) -> WifiState;
    fn set_state(&mut self, state: WifiState);
    fn set_connection(&mut self, connection: ::services::WifiState);
    fn set_strength(&mut self, strength: u8);
}

impl WifiIndicatorContext for Context<'_, WifiIndicator> {
    #[inline]
    fn state(&mut self) -> WifiState {
        self.me().state
    }

    fn set_state(&mut self, state: WifiState) {
        if self.me().state == state {
            return;
        }
        self.me().state = state;
        let visible = state.is_visible();
        self.set_style(indicator_layout(visible));

        if visible {
            let sprite = self.me().icons.sprite(state);
            let icon_node = self.me().icon_node;
            self.at(icon_node).unwrap().set_sprite(sprite);
        }
    }

    fn set_connection(&mut self, connection: ::services::WifiState) {
        self.me().connection = connection;
        let state = WifiState::from_service(connection, self.me().strength);
        self.set_state(state);
    }

    fn set_strength(&mut self, strength: u8) {
        self.me().strength = strength;
        let state = WifiState::from_service(self.me().connection, strength);
        self.set_state(state);
    }
}
