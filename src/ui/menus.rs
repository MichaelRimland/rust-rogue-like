//! Main menu, pause overlay and game over screen.

use bevy::prelude::*;

use super::{
    ACCENT_COLOR, MUTED_TEXT_COLOR, OVERLAY_COLOR, TEXT_COLOR, centered_column, format_time, label,
    panel_image,
};
use crate::{
    AppState, GamePhase,
    assets::GameAssets,
    dungeon::{Dungeon, GameMode},
    logic::{
        dungeon::FLOORS,
        meta::{MAX_RANK, ShrineUpgrade},
    },
    run::{RunStats, Xp},
    save::{Meta, SaveDisabled, persist},
};

pub struct MenusPlugin;

impl Plugin for MenusPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MenuChoice>()
            .add_systems(OnEnter(AppState::MainMenu), spawn_main_menu)
            .add_systems(
                Update,
                (main_menu_input, place_portrait).run_if(in_state(AppState::MainMenu)),
            )
            .add_systems(OnEnter(AppState::Shrine), spawn_shrine)
            .add_systems(Update, shrine_input.run_if(in_state(AppState::Shrine)))
            .add_systems(OnEnter(GamePhase::Paused), spawn_pause_overlay)
            .add_systems(OnEnter(AppState::GameOver), spawn_game_over)
            .add_systems(Update, game_over_input.run_if(in_state(AppState::GameOver)));
    }
}

const CONFIRM_KEYS: [KeyCode; 3] = [KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space];

/// What was chosen in the main menu, by key or by mouse.
#[derive(Message, Debug, Clone, Copy)]
enum MenuChoice {
    Dungeon,
    Endless,
    Shrine,
}

/// The wizard shown above the main menu title.
#[derive(Component)]
struct MenuPortrait;

/// Shows the wizard above the title (as a world sprite with crisp pixels) and keeps him
/// centered, wherever the camera was left. The sprite is created here rather than with the
/// menu because the first menu appears before the game assets exist.
fn place_portrait(
    mut commands: Commands,
    assets: Res<GameAssets>,
    camera: Single<&Transform, (With<Camera2d>, Without<MenuPortrait>)>,
    portrait: Option<Single<&mut Transform, With<MenuPortrait>>>,
) {
    let position = camera.translation.truncate() + Vec2::new(0.0, 290.0);
    match portrait {
        Some(mut portrait) => {
            portrait.translation.x = position.x;
            portrait.translation.y = position.y;
        }
        None => {
            commands.spawn((
                MenuPortrait,
                Sprite::from_image(assets.wizard[0].clone()),
                Transform::from_xyz(position.x, position.y, 1.0).with_scale(Vec3::splat(3.0)),
                DespawnOnExit(AppState::MainMenu),
            ));
        }
    }
}

/// Tint of a menu button frame under the mouse (above 1.0, so it brightens).
const BUTTON_HOVER: Color = Color::srgb(1.5, 1.4, 1.1);

fn spawn_main_menu(mut commands: Commands, meta: Res<Meta>, asset_server: Res<AssetServer>) {
    // `GameAssets` does not exist yet at the first menu, so load the frame directly.
    let panel: Handle<Image> = asset_server.load("sprites/panel.png");
    let meta = &meta.0;
    let records = format!(
        "Shards {}   Runs {}   Victories {}   Best floor {}   Best survival {}",
        meta.shards,
        meta.runs,
        meta.victories,
        meta.best_floor,
        format_time(meta.best_time)
    );

    let root = commands
        .spawn((centered_column(12.0), DespawnOnExit(AppState::MainMenu)))
        .id();

    // Room for the wizard portrait, which `place_portrait` puts above the title.
    commands.spawn((
        Node {
            height: px(112.0),
            ..default()
        },
        ChildOf(root),
    ));
    commands.spawn((label("HEXFALL", 72.0, ACCENT_COLOR), ChildOf(root)));
    commands.spawn((
        label("Fight through the cursed dungeon.", 22.0, TEXT_COLOR),
        ChildOf(root),
    ));
    commands.spawn((
        Node {
            height: px(10.0),
            ..default()
        },
        ChildOf(root),
    ));

    menu_button(
        &mut commands,
        root,
        &panel,
        "1",
        "Dungeon",
        "Rounds, shops and bosses - three floors to the end boss",
        MenuChoice::Dungeon,
    );
    menu_button(
        &mut commands,
        root,
        &panel,
        "2",
        "Endless survival",
        "How long can you last in the cursed forest?",
        MenuChoice::Endless,
    );
    menu_button(
        &mut commands,
        root,
        &panel,
        "3",
        "Shrine",
        "Spend soul shards on permanent upgrades",
        MenuChoice::Shrine,
    );

    for (text, size, color) in [
        (records, 16.0, ACCENT_COLOR),
        (
            "WASD move  -  mouse aims  -  Space dash  -  right mouse blink  -  Esc pause"
                .to_string(),
            15.0,
            MUTED_TEXT_COLOR,
        ),
    ] {
        commands.spawn((label(text, size, color), ChildOf(root)));
    }
}

/// One clickable menu panel; the number key does the same thing.
fn menu_button(
    commands: &mut Commands,
    parent: Entity,
    panel: &Handle<Image>,
    key: &str,
    title: &str,
    subtitle: &str,
    choice: MenuChoice,
) {
    commands
        .spawn((
            Node {
                width: px(700.0),
                height: px(112.0),
                flex_shrink: 0.0,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(36.0), px(16.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(2.0),
                ..default()
            },
            panel_image(panel.clone()),
            ChildOf(parent),
            children![
                label(format!("[{key}]  {title}"), 26.0, TEXT_COLOR),
                label(subtitle.to_string(), 16.0, MUTED_TEXT_COLOR),
            ],
        ))
        .observe(
            move |_click: On<PointerClick>, mut picks: MessageWriter<MenuChoice>| {
                picks.write(choice);
            },
        )
        .observe(
            |over: On<PointerOver>, mut buttons: Query<&mut ImageNode>| {
                if let Ok(mut button) = buttons.get_mut(over.event_target()) {
                    button.color = BUTTON_HOVER;
                }
            },
        )
        .observe(|out: On<PointerOut>, mut buttons: Query<&mut ImageNode>| {
            if let Ok(mut button) = buttons.get_mut(out.event_target()) {
                button.color = Color::WHITE;
            }
        });
}

fn main_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut clicks: MessageReader<MenuChoice>,
    mut mode: ResMut<GameMode>,
    mut next: ResMut<NextState<AppState>>,
) {
    let from_keys = if keys.any_just_pressed([KeyCode::Digit3, KeyCode::Numpad3]) {
        Some(MenuChoice::Shrine)
    } else if keys.any_just_pressed([KeyCode::Digit2, KeyCode::Numpad2]) {
        Some(MenuChoice::Endless)
    } else if keys.any_just_pressed([KeyCode::Digit1, KeyCode::Numpad1])
        || keys.any_just_pressed(CONFIRM_KEYS)
    {
        Some(MenuChoice::Dungeon)
    } else {
        None
    };
    match clicks.read().last().copied().or(from_keys) {
        Some(MenuChoice::Shrine) => next.set(AppState::Shrine),
        Some(MenuChoice::Endless) => {
            *mode = GameMode::Endless;
            next.set(AppState::InGame);
        }
        Some(MenuChoice::Dungeon) => {
            *mode = GameMode::Dungeon;
            next.set(AppState::InGame);
        }
        None => {}
    }
}

fn spawn_shrine(mut commands: Commands, meta: Res<Meta>) {
    let meta = &meta.0;
    commands
        .spawn((centered_column(16.0), DespawnOnExit(AppState::Shrine)))
        .with_children(|parent| {
            parent.spawn(label("THE SHRINE", 64.0, ACCENT_COLOR));
            parent.spawn(label(
                format!("{} soul shards", meta.shards),
                28.0,
                TEXT_COLOR,
            ));
            parent.spawn(Node {
                height: px(16.0),
                ..default()
            });
            for (index, upgrade) in ShrineUpgrade::ALL.into_iter().enumerate() {
                let cost = match meta.cost(upgrade) {
                    Some(cost) => format!("cost {cost}"),
                    None => "MAX".to_string(),
                };
                let color = if meta.can_buy(upgrade) {
                    TEXT_COLOR
                } else {
                    MUTED_TEXT_COLOR
                };
                parent.spawn(label(
                    format!(
                        "{}  -  {}  rank {}/{}   {}   ({})",
                        index + 1,
                        upgrade.name(),
                        meta.rank(upgrade),
                        MAX_RANK,
                        upgrade.description(),
                        cost
                    ),
                    22.0,
                    color,
                ));
            }
            parent.spawn(Node {
                height: px(16.0),
                ..default()
            });
            parent.spawn(label(
                "Esc  -  back to the main menu",
                18.0,
                MUTED_TEXT_COLOR,
            ));
        });
}

fn shrine_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut meta: ResMut<Meta>,
    disabled: Option<Res<SaveDisabled>>,
    mut next: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::MainMenu);
        return;
    }
    const KEYS: [[KeyCode; 2]; 3] = [
        [KeyCode::Digit1, KeyCode::Numpad1],
        [KeyCode::Digit2, KeyCode::Numpad2],
        [KeyCode::Digit3, KeyCode::Numpad3],
    ];
    for (upgrade, keys_for) in ShrineUpgrade::ALL.into_iter().zip(KEYS) {
        if keys.any_just_pressed(keys_for) && meta.0.buy(upgrade) {
            persist(&meta.0, disabled.is_some());
            // Entering the state again rebuilds the screen with the new numbers.
            next.set(AppState::Shrine);
        }
    }
}

fn spawn_pause_overlay(mut commands: Commands) {
    commands.spawn((
        centered_column(12.0),
        BackgroundColor(OVERLAY_COLOR),
        // Paused is a sub-state of InGame, so this also goes away when the run ends.
        DespawnOnExit(GamePhase::Paused),
        children![
            label("Paused", 56.0, ACCENT_COLOR),
            label("Press Esc to continue", 22.0, TEXT_COLOR),
        ],
    ));
}

fn spawn_game_over(
    mut commands: Commands,
    run: Res<RunStats>,
    xp: Res<Xp>,
    mode: Res<GameMode>,
    dungeon: Res<Dungeon>,
) {
    let (title, color) = if run.victory {
        ("Victory!", Color::srgb(0.98, 0.80, 0.40))
    } else {
        ("You have fallen", Color::srgb(0.90, 0.30, 0.30))
    };
    let progress = match *mode {
        GameMode::Dungeon => format!(
            "Reached floor {} of {}, room {}   -   {} gold",
            dungeon.floor, FLOORS, run.room, run.gold
        ),
        GameMode::Endless => format!("Survived {}", format_time(run.elapsed)),
    };
    commands.spawn((
        centered_column(14.0),
        BackgroundColor(OVERLAY_COLOR),
        DespawnOnExit(AppState::GameOver),
        children![
            label(title, 64.0, color),
            label(progress, 26.0, TEXT_COLOR),
            label(
                format!("Time {}", format_time(run.elapsed)),
                22.0,
                TEXT_COLOR
            ),
            label(format!("Reached level {}", xp.level), 22.0, TEXT_COLOR),
            label(format!("Defeated {} monsters", run.kills), 22.0, TEXT_COLOR),
            label(format!("Seed {}", run.seed), 16.0, MUTED_TEXT_COLOR),
            (Node {
                height: px(24.0),
                ..default()
            },),
            label("Enter - try again      Esc - main menu", 22.0, ACCENT_COLOR),
        ],
    ));
}

fn game_over_input(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.any_just_pressed(CONFIRM_KEYS) {
        next.set(AppState::InGame);
    } else if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::MainMenu);
    }
}
