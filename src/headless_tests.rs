//! Headless integration tests: the whole [`GamePlugin`] runs on `MinimalPlugins`, without a
//! window or renderer, and the tests drive it with simulated key presses.
//!
//! They cover the glue between the plugins (state flow, run setup, level-up, death), which
//! the engine-independent unit tests in `logic/` cannot see.

use std::time::Duration;

use bevy::{
    audio::AudioSource, input::InputSystems, prelude::*, state::app::StatesPlugin,
    time::TimeUpdateStrategy,
};

use crate::{
    AppState, GamePhase, GamePlugin,
    dungeon::GameMode,
    enemy::Enemy,
    logic::stats::Upgrade,
    player::{Health, Player},
    progression::{LevelUpChoices, PendingLevelUps},
    run::{RunStats, Stats, Xp},
    save::{Meta, SaveDisabled},
};

/// Fixed simulated frame time, so tests do not depend on how fast the machine is.
const FRAME: Duration = Duration::from_millis(16);

/// Keys to press during the next update (taps; they are released again right after).
#[derive(Resource, Default)]
struct KeyTaps(Vec<KeyCode>);

/// Applies [`KeyTaps`] after Bevy's own input system, which would otherwise clear a
/// `just_pressed` that was set from outside before the frame starts.
fn apply_taps(mut keys: ResMut<ButtonInput<KeyCode>>, mut taps: ResMut<KeyTaps>) {
    keys.release_all();
    for key in taps.0.drain(..) {
        keys.press(key);
    }
}

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        StatesPlugin,
        bevy::input::InputPlugin,
        AssetPlugin::default(),
    ))
    .init_asset::<Image>()
    .init_asset::<Mesh>()
    .init_asset::<ColorMaterial>()
    .init_asset::<Font>()
    .init_asset::<AudioSource>()
    .init_resource::<KeyTaps>()
    .insert_resource(TimeUpdateStrategy::ManualDuration(FRAME))
    .add_systems(PreUpdate, apply_taps.after(InputSystems))
    .add_plugins(GamePlugin)
    // After the plugins: neither the developer's `savegame.ron` nor a write may leak in.
    .insert_resource(Meta::default())
    .insert_resource(SaveDisabled);
    // The first update runs the startup schedules and the initial state transition.
    app.update();
    app
}

fn frames(app: &mut App, count: usize) {
    for _ in 0..count {
        app.update();
    }
}

/// Presses a key for one frame, then lets a few frames pass so state transitions apply.
fn tap(app: &mut App, key: KeyCode) {
    app.world_mut().resource_mut::<KeyTaps>().0.push(key);
    frames(app, 3);
}

fn app_state(app: &App) -> AppState {
    *app.world().resource::<State<AppState>>().get()
}

fn phase(app: &App) -> GamePhase {
    *app.world().resource::<State<GamePhase>>().get()
}

fn count<T: Component>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<(), With<T>>()
        .iter(app.world())
        .count()
}

/// An app with a freshly started dungeon run.
fn running_dungeon() -> App {
    let mut app = test_app();
    tap(&mut app, KeyCode::Enter);
    assert_eq!(app_state(&app), AppState::InGame);
    app
}

#[test]
fn boots_into_the_main_menu() {
    let app = test_app();
    assert_eq!(app_state(&app), AppState::MainMenu);
}

#[test]
fn enter_starts_a_dungeon_run_with_a_healthy_player() {
    let mut app = running_dungeon();

    assert_eq!(*app.world().resource::<GameMode>(), GameMode::Dungeon);
    assert_eq!(phase(&app), GamePhase::Running);
    assert_eq!(count::<Player>(&mut app), 1);

    let max_health = app.world().resource::<Stats>().max_health;
    let health = app
        .world_mut()
        .query_filtered::<&Health, With<Player>>()
        .single(app.world())
        .unwrap();
    assert_eq!(health.current, max_health);
    assert_eq!(app.world().resource::<Xp>().level, 1);
}

#[test]
fn key_two_starts_an_endless_run() {
    let mut app = test_app();
    tap(&mut app, KeyCode::Digit2);

    assert_eq!(app_state(&app), AppState::InGame);
    assert_eq!(*app.world().resource::<GameMode>(), GameMode::Endless);
}

#[test]
fn the_run_clock_ticks_and_enemies_arrive() {
    let mut app = running_dungeon();
    let before = app.world().resource::<RunStats>().elapsed;

    // About ten seconds of play.
    frames(&mut app, 600);

    assert!(app.world().resource::<RunStats>().elapsed > before + 5.0);
    assert!(count::<Enemy>(&mut app) > 0, "no enemy spawned in 10 s");
}

#[test]
fn pausing_freezes_the_run() {
    let mut app = running_dungeon();
    tap(&mut app, KeyCode::Escape);
    assert_eq!(phase(&app), GamePhase::Paused);

    let frozen = app.world().resource::<RunStats>().elapsed;
    frames(&mut app, 60);
    assert_eq!(app.world().resource::<RunStats>().elapsed, frozen);

    tap(&mut app, KeyCode::Escape);
    assert_eq!(phase(&app), GamePhase::Running);
    frames(&mut app, 60);
    assert!(app.world().resource::<RunStats>().elapsed > frozen);
}

#[test]
fn a_level_up_offers_cards_and_applies_the_choice() {
    let mut app = running_dungeon();
    let stats_before = app.world().resource::<Stats>().0.clone();

    app.world_mut().resource_mut::<PendingLevelUps>().0 = 1;
    frames(&mut app, 3);
    assert_eq!(phase(&app), GamePhase::LevelUp);

    let offered: Vec<Upgrade> = app.world().resource::<LevelUpChoices>().0.clone();
    assert_eq!(offered.len(), 3, "a level-up offers three cards");

    tap(&mut app, KeyCode::Digit1);
    assert_eq!(phase(&app), GamePhase::Running);
    assert_eq!(app.world().resource::<PendingLevelUps>().0, 0);
    assert_ne!(
        app.world().resource::<Stats>().0,
        stats_before,
        "picking {:?} must change the stats",
        offered[0]
    );
}

#[test]
fn stacked_level_ups_offer_a_fresh_set_each_time() {
    let mut app = running_dungeon();
    app.world_mut().resource_mut::<PendingLevelUps>().0 = 2;
    frames(&mut app, 3);
    assert_eq!(phase(&app), GamePhase::LevelUp);

    tap(&mut app, KeyCode::Digit1);
    assert_eq!(phase(&app), GamePhase::LevelUp, "second card set expected");
    assert_eq!(app.world().resource::<PendingLevelUps>().0, 1);

    tap(&mut app, KeyCode::Digit1);
    assert_eq!(phase(&app), GamePhase::Running);
}

#[test]
fn dying_ends_the_run_and_banks_the_result() {
    let mut app = running_dungeon();
    let mut query = app
        .world_mut()
        .query_filtered::<&mut Health, With<Player>>();
    query.single_mut(app.world_mut()).unwrap().current = 0.0;

    frames(&mut app, 3);

    assert_eq!(app_state(&app), AppState::GameOver);
    assert_eq!(app.world().resource::<Meta>().0.runs, 1);
}

#[test]
fn a_new_run_after_game_over_starts_from_scratch() {
    let mut app = running_dungeon();
    app.world_mut().resource_mut::<Xp>().level = 5;
    app.world_mut().resource_mut::<RunStats>().kills = 42;
    let mut query = app
        .world_mut()
        .query_filtered::<&mut Health, With<Player>>();
    query.single_mut(app.world_mut()).unwrap().current = 0.0;
    frames(&mut app, 3);
    assert_eq!(app_state(&app), AppState::GameOver);

    tap(&mut app, KeyCode::Enter);

    assert_eq!(app_state(&app), AppState::InGame);
    assert_eq!(app.world().resource::<Xp>().level, 1);
    assert_eq!(app.world().resource::<RunStats>().kills, 0);
    assert_eq!(count::<Player>(&mut app), 1);
}

#[test]
fn escape_on_game_over_returns_to_the_menu_and_clears_the_world() {
    let mut app = running_dungeon();
    frames(&mut app, 300);
    let mut query = app
        .world_mut()
        .query_filtered::<&mut Health, With<Player>>();
    query.single_mut(app.world_mut()).unwrap().current = 0.0;
    frames(&mut app, 3);

    tap(&mut app, KeyCode::Escape);

    assert_eq!(app_state(&app), AppState::MainMenu);
    assert_eq!(count::<Player>(&mut app), 0);
    assert_eq!(count::<Enemy>(&mut app), 0);
}
