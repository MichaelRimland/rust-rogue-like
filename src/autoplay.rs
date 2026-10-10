//! Unattended runs for smoke tests and benchmarks (native builds only).
//!
//! ```text
//! cargo run --release -- --autoplay 20                  # dungeon mode, 20 s
//! cargo run --release -- --autoplay 20 --endless        # endless mode
//! cargo run --release -- --autoplay 20 --enemies 5000   # plus a crowd for benchmarks
//! cargo run -- --autoplay 30 --screenshot shot.png      # also save a screenshot
//! ```
//!
//! The bot starts a run by itself, walks towards the nearest enemy (so the missiles, which
//! fire in the facing direction without a cursor, hit something), walks through open doors,
//! picks the first upgrade at every level-up and exits with frame-time statistics. A panic
//! shows up as a non-zero exit code, which makes this a cheap end-to-end smoke test.

use bevy::{
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};

use crate::{
    AppState, GamePhase,
    assets::GameAssets,
    bench::{Bench, spawn_enemy_burst},
    dungeon::{DebugStart, Door, Dungeon, GameMode},
    enemy::Enemy,
    logic::dungeon::RoomKind,
    logic::stats::Upgrade,
    player::Player,
    run::{GameRng, RunStats, Stats},
    save::SaveDisabled,
};

pub struct AutoplayPlugin;

impl Plugin for AutoplayPlugin {
    fn build(&self, app: &mut App) {
        let Some(config) = AutoplayConfig::from_args(std::env::args()) else {
            return;
        };
        if let Some(floor) = config.start_floor {
            app.insert_resource(DebugStart {
                floor,
                boss_room: config.boss_room,
            });
        }
        app.insert_resource(config)
            .insert_resource(SaveDisabled)
            .init_resource::<AutoplayStats>()
            .add_systems(PreUpdate, autoplay_input.after(InputSystems))
            .add_systems(Update, autoplay_run.run_if(in_state(AppState::InGame)))
            .add_systems(Update, capture_scene);
    }
}

/// Default time of the optional screenshot, in seconds of play (`--screenshot-at` changes it).
const SCREENSHOT_AFTER_SECS: f32 = 8.0;
/// The bot stops steering when it is this close to its target.
const ARRIVED_DISTANCE: f32 = 12.0;

#[derive(Resource)]
struct AutoplayConfig {
    seconds: f32,
    enemies: usize,
    endless: bool,
    /// Start with every upgrade at its maximum level (`--maxed`), to exercise them all.
    maxed: bool,
    /// Do not pick upgrades automatically (to test the mouse selection by hand).
    hold_level_up: bool,
    /// Dungeon only: begin on this floor (`--floor N`), in its boss room with `--boss`.
    start_floor: Option<u32>,
    boss_room: bool,
    /// Instead of playing, show a menu screen (`menu` or `shrine`), save the screenshot
    /// and exit. Handy for checking the UI.
    scene: Option<String>,
    /// Saves a screenshot of the window to this path after a few seconds of play.
    screenshot: Option<String>,
    screenshot_at: f32,
}

impl AutoplayConfig {
    fn from_args(args: impl Iterator<Item = String>) -> Option<Self> {
        let args: Vec<String> = args.collect();
        let number = |flag: &str| {
            args.iter()
                .position(|arg| arg == flag)
                .and_then(|i| args.get(i + 1))
                .and_then(|v| v.parse::<f32>().ok())
        };
        let screenshot = args
            .iter()
            .position(|arg| arg == "--screenshot")
            .and_then(|i| args.get(i + 1))
            .cloned();
        let scene = args
            .iter()
            .position(|arg| arg == "--scene")
            .and_then(|i| args.get(i + 1))
            .cloned();
        let boss_room = args.iter().any(|arg| arg == "--boss");
        let start_floor = number("--floor")
            .map(|floor| floor as u32)
            .or(boss_room.then_some(1));
        Some(Self {
            maxed: args.iter().any(|arg| arg == "--maxed"),
            hold_level_up: args.iter().any(|arg| arg == "--hold-levelup"),
            start_floor,
            boss_room,
            scene,
            seconds: number("--autoplay")?,
            enemies: number("--enemies").unwrap_or(0.0) as usize,
            endless: args.iter().any(|arg| arg == "--endless"),
            screenshot,
            screenshot_at: number("--screenshot-at").unwrap_or(SCREENSHOT_AFTER_SECS),
        })
    }
}

#[derive(Resource, Default)]
struct AutoplayStats {
    started: bool,
    screenshot_taken: bool,
    next_report: f32,
    run_time: f32,
    frame_times: Vec<f32>,
}

/// Presses the keys a player would: confirm in menus, pick card 1, steer towards the goal.
fn autoplay_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    config: Res<AutoplayConfig>,
    state: Res<State<AppState>>,
    phase: Option<Res<State<GamePhase>>>,
    stats: Res<AutoplayStats>,
    dungeon: Res<Dungeon>,
    player: Query<&Transform, With<Player>>,
    enemies: Query<&Transform, With<Enemy>>,
    doors: Query<&Transform, With<Door>>,
) {
    // Nothing releases the keys we press, so start every frame from a clean slate.
    keys.release_all();
    if let Some(scene) = &config.scene {
        // Scene mode only navigates to the requested screen.
        if *state.get() == AppState::MainMenu && scene == "shrine" {
            keys.press(KeyCode::Digit3);
        }
        return;
    }
    match state.get() {
        AppState::MainMenu => {
            keys.press(if config.endless {
                KeyCode::Digit2
            } else {
                KeyCode::Enter
            });
        }
        AppState::GameOver => keys.press(KeyCode::Enter),
        AppState::Shrine => keys.press(KeyCode::Escape),
        AppState::InGame => {
            if !config.hold_level_up
                && phase.is_some_and(|phase| *phase.get() == GamePhase::LevelUp)
            {
                keys.press(KeyCode::Digit1);
            }
            let Ok(player) = player.single() else {
                return;
            };
            let here = player.translation.truncate();
            let nearest = |transforms: &mut dyn Iterator<Item = &Transform>| {
                transforms.map(|t| t.translation.truncate()).min_by(|a, b| {
                    a.distance_squared(here)
                        .total_cmp(&b.distance_squared(here))
                })
            };

            let target = nearest(&mut enemies.iter())
                .or_else(|| {
                    if dungeon.is_cleared() {
                        nearest(&mut doors.iter())
                    } else if !dungeon.kind.is_combat() && dungeon.kind != RoomKind::Shop {
                        // An unopened treasure chest in the middle of the room.
                        Some(Vec2::new(0.0, 40.0))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    // Nothing to chase: walk a slow circle.
                    let angle = stats.run_time * 0.8;
                    here + Vec2::from_angle(angle) * 100.0
                });

            let offset = target - here;
            if offset.length() > ARRIVED_DISTANCE {
                let threshold = 6.0;
                if offset.x > threshold {
                    keys.press(KeyCode::KeyD);
                } else if offset.x < -threshold {
                    keys.press(KeyCode::KeyA);
                }
                if offset.y > threshold {
                    keys.press(KeyCode::KeyW);
                } else if offset.y < -threshold {
                    keys.press(KeyCode::KeyS);
                }
            }
            keys.press(KeyCode::Space);
        }
    }
}

fn autoplay_run(
    mut commands: Commands,
    time: Res<Time<Real>>,
    config: Res<AutoplayConfig>,
    assets: Res<GameAssets>,
    run_stats: Res<RunStats>,
    dungeon: Res<Dungeon>,
    mut stats: ResMut<AutoplayStats>,
    mut bench: ResMut<Bench>,
    mut rng: ResMut<GameRng>,
    mut mode: ResMut<GameMode>,
    mut stats_resource: ResMut<Stats>,
    phase: Option<Res<State<GamePhase>>>,
    mut last_phase: Local<Option<GamePhase>>,
    player: Query<&Transform, With<Player>>,
    enemies: Query<(), With<Enemy>>,
    mut exit: MessageWriter<AppExit>,
) {
    if let Some(phase) = phase {
        let phase = *phase.get();
        if *last_phase != Some(phase) {
            println!("[phase] {:?} -> {:?}", *last_phase, phase);
            *last_phase = Some(phase);
        }
    }
    let Ok(player) = player.single() else {
        return;
    };
    if !stats.started {
        stats.started = true;
        // Nothing may kill the player during a benchmark.
        bench.god_mode = true;
        if config.endless {
            *mode = GameMode::Endless;
        }
        if config.maxed {
            for upgrade in Upgrade::ALL {
                for _ in 0..6 {
                    upgrade.apply(&mut stats_resource);
                }
            }
        }
        spawn_enemy_burst(
            &mut commands,
            &assets,
            &mut rng,
            player.translation.truncate(),
            config.enemies,
        );
    }

    stats.run_time += time.delta_secs();
    if stats.run_time >= stats.next_report {
        stats.next_report += 20.0;
        println!(
            "[{:>5.0}s] floor {} room {} {:?} {:?}, {} enemies, {} kills, {} gold, player at ({:.0}, {:.0})",
            stats.run_time,
            dungeon.floor,
            run_stats.room,
            dungeon.kind,
            dungeon.state,
            enemies.iter().count(),
            run_stats.kills,
            run_stats.gold,
            player.translation.x,
            player.translation.y,
        );
    }
    if let Some(path) = &config.screenshot
        && !stats.screenshot_taken
        && stats.run_time > config.screenshot_at
    {
        stats.screenshot_taken = true;
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.clone()));
    }
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
         {} enemies alive | floor {} room {} gold {} kills {}",
        stats.run_time,
        frames.len(),
        average_ms,
        1000.0 / average_ms.max(f32::EPSILON),
        percentile(0.99),
        percentile(1.0),
        enemies.iter().count(),
        run_stats.floor,
        run_stats.room,
        run_stats.gold,
        run_stats.kills,
    );
    exit.write(AppExit::Success);
}

/// `--scene`: waits for the screen to settle, saves a screenshot and exits.
fn capture_scene(
    mut commands: Commands,
    time: Res<Time<Real>>,
    config: Res<AutoplayConfig>,
    mut stats: ResMut<AutoplayStats>,
    mut exit: MessageWriter<AppExit>,
) {
    if config.scene.is_none() {
        return;
    }
    stats.run_time += time.delta_secs();
    if let Some(path) = &config.screenshot
        && !stats.screenshot_taken
        && stats.run_time > 3.0
    {
        stats.screenshot_taken = true;
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.clone()));
    }
    if stats.run_time > 5.0 {
        exit.write(AppExit::Success);
    }
}
