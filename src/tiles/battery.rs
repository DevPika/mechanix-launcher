use mecha_wayland::prelude::*;

use crate::services::BatteryPercentageChanged;

use super::{build_label, build_tile};

mod svg {
    pub const LOW: &[u8] = include_bytes!("../../assets/battery/battery-low.svg");
}

pub struct Battery {
    pub icon: Handle<Icon>,
    pub percentage: Handle<Text>,
}

pub struct BatteryBuilder {
    pub font: FontId,
}

impl Build for BatteryBuilder {
    type Widget = Battery;
}

impl Widget for Battery {
    type Builder = BatteryBuilder;

    fn build(b: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let icon = build_tile(me, s, svg::LOW);
        let percentage = build_label(me, s, b.font, "N/A");

        s.on::<BatteryPercentageChanged>(me, |ctx, event| {
            ctx.set_percentage(event.percentage);
        });

        Battery { icon, percentage }
    }
}

pub trait BatteryContext {
    fn set_percentage(&mut self, percentage: Option<u8>);
}

impl BatteryContext for Context<'_, Battery> {
    fn set_percentage(&mut self, percentage: Option<u8>) {
        let label = match percentage {
            Some(value) => format!("{value}%"),
            None => "N/A".to_owned(),
        };
        let text = self.me().percentage;
        self.at(text).unwrap().set_text(label);
    }
}
