//! Camera and the endless forest floor.

use bevy::prelude::*;

use crate::{
    AppState, GameSet,
    assets::{GameAssets, SPRITE_SCALE},
    player::Player,
    run::GameRng,
};

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraShake>()
            .add_systems(Startup, (spawn_camera, spawn_ground.after(spawn_camera)))
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

/// The tiled forest floor, kept centered on the camera in whole tiles.
#[derive(Component)]
struct Ground;

/// Size of the floor sprite in world units; a multiple of the on-screen tile size so it
/// can be snapped to the tile grid, and larger than any window we expect.
const GROUND_SIZE: Vec2 = Vec2::new(3072.0, 1920.0);
const GROUND_TILE: f32 = 32.0 * SPRITE_SCALE;
/// Highest camera offset in world units at full trauma.
const SHAKE_MAX_OFFSET: f32 = 14.0;
/// How quickly trauma fades (per second).
const SHAKE_DECAY: f32 = 1.8;

/// Screen shake: events add trauma, the camera offset grows with its square.
#[derive(Resource, Default)]
pub struct CameraShake {
    trauma: f32,
    /// Where the camera would be without shake; shake is applied on top of it.
    base: Vec2,
}

impl CameraShake {
    pub fn add_trauma(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }
}

fn spawn_ground(mut commands: Commands, assets: Res<GameAssets>) {
    commands.spawn((
        Ground,
        Sprite {
            image: assets.ground.clone(),
            custom_size: Some(GROUND_SIZE),
            image_mode: SpriteImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: SPRITE_SCALE,
            },
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -20.0),
    ));
}

/// The mouse cursor in world coordinates, `None` while it is outside the window.
pub(crate) fn cursor_world_position(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
) -> Option<Vec2> {
    let cursor = window.cursor_position()?;
    camera.viewport_to_world_2d(camera_transform, cursor).ok()
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn reset_camera(
    mut camera: Single<&mut Transform, With<Camera2d>>,
    mut shake: ResMut<CameraShake>,
) {
    camera.translation.x = 0.0;
    camera.translation.y = 0.0;
    *shake = CameraShake::default();
}

/// Scatters bushes, stones and bones so the player can see that they are moving.
fn spawn_decor(mut commands: Commands, assets: Res<GameAssets>, mut rng: ResMut<GameRng>) {
    let weights: Vec<f32> = assets.decor.iter().map(|(_, weight)| *weight).collect();
    for _ in 0..DECOR_COUNT {
        let x = rng.0.range_f32(-DECOR_HALF_EXTENT.x, DECOR_HALF_EXTENT.x);
        let y = rng.0.range_f32(-DECOR_HALF_EXTENT.y, DECOR_HALF_EXTENT.y);
        let index = rng.0.weighted_index(&weights).unwrap_or(0);
        let mut sprite = Sprite::from_image(assets.decor[index].0.clone());
        sprite.flip_x = rng.0.chance(0.5);
        commands.spawn((
            Decor,
            sprite,
            Transform::from_xyz(x, y, -10.0).with_scale(Vec3::splat(SPRITE_SCALE)),
            DespawnOnExit(AppState::InGame),
        ));
    }
}

fn follow_player(
    time: Res<Time>,
    player: Single<&Transform, (With<Player>, Without<Camera2d>)>,
    mut camera: Single<&mut Transform, With<Camera2d>>,
    mut ground: Single<&mut Transform, (With<Ground>, Without<Camera2d>, Without<Player>)>,
    mut shake: ResMut<CameraShake>,
) {
    let dt = time.delta_secs();
    let target = player.translation.truncate();
    // Frame-rate independent exponential smoothing.
    let t = 1.0 - (-CAMERA_FOLLOW_SPEED * dt).exp();
    shake.base = shake.base.lerp(target, t);

    // Cheap deterministic "noise" from the clock; no RNG needed for a visual effect.
    let clock = time.elapsed_secs();
    let noise = Vec2::new((clock * 61.0).sin(), (clock * 47.0 + 1.3).cos());
    let offset = noise * SHAKE_MAX_OFFSET * shake.trauma * shake.trauma;
    shake.trauma = (shake.trauma - SHAKE_DECAY * dt).max(0.0);

    let position = shake.base + offset;
    camera.translation.x = position.x;
    camera.translation.y = position.y;

    // The floor snaps to whole tiles, so the pattern never visibly slides.
    ground.translation.x = (shake.base.x / GROUND_TILE).round() * GROUND_TILE;
    ground.translation.y = (shake.base.y / GROUND_TILE).round() * GROUND_TILE;
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
