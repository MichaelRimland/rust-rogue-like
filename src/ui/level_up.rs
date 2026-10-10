//! The "choose an upgrade" screen.

use bevy::prelude::*;

use super::{ACCENT_COLOR, MUTED_TEXT_COLOR, OVERLAY_COLOR, PANEL_COLOR, TEXT_COLOR, label};
use crate::{
    GamePhase,
    progression::{LevelUpChoices, PendingLevelUps},
    run::Stats,
};

pub struct LevelUpUiPlugin;

impl Plugin for LevelUpUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            rebuild_level_up_screen
                .run_if(in_state(GamePhase::LevelUp))
                .run_if(resource_changed::<LevelUpChoices>),
        );
    }
}

#[derive(Component)]
struct LevelUpScreen;

fn rebuild_level_up_screen(
    mut commands: Commands,
    choices: Res<LevelUpChoices>,
    pending: Res<PendingLevelUps>,
    stats: Res<Stats>,
    existing: Query<Entity, With<LevelUpScreen>>,
) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    if choices.0.is_empty() {
        return;
    }

    let heading = if pending.0 > 1 {
        format!("Level up!  ({} more after this)", pending.0 - 1)
    } else {
        "Level up!".to_string()
    };

    let root = commands
        .spawn((
            LevelUpScreen,
            Node {
                width: percent(100.0),
                height: percent(100.0),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(24.0),
                ..default()
            },
            BackgroundColor(OVERLAY_COLOR),
            DespawnOnExit(GamePhase::LevelUp),
        ))
        .id();

    commands.spawn((label(heading, 48.0, ACCENT_COLOR), ChildOf(root)));

    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(20.0),
                ..default()
            },
            ChildOf(root),
        ))
        .id();

    for (index, upgrade) in choices.0.iter().enumerate() {
        commands.spawn((
            Node {
                width: px(260.0),
                min_height: px(150.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(12.0),
                padding: UiRect::all(px(16.0)),
                border: UiRect::all(px(2.0)),
                border_radius: BorderRadius::all(px(10.0)),
                ..default()
            },
            BackgroundColor(PANEL_COLOR),
            BorderColor::all(ACCENT_COLOR),
            ChildOf(row),
            children![
                label(format!("[{}]", index + 1), 22.0, ACCENT_COLOR),
                label(upgrade.title(&stats), 22.0, TEXT_COLOR),
                label(upgrade.description(&stats), 17.0, MUTED_TEXT_COLOR),
            ],
        ));
    }

    commands.spawn((
        label("Press 1, 2 or 3", 20.0, MUTED_TEXT_COLOR),
        ChildOf(root),
    ));
}
