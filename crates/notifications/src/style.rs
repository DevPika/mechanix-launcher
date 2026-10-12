use mecha_wayland::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub variant: TextVariant,
    pub color: ColorRole,
}

impl TextStyle {
    const fn new(variant: TextVariant, color: ColorRole) -> Self {
        Self { variant, color }
    }
}

pub mod color {
    use super::ColorRole;

    /// Panel behind the list. Figma `#0D0D0D`.
    pub const SCREEN: ColorRole = ColorRole::SurfaceContainerLowest;
    /// Notification card. Figma `#303030`.
    pub const CARD: ColorRole = ColorRole::SurfaceContainerHighest;
    /// "Options" swipe button. Figma `#303030`.
    pub const OPTIONS: ColorRole = ColorRole::SurfaceContainerHighest;
    /// "Clear" swipe button. Figma `#202020`.
    pub const CLEAR: ColorRole = ColorRole::SurfaceContainerHigh;
    /// "Clear all" / group "Clear" chip. Figma `#303030`.
    pub const CHIP: ColorRole = ColorRole::SecondaryContainer;
    /// Unread count badge. Figma `#FF4E4E`.
    pub const COUNT_BADGE: ColorRole = ColorRole::Error;
    pub const COUNT_BADGE_TEXT: ColorRole = ColorRole::OnError;
    /// Stack count badge. Figma `#CCCCCC` on `#2E2E2E`.
    pub const STACK_BADGE: ColorRole = ColorRole::InverseSurface;
    pub const STACK_BADGE_TEXT: ColorRole = ColorRole::InverseOnSurface;
    /// Heads-up "Decline" / "Accept".
    pub const DECLINE: ColorRole = ColorRole::Error;
    pub const ACCEPT: ColorRole = ColorRole::Success;
    /// Card shadow. Figma `rgba(0,0,0,0.25)`, offset 2,0, blur 1.5.
    pub const SHADOW: ColorRole = ColorRole::Shadow;
}

pub mod text {
    use super::TextStyle;
    use super::{ColorRole, TextVariant};

    /// "Notifications". Figma Sora Bold 20, white.
    pub const SCREEN_TITLE: TextStyle =
        TextStyle::new(TextVariant::TitleLarge, ColorRole::OnSurface);
    /// Group label, e.g. "X/Twitter". Figma SemiBold 15, `#A2A2A2`.
    pub const GROUP_HEADER: TextStyle =
        TextStyle::new(TextVariant::LabelLarge, ColorRole::OnSecondaryContainer);
    /// Sender / summary. Figma SemiBold 16, white.
    pub const CARD_TITLE: TextStyle = TextStyle::new(TextVariant::TitleSmall, ColorRole::OnSurface);
    /// "2 h". Figma Regular 15, `#6A6A6A` (15 is off-scale).
    pub const CARD_TIME: TextStyle =
        TextStyle::new(TextVariant::BodyMedium, ColorRole::OnSurfaceVariant);
    /// Body / subtext. Figma Regular 14, `#868686`.
    pub const CARD_BODY: TextStyle =
        TextStyle::new(TextVariant::BodyMedium, ColorRole::OnSurfaceVariant);
    /// Action bar label, e.g. "Reply". Figma SemiBold 15, white.
    pub const ACTION: TextStyle = TextStyle::new(TextVariant::LabelLarge, ColorRole::OnSurface);
    /// "Options" / "Clear". Figma SemiBold 16, white.
    pub const SWIPE_BUTTON: TextStyle =
        TextStyle::new(TextVariant::TitleSmall, ColorRole::OnSurface);
    /// "Clear all" chip. Figma SemiBold 13, `#B0B0B0`.
    pub const CHIP: TextStyle =
        TextStyle::new(TextVariant::LabelMedium, ColorRole::OnSecondaryContainer);
    /// Badge digit. Figma SemiBold 14–16.
    pub const BADGE: TextStyle = TextStyle::new(TextVariant::LabelMedium, ColorRole::OnError);
    /// "No notifications". Figma centered caption.
    pub const EMPTY: TextStyle =
        TextStyle::new(TextVariant::BodyMedium, ColorRole::OnSurfaceVariant);
}

pub mod space {
    use super::Spacing;

    /// Panel side padding. Figma 30 (off-scale; 32 is nearest).
    pub const SCREEN_X: Spacing = Spacing::Space400;
    /// Panel top padding to the title. Figma 40.
    pub const SCREEN_TOP: Spacing = Spacing::Space500;
    /// Title row to first card. Figma 30 (96 − 40 − 26 title line;
    /// off-scale, 32 is nearest).
    pub const HEADER_TO_LIST: Spacing = Spacing::Space400;
    /// Between cards. Figma 18 (off-scale; 16 is nearest).
    pub const CARD_GAP: Spacing = Spacing::Space200;
    /// Card left/right padding. Figma 20.
    pub const CARD_X: Spacing = Spacing::Space250;
    /// Expanded card top padding. Figma 20.
    pub const CARD_TOP: Spacing = Spacing::Space250;
    /// Expanded card bottom padding. Figma 16.
    pub const CARD_BOTTOM: Spacing = Spacing::Space200;
    /// Icon to text column. Figma 16.
    pub const ICON_TO_TEXT: Spacing = Spacing::Space200;
    /// Title row to body. Figma 8.
    pub const TITLE_TO_BODY: Spacing = Spacing::Space100;
    /// Expanded content to action bar. Figma 20.
    pub const CONTENT_TO_ACTIONS: Spacing = Spacing::Space250;
    /// Card / Options / Clear while swiping. Figma 14.
    pub const SWIPE_GAP: Spacing = Spacing::Space175;
    /// Chip horizontal padding. Figma 21–24.
    pub const CHIP_X: Spacing = Spacing::Space250;
}

pub mod shape {
    use super::Shape;

    /// Card, swipe buttons. Figma radius 12.
    pub const CARD: Shape = Shape::Medium;
    /// App icon square. Figma radius 12.
    pub const APP_ICON: Shape = Shape::Medium;
    /// Contact avatar. Figma radius 26 on 52 (circle).
    pub const AVATAR: Shape = Shape::Full;
    /// Chips and badges. Figma pill.
    pub const PILL: Shape = Shape::Full;
}

/// Component sizes, px. Not theme tokens: they belong to this screen.
pub mod size {
    /// Panel width. Figma 540.
    pub const PANEL_WIDTH: f32 = 540.0;
    /// Panel height. Figma "bottom drawer" 540 × 620.
    pub const PANEL_HEIGHT: f32 = 620.0;
    /// Card width. Figma 480.
    pub const CARD_WIDTH: f32 = 480.0;
    /// Compact card height. Figma 96.
    pub const CARD_HEIGHT: f32 = 96.0;
    /// Icon / avatar. Figma 52.
    pub const ICON: f32 = 52.0;
    /// Count badge, top-right of the icon. Figma 24.
    pub const COUNT_BADGE: f32 = 24.0;
    /// App badge, bottom-right of the icon. Figma 22.
    pub const APP_BADGE: f32 = 22.0;
    /// "Clear all" chip. Figma 96 × 28.
    pub const CHIP_HEIGHT: f32 = 28.0;
    /// "Options" swipe button. Figma 113 × 96.
    pub const OPTIONS_WIDTH: f32 = 113.0;
    /// "Clear" swipe button. Figma 226 × 96.
    pub const CLEAR_WIDTH: f32 = 226.0;
    /// Icon action token. Figma 48.
    pub const ACTION_ICON: f32 = 48.0;
    /// Text action token height. Figma 36.
    pub const ACTION_CHIP_HEIGHT: f32 = 36.0;
    /// Visible cards before the list peeks. Figma "3 + 1 peeking".
    pub const VISIBLE_CARDS: usize = 3;
}

/// Swipe thresholds, as a fraction of the card width.
pub mod swipe {
    /// Options + Clear start to show and stick. Figma 20 %.
    pub const REVEAL_MIN: f32 = 0.20;
    /// Upper end of the sticky reveal. Figma 60 %.
    pub const REVEAL_MAX: f32 = 0.60;
    /// Past this the card is dismissed. Figma 65 %.
    pub const DISMISS: f32 = 0.65;
}
