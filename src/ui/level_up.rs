//! The "choose an upgrade" screen.

use bevy::prelude::*;

use super::{ACCENT_COLOR, MUTED_TEXT_COLOR, OVERLAY_COLOR, TEXT_COLOR, label, panel_image};
use crate::{
    GamePhase,
    assets::GameAssets,
    progression::{ChooseUpgrade, LevelUpChoices, PendingLevelUps},
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

/// Background of the card under the mouse.
const HOVER_COLOR: Color = Color::srgb(1.5, 1.4, 1.1);

#[derive(Component)]
struct LevelUpScreen;

fn rebuild_level_up_screen(
    mut commands: Commands,
    choices: Res<LevelUpChoices>,
    pending: Res<PendingLevelUps>,
    stats: Res<Stats>,
    assets: Res<GameAssets>,
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
        commands
            .spawn((
                Node {
                    width: px(320.0),
                    min_height: px(290.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(10.0),
                    padding: UiRect::all(px(30.0)),
                    ..default()
                },
                panel_image(assets.panel.clone()),
                ChildOf(row),
                children![
                    label(format!("[{}]", index + 1), 22.0, ACCENT_COLOR),
                    (
                        ImageNode::new(assets.icon(*upgrade)),
                        Node {
                            width: px(64.0),
                            height: px(64.0),
                            ..default()
                        },
                    ),
                    (label(upgrade.title(&stats), 20.0, TEXT_COLOR), wrapped(),),
                    (
                        label(upgrade.description(&stats), 15.0, MUTED_TEXT_COLOR),
                        wrapped(),
                    ),
                ],
            ))
            // Cards can be picked with the mouse: hover brightens the frame, a click chooses.
            .observe(
                move |_click: On<PointerClick>, mut picks: MessageWriter<ChooseUpgrade>| {
                    picks.write(ChooseUpgrade(index));
                },
            )
            .observe(|over: On<PointerOver>, mut cards: Query<&mut ImageNode>| {
                if let Ok(mut card) = cards.get_mut(over.event_target()) {
                    card.color = HOVER_COLOR;
                }
            })
            .observe(|out: On<PointerOut>, mut cards: Query<&mut ImageNode>| {
                if let Ok(mut card) = cards.get_mut(out.event_target()) {
                    card.color = Color::WHITE;
                }
            });
    }

    commands.spawn((
        label("Click a card or press 1, 2 or 3", 20.0, MUTED_TEXT_COLOR),
        ChildOf(root),
    ));
}

/// Makes a card text fill the card's inner width, so long lines wrap inside the frame.
fn wrapped() -> impl Bundle {
    (
        Node {
            width: px(250.0),
            ..default()
        },
        TextLayout::default().with_justify(Justify::Center),
    )
}
