use mecha_wayland::prelude::*;

use super::{build_label, build_tile};
use crate::services::BatteryPercentageChanged;

pub struct Battery {
    pub icon: Handle<Icon>,
    pub percentage: Handle<Text>,
}

pub struct BatteryBuilder {
    pub font: FontId,
    pub percentage: u8,
}

impl Build for BatteryBuilder {
    type Widget = Battery;
}

impl Widget for Battery {
    type Builder = BatteryBuilder;

    fn build(b: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let icon = build_tile(
            me,
            s,
            include_bytes!("../../assets/battery/battery-low.svg"),
        );
        let percentage = build_label(me, s, b.font, format!("{}%", b.percentage));
        Battery { icon, percentage }
    }
}
