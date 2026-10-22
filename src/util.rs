use mecha_wayland::prelude::*;

pub const ICON_SIZE: u32 = 32;

pub fn full_width(height: f32) -> LayoutStyle {
    LayoutStyle::default()
        .size(percent(100.0), px(height))
        .shrink(0.0)
}

pub fn load_icon(atlas: &mut Atlas, svg: &[u8]) -> SpriteId {
    let bitmap = Bitmap::from_svg(svg, ICON_SIZE).expect("embedded icon SVG should render");
    atlas
        .insert(Class::Icon, &bitmap)
        .expect("icon should fit in the atlas")
}
