//! Unattended runs for smoke tests and benchmarks (native builds only).
//!
//! ```text
//! cargo run --release -- --autoplay 20            # play 20 s, then print stats and exit
//! cargo run --release -- --autoplay 20 --enemies 5000
//! ```
//!
//! The game starts a run by itself, walks in circles, picks the first upgrade at every
//! level-up and exits with the frame-time statistics. A panic therefore shows up as a
//! non-zero exit code, which makes this a cheap end-to-end smoke test.

use bevy::{input::InputSystems, prelude::*};

use crate::{
    AppState, GamePhase,
    assets::GameAssets,
    bench::{Bench, spawn_enemy_burst},
    enemy::Enemy,
    player::Player,
    run::GameRng,
};

pub struct AutoplayPlugin;

impl Plugin for AutoplayPlugin {
    fn build(&self, app: &mut App) {
        let Some(config) = AutoplayConfig::from_args(std::env::args()) else {
            return;
        };
        app.insert_resource(config)
            .init_resource::<AutoplayStats>()
            .add_systems(PreUpdate, autoplay_input.after(InputSystems))
            .add_systems(Update, autoplay_run.run_if(in_state(AppState::InGame)));
    }
}

#[derive(Resource)]
struct AutoplayConfig {
    seconds: f32,
    enemies: usize,
}

impl AutoplayConfig {
    fn from_args(args: impl Iterator<Item = String>) -> Option<Self> {
        let args: Vec<String> = args.collect();
        let value = |flag: &str| {
            args.iter()
                .position(|arg| arg == flag)
                .and_then(|i| args.get(i + 1))
                .and_then(|v| v.parse::<f32>().ok())
        };
        Some(Self {
            seconds: value("--autoplay")?,
            enemies: value("--enemies").unwrap_or(0.0) as usize,
        })
    }
}

#[derive(Resource, Default)]
struct AutoplayStats {
    started: bool,
    run_time: f32,
    frame_times: Vec<f32>,
}

/// Presses the keys a player would: confirm in menus, pick card 1, walk in circles.
fn autoplay_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    state: Res<State<AppState>>,
    phase: Option<Res<State<GamePhase>>>,
    stats: Res<AutoplayStats>,
) {
    match state.get() {
        AppState::MainMenu | AppState::GameOver => keys.press(KeyCode::Enter),
        AppState::InGame => {
            if phase.is_some_and(|phase| *phase.get() == GamePhase::LevelUp) {
                keys.press(KeyCode::Digit1);
            }
            // Walk W, D, S, A in turns of two seconds each, and dash/blink when possible.
            let key = match (stats.run_time / 2.0) as u32 % 4 {
                0 => KeyCode::KeyW,
                1 => KeyCode::KeyD,
                2 => KeyCode::KeyS,
                _ => KeyCode::KeyA,
            };
            keys.press(key);
            keys.press(KeyCode::Space);
        }
    }
}

fn autoplay_run(
    mut commands: Commands,
    time: Res<Time<Real>>,
    config: Res<AutoplayConfig>,
    assets: Res<GameAssets>,
    mut stats: ResMut<AutoplayStats>,
    mut bench: ResMut<Bench>,
    mut rng: ResMut<GameRng>,
    player: Query<&Transform, With<Player>>,
    enemies: Query<(), With<Enemy>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    if !stats.started {
        stats.started = true;
        // Nothing may kill the player during a benchmark.
        bench.god_mode = true;
        spawn_enemy_burst(
            &mut commands,
            &assets,
            &mut rng,
            player.translation.truncate(),
            config.enemies,
        );
    }

    stats.run_time += time.delta_secs();
    // Skip the first second: asset loading and shader compilation distort it.
    if stats.run_time > 1.0 {
        stats.frame_times.push(time.delta_secs());
    }
    if stats.run_time < config.seconds {
        return;
    }

    let mut frames = stats.frame_times.clone();
    frames.sort_by(f32::total_cmp);
    let total: f32 = frames.iter().sum();
    let average_ms = total / frames.len().max(1) as f32 * 1000.0;
    let percentile = |p: f32| {
        let index = ((frames.len() as f32 * p) as usize).min(frames.len().saturating_sub(1));
        frames.get(index).copied().unwrap_or(0.0) * 1000.0
    };
    println!(
        "AUTOPLAY OK: {:.1}s, {} frames, avg {:.2} ms ({:.0} FPS), p99 {:.2} ms, max {:.2} ms, \
         {} enemies alive",
        stats.run_time,
        frames.len(),
        average_ms,
        1000.0 / average_ms.max(f32::EPSILON),
        percentile(0.99),
        percentile(1.0),
        enemies.iter().count(),
    );
    exit.write(AppExit::Success);
}
