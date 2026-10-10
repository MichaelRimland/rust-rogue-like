//! Main menu, pause overlay and game over screen.

use bevy::prelude::*;

use super::{
    ACCENT_COLOR, MUTED_TEXT_COLOR, OVERLAY_COLOR, TEXT_COLOR, centered_column, format_time, label,
};
use crate::{
    AppState, GamePhase,
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
        app.add_systems(OnEnter(AppState::MainMenu), spawn_main_menu)
            .add_systems(Update, main_menu_input.run_if(in_state(AppState::MainMenu)))
            .add_systems(OnEnter(AppState::Shrine), spawn_shrine)
            .add_systems(Update, shrine_input.run_if(in_state(AppState::Shrine)))
            .add_systems(OnEnter(GamePhase::Paused), spawn_pause_overlay)
            .add_systems(OnEnter(AppState::GameOver), spawn_game_over)
            .add_systems(Update, game_over_input.run_if(in_state(AppState::GameOver)));
    }
}

const CONFIRM_KEYS: [KeyCode; 3] = [KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space];

fn spawn_main_menu(mut commands: Commands, meta: Res<Meta>) {
    let meta = &meta.0;
    let records = format!(
        "Shards {}   Runs {}   Victories {}   Best floor {}   Best survival {}",
        meta.shards,
        meta.runs,
        meta.victories,
        meta.best_floor,
        format_time(meta.best_time)
    );
    commands.spawn((
        centered_column(18.0),
        DespawnOnExit(AppState::MainMenu),
        children![
            label("RUST ROGUE-LIKE", 72.0, ACCENT_COLOR),
            label("Fight through the cursed dungeon.", 22.0, TEXT_COLOR),
            (Node {
                height: px(30.0),
                ..default()
            },),
            label("WASD / Arrow keys  -  move", 18.0, MUTED_TEXT_COLOR),
            label(
                "Spells cast themselves towards the mouse cursor",
                18.0,
                MUTED_TEXT_COLOR
            ),
            label(
                "Space  -  dash      Right mouse  -  blink      (found as upgrades)",
                18.0,
                MUTED_TEXT_COLOR
            ),
            label(
                "1 / 2 / 3  -  choose upgrade      Esc  -  pause",
                18.0,
                MUTED_TEXT_COLOR
            ),
            (Node {
                height: px(30.0),
                ..default()
            },),
            label("1  -  Dungeon (floors, rooms, bosses)", 26.0, TEXT_COLOR),
            label("2  -  Endless survival", 26.0, TEXT_COLOR),
            label("3  -  Shrine (permanent upgrades)", 26.0, TEXT_COLOR),
            label("Enter / Space  -  Dungeon", 18.0, MUTED_TEXT_COLOR),
            (Node {
                height: px(10.0),
                ..default()
            },),
            label(records, 16.0, ACCENT_COLOR),
        ],
    ));
}

fn main_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<GameMode>,
    mut next: ResMut<NextState<AppState>>,
) {
    if keys.any_just_pressed([KeyCode::Digit3, KeyCode::Numpad3]) {
        next.set(AppState::Shrine);
    } else if keys.any_just_pressed([KeyCode::Digit2, KeyCode::Numpad2]) {
        *mode = GameMode::Endless;
        next.set(AppState::InGame);
    } else if keys.any_just_pressed([KeyCode::Digit1, KeyCode::Numpad1])
        || keys.any_just_pressed(CONFIRM_KEYS)
    {
        *mode = GameMode::Dungeon;
        next.set(AppState::InGame);
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
