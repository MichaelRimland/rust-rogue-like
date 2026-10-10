//! All screens and overlays.

mod ambience;
mod hud;
mod level_up;
mod menus;

use bevy::prelude::*;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ambience::AmbiencePlugin,
            hud::HudPlugin,
            level_up::LevelUpUiPlugin,
            menus::MenusPlugin,
        ));
    }
}

pub const TEXT_COLOR: Color = Color::srgb(0.92, 0.90, 0.84);
pub const MUTED_TEXT_COLOR: Color = Color::srgb(0.62, 0.64, 0.60);
pub const ACCENT_COLOR: Color = Color::srgb(0.98, 0.80, 0.40);
pub const PANEL_COLOR: Color = Color::srgba(0.06, 0.07, 0.10, 0.92);
pub const OVERLAY_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.6);

/// A nine-slice frame image for panels. The corners are drawn up to three times larger,
/// so the carved border keeps its proportions on big cards.
pub fn panel_image(image: Handle<Image>) -> ImageNode {
    let mut node = ImageNode::new(image);
    node.image_mode = NodeImageMode::Sliced(TextureSlicer {
        border: BorderRect::all(8.0),
        max_corner_scale: 3.0,
        ..default()
    });
    node
}

/// The pixel font is drawn smaller than Bevy's default font at the same nominal size.
pub const FONT_SCALE: f32 = 1.3;

/// A text node with a fixed pixel size.
pub fn label(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(size * FONT_SCALE),
            ..default()
        },
        TextColor(color),
    )
}

/// A full-screen node that centers its children in a column.
pub fn centered_column(gap: f32) -> Node {
    Node {
        width: percent(100.0),
        height: percent(100.0),
        position_type: PositionType::Absolute,
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        row_gap: px(gap),
        ..default()
    }
}

/// Formats seconds as `mm:ss`.
pub fn format_time(seconds: f32) -> String {
    let total = seconds.max(0.0) as u32;
    format!("{:02}:{:02}", total / 60, total % 60)
}
