//! Main menu, pause overlay and game over screen.

use bevy::prelude::*;

use super::{
    ACCENT_COLOR, MUTED_TEXT_COLOR, OVERLAY_COLOR, TEXT_COLOR, centered_column, format_time, label,
};
use crate::{
    AppState, GamePhase,
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
            label("Survive the night in the cursed forest.", 22.0, TEXT_COLOR),
            (Node {
                height: px(30.0),
                ..default()
            },),
            label("WASD / Arrow keys  -  move", 18.0, MUTED_TEXT_COLOR),
            label(
                "Spells cast themselves  -  just stay alive",
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
            label("Press Enter or Space to begin", 26.0, TEXT_COLOR),
        ],
    ));
}

fn main_menu_input(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.any_just_pressed(CONFIRM_KEYS) {
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

fn spawn_game_over(mut commands: Commands, run: Res<RunStats>, xp: Res<Xp>) {
    commands.spawn((
        centered_column(14.0),
        BackgroundColor(OVERLAY_COLOR),
        DespawnOnExit(AppState::GameOver),
        children![
            label("You have fallen", 64.0, Color::srgb(0.90, 0.30, 0.30)),
            label(
                format!("Survived {}", format_time(run.elapsed)),
                26.0,
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
