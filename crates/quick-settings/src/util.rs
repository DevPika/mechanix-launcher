use mecha_wayland::prelude::*;

const ICON_SIZE: u32 = 32;

pub fn full_width(height: f32) -> LayoutStyle {
    LayoutStyle::default()
        .size(percent(100.0), px(height))
        .shrink(0.0)
}

pub fn load_icon(atlas: &mut Atlas, svg: &[u8]) -> SpriteId {
    load_icon_sized(atlas, svg, ICON_SIZE)
}

pub fn load_icon_sized(atlas: &mut Atlas, svg: &[u8], size: u32) -> SpriteId {
    let bitmap = Bitmap::from_svg(svg, size).expect("embedded icon SVG should render");
    atlas
        .insert(Class::Icon, &bitmap)
        .expect("icon should fit in the atlas")
}
