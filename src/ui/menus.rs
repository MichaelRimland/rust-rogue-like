//! Main menu, pause overlay and game over screen.

use bevy::prelude::*;

use super::{
    ACCENT_COLOR, MUTED_TEXT_COLOR, OVERLAY_COLOR, TEXT_COLOR, centered_column, format_time, label,
};
use crate::{
    AppState, GamePhase,
    dungeon::{Dungeon, GameMode},
    logic::dungeon::FLOORS,
    run::{RunStats, Xp},
};

pub struct MenusPlugin;

impl Plugin for MenusPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::MainMenu), spawn_main_menu)
            .add_systems(Update, main_menu_input.run_if(in_state(AppState::MainMenu)))
            .add_systems(OnEnter(GamePhase::Paused), spawn_pause_overlay)
            .add_systems(OnEnter(AppState::GameOver), spawn_game_over)
            .add_systems(Update, game_over_input.run_if(in_state(AppState::GameOver)));
    }
}

const CONFIRM_KEYS: [KeyCode; 3] = [KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space];

fn spawn_main_menu(mut commands: Commands) {
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
            label("Enter / Space  -  Dungeon", 18.0, MUTED_TEXT_COLOR),
        ],
    ));
}

fn main_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<GameMode>,
    mut next: ResMut<NextState<AppState>>,
) {
    if keys.any_just_pressed([KeyCode::Digit2, KeyCode::Numpad2]) {
        *mode = GameMode::Endless;
        next.set(AppState::InGame);
    } else if keys.any_just_pressed([KeyCode::Digit1, KeyCode::Numpad1])
        || keys.any_just_pressed(CONFIRM_KEYS)
    {
        *mode = GameMode::Dungeon;
        next.set(AppState::InGame);
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
