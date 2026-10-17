use mecha_wayland::mechanix_widgets::prelude::{TextVariant, text};
use mecha_wayland::prelude::*;

/// One navigation section of the dock. Each section lives in its own crate and
/// the dock holds them as `Vec<Box<dyn Section>>`, so this trait must stay
/// object-safe.
pub trait Section: 'static {
    /// Stable identifier, e.g. "favourites", "search", "recents",
    /// "notifications", "quick-settings".
    fn id(&self) -> &'static str;
    /// Human-readable name, e.g. "Favourites", "Recent Apps".
    fn label(&self) -> &'static str;
    /// Phosphor fill SVG bytes for the dock icon.
    fn icon_svg(&self) -> &'static [u8];
    /// Spawns this section's page under `parent` and returns its root node.
    fn spawn_page(&self, app: &mut App, parent: NodeId) -> NodeId;
}

/// Spawns a themed full-size page showing `label` centred. Used by sections
/// that have no real page yet.
pub fn placeholder_page(app: &mut App, parent: NodeId, label: &str) -> NodeId {
    app.spawn(
        parent,
        PlaceholderPage {
            label: label.to_string(),
        },
    )
    .id()
}

struct PlaceholderPage {
    label: String,
}

impl Build for PlaceholderPage {
    type Widget = PlaceholderPage;
}

impl Widget for PlaceholderPage {
    type Builder = PlaceholderPage;

    fn build(b: Self, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = LayoutStyle::default()
            .center()
            .width(percent(100.0))
            .height(percent(100.0));

        let bg = s.color(ColorRole::Surface);
        *s.component_mut::<Paint>(me).unwrap() = Paint::Quad(Quad::new(bg));
        s.on_theme(me, move |ctx| {
            let bg = ctx.color(ColorRole::Surface);
            ctx.set_paint(Paint::Quad(Quad::new(bg)));
        });

        s.spawn(
            me,
            text(b.label.clone()).variant(TextVariant::HeadlineMedium),
        );
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mecha_wayland::mechanix_widgets::prelude::{FontBook, Text};
    use status_bar::GEIST_MONO_BYTES;

    #[test]
    fn placeholder_page_spawns_label() {
        let mut app = App::new();
        app.add_module(LayoutModule)
            .add_module(PaintModule)
            .add_module(MechanixTheme::dark())
            .insert_resource(Atlas::new());
        let font = app
            .resource_mut::<Atlas>()
            .add_font(GEIST_MONO_BYTES)
            .expect("GeistMono loads");
        app.insert_resource(FontBook::new(font));

        let root = app.root();
        let page = placeholder_page(&mut app, root, "Search");
        app.tick();

        let texts: Vec<_> = app
            .descendants(page)
            .filter(|n| app.widget::<Text>(*n).is_some())
            .collect();
        assert_eq!(texts.len(), 1);
    }

    #[test]
    fn section_is_object_safe() {
        fn _takes(_: Vec<Box<dyn Section>>) {}
    }
}
