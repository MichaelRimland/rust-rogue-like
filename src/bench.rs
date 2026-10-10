//! Developer tools for the performance milestone: a frame-time overlay and a stress test.
//!
//! - `F3` toggles the overlay (FPS, frame time, enemy count).
//! - `F4` during a run adds a burst of enemies around the player and enables god mode, so
//!   the crowd can be pushed towards the 5 000 enemy target without dying.

use std::f32::consts::TAU;

use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
};

use crate::{
    AppState, GameSet,
    assets::GameAssets,
    enemy::{Enemy, spawn_enemy},
    logic::enemies::pick_enemy_kind,
    player::{Health, Player},
    run::GameRng,
    ui::{TEXT_COLOR, label},
};

pub struct BenchPlugin;

impl Plugin for BenchPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .init_resource::<Bench>()
            .add_systems(Startup, spawn_overlay)
            .add_systems(OnEnter(AppState::InGame), reset_bench)
            .add_systems(
                Update,
                (toggle_overlay, update_overlay, spawn_burst).chain(),
            )
            .add_systems(
                Update,
                keep_player_alive
                    .run_if(|bench: Res<Bench>| bench.god_mode)
                    .in_set(GameSet::Damage),
            );
    }
}

/// Number of enemies added per `F4` press.
const BURST_SIZE: usize = 500;
/// Burst enemies appear in a ring this far from the player (inside the visible area).
const BURST_DISTANCE: (f32, f32) = (250.0, 900.0);
/// Run time used to pick enemy kinds, so bursts contain every kind.
const BURST_KIND_TIME: f32 = 600.0;

#[derive(Resource, Default)]
pub(crate) struct Bench {
    overlay: bool,
    pub(crate) god_mode: bool,
}

#[derive(Component)]
struct OverlayText;

fn reset_bench(mut bench: ResMut<Bench>) {
    bench.god_mode = false;
}

fn spawn_overlay(mut commands: Commands) {
    commands.spawn((
        OverlayText,
        label("", 14.0, TEXT_COLOR),
        Node {
            position_type: PositionType::Absolute,
            top: px(20.0),
            right: px(16.0),
            ..default()
        },
        Visibility::Hidden,
    ));
}

fn toggle_overlay(keys: Res<ButtonInput<KeyCode>>, mut bench: ResMut<Bench>) {
    if keys.just_pressed(KeyCode::F3) {
        bench.overlay = !bench.overlay;
    }
}

fn update_overlay(
    bench: Res<Bench>,
    diagnostics: Res<DiagnosticsStore>,
    enemies: Query<(), With<Enemy>>,
    overlay: Single<(&mut Text, &mut Visibility), With<OverlayText>>,
) {
    let (mut text, mut visibility) = overlay.into_inner();
    *visibility = if bench.overlay {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if !bench.overlay {
        return;
    }

    let smoothed = |id| {
        diagnostics
            .get(id)
            .and_then(|diagnostic| diagnostic.smoothed())
            .unwrap_or(0.0)
    };
    text.0 = format!(
        "{:.0} FPS | {:.1} ms\nenemies: {}\nF4: +{BURST_SIZE} enemies{}",
        smoothed(&FrameTimeDiagnosticsPlugin::FPS),
        smoothed(&FrameTimeDiagnosticsPlugin::FRAME_TIME),
        enemies.iter().count(),
        if bench.god_mode { " (god mode)" } else { "" },
    );
}

fn spawn_burst(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<AppState>>,
    assets: Res<GameAssets>,
    mut bench: ResMut<Bench>,
    mut rng: ResMut<GameRng>,
    player: Query<&Transform, With<Player>>,
) {
    if !keys.just_pressed(KeyCode::F4) || *state.get() != AppState::InGame {
        return;
    }
    let Ok(player) = player.single() else {
        return;
    };
    bench.god_mode = true;
    spawn_enemy_burst(
        &mut commands,
        &assets,
        &mut rng,
        player.translation.truncate(),
        BURST_SIZE,
    );
}

/// Spawns `count` enemies of every kind in a ring around `center`.
pub(crate) fn spawn_enemy_burst(
    commands: &mut Commands,
    assets: &GameAssets,
    rng: &mut GameRng,
    center: Vec2,
    count: usize,
) {
    for _ in 0..count {
        let kind = pick_enemy_kind(&mut rng.0, BURST_KIND_TIME);
        let angle = rng.0.range_f32(0.0, TAU);
        let distance = rng.0.range_f32(BURST_DISTANCE.0, BURST_DISTANCE.1);
        let position = center + Vec2::from_angle(angle) * distance;
        spawn_enemy(commands, assets, kind, position, 1.0);
    }
}

fn keep_player_alive(mut health: Single<&mut Health, With<Player>>) {
    health.current = health.max;
}
