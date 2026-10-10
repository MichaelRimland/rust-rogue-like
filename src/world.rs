//! Camera and the endless forest floor.

use bevy::prelude::*;

use crate::{AppState, GameSet, assets::GameAssets, player::Player, run::GameRng};

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera)
            .add_systems(
                OnEnter(AppState::InGame),
                (reset_camera, spawn_decor.after(crate::run::start_run)),
            )
            .add_systems(
                Update,
                (follow_player, recycle_decor)
                    .chain()
                    .after(GameSet::Movement)
                    .before(GameSet::Combat)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

/// Half extents of the area the decoration tiles wrap around in.
/// Must be larger than half the visible area so wrapping happens off screen.
const DECOR_HALF_EXTENT: Vec2 = Vec2::new(900.0, 600.0);
const DECOR_COUNT: usize = 420;
/// How quickly the camera catches up with the player (higher = snappier).
const CAMERA_FOLLOW_SPEED: f32 = 8.0;

#[derive(Component)]
struct Decor;

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn reset_camera(mut camera: Single<&mut Transform, With<Camera2d>>) {
    camera.translation.x = 0.0;
    camera.translation.y = 0.0;
}

/// Scatters bushes and stones so the player can see that they are moving.
fn spawn_decor(mut commands: Commands, assets: Res<GameAssets>, mut rng: ResMut<GameRng>) {
    for _ in 0..DECOR_COUNT {
        let x = rng.0.range_f32(-DECOR_HALF_EXTENT.x, DECOR_HALF_EXTENT.x);
        let y = rng.0.range_f32(-DECOR_HALF_EXTENT.y, DECOR_HALF_EXTENT.y);
        let size = rng.0.range_f32(3.0, 14.0);
        let mesh = if rng.0.chance(0.7) {
            assets.circle.clone()
        } else {
            assets.hexagon.clone()
        };
        let material = assets.decor[rng.0.below(assets.decor.len())].clone();
        commands.spawn((
            Decor,
            Mesh2d(mesh),
            MeshMaterial2d(material),
            Transform::from_xyz(x, y, -10.0).with_scale(Vec3::splat(size)),
            DespawnOnExit(AppState::InGame),
        ));
    }
}

fn follow_player(
    time: Res<Time>,
    player: Single<&Transform, (With<Player>, Without<Camera2d>)>,
    mut camera: Single<&mut Transform, With<Camera2d>>,
) {
    let target = player.translation.truncate();
    let current = camera.translation.truncate();
    // Frame-rate independent exponential smoothing.
    let t = 1.0 - (-CAMERA_FOLLOW_SPEED * time.delta_secs()).exp();
    let next = current.lerp(target, t);
    camera.translation.x = next.x;
    camera.translation.y = next.y;
}

/// Decoration that falls out of the area around the camera is moved to the opposite
/// side, which makes a few hundred entities look like an endless forest.
fn recycle_decor(
    camera: Single<&Transform, (With<Camera2d>, Without<Decor>)>,
    mut decor: Query<&mut Transform, With<Decor>>,
) {
    let center = camera.translation.truncate();
    let size = DECOR_HALF_EXTENT * 2.0;
    for mut transform in &mut decor {
        let offset = transform.translation.truncate() - center;
        if offset.x > DECOR_HALF_EXTENT.x {
            transform.translation.x -= size.x;
        } else if offset.x < -DECOR_HALF_EXTENT.x {
            transform.translation.x += size.x;
        }
        if offset.y > DECOR_HALF_EXTENT.y {
            transform.translation.y -= size.y;
        } else if offset.y < -DECOR_HALF_EXTENT.y {
            transform.translation.y += size.y;
        }
    }
}
