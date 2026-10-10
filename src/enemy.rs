//! Enemy spawning, steering and the spatial grid used for every hit test.

use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::{
    AppState, GameSet,
    assets::GameAssets,
    logic::{
        enemies::{EnemyKind, pick_enemy_kind},
        progression::{enemy_health_multiplier, spawn_batch_size, spawn_interval},
        spatial::SpatialGrid,
    },
    player::Player,
    run::{GameRng, RunStats},
};

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(EnemyGrid(SpatialGrid::new(GRID_CELL_SIZE)))
            .init_resource::<SpawnTimer>()
            .add_systems(OnEnter(AppState::InGame), reset_spawner)
            .add_systems(Update, rebuild_grid.in_set(GameSet::Spatial))
            .add_systems(
                Update,
                (move_enemies, wrap_far_enemies, animate_hit_flash)
                    .chain()
                    .after(crate::player::move_player)
                    .in_set(GameSet::Movement),
            )
            .add_systems(Update, spawn_enemies.in_set(GameSet::Progression));
    }
}

const GRID_CELL_SIZE: f32 = 64.0;
/// Radius of the largest enemy; used to widen grid queries so no overlap is missed.
pub const MAX_ENEMY_RADIUS: f32 = 22.0;
const MAX_ENEMIES: usize = 500;
const SPAWN_DISTANCE: (f32, f32) = (760.0, 860.0);
/// Enemies farther away than this are moved back towards the player.
const WRAP_DISTANCE: f32 = 1_300.0;
const SEPARATION_STRENGTH: f32 = 4.0;
pub const HIT_FLASH_SECS: f32 = 0.1;

#[derive(Component)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub health: f32,
    pub speed: f32,
    pub radius: f32,
    pub contact_dps: f32,
    pub xp: u32,
    /// Remaining seconds of the "got hit" squash animation.
    pub hit_flash: f32,
}

/// All enemies bucketed by position. Rebuilt every frame in [`GameSet::Spatial`].
#[derive(Resource)]
pub struct EnemyGrid(pub SpatialGrid<Entity>);

#[derive(Resource, Default)]
struct SpawnTimer(f32);

fn reset_spawner(mut timer: ResMut<SpawnTimer>, mut grid: ResMut<EnemyGrid>) {
    // Give the player a moment before the first goblins arrive.
    timer.0 = 1.0;
    grid.0.clear();
}

fn rebuild_grid(mut grid: ResMut<EnemyGrid>, enemies: Query<(Entity, &Transform), With<Enemy>>) {
    grid.0.clear();
    for (entity, transform) in &enemies {
        grid.0
            .insert(entity, transform.translation.x, transform.translation.y);
    }
}

fn spawn_enemies(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    run: Res<RunStats>,
    mut rng: ResMut<GameRng>,
    mut timer: ResMut<SpawnTimer>,
    player: Single<&Transform, With<Player>>,
    enemies: Query<(), With<Enemy>>,
) {
    timer.0 -= time.delta_secs();
    if timer.0 > 0.0 {
        return;
    }
    timer.0 += spawn_interval(run.elapsed);

    let alive = enemies.iter().count();
    let batch = (spawn_batch_size(run.elapsed) as usize).min(MAX_ENEMIES.saturating_sub(alive));
    let health_multiplier = enemy_health_multiplier(run.elapsed);
    let center = player.translation.truncate();

    // A wave arrives roughly from one direction, which reads better than pure noise.
    let wave_angle = rng.0.range_f32(0.0, TAU);
    for _ in 0..batch {
        let kind = pick_enemy_kind(&mut rng.0, run.elapsed);
        let angle = wave_angle + rng.0.range_f32(-0.9, 0.9);
        let distance = rng.0.range_f32(SPAWN_DISTANCE.0, SPAWN_DISTANCE.1);
        let position = center + Vec2::from_angle(angle) * distance;
        spawn_enemy(&mut commands, &assets, kind, position, health_multiplier);
    }
}

pub(crate) fn spawn_enemy(
    commands: &mut Commands,
    assets: &GameAssets,
    kind: EnemyKind,
    position: Vec2,
    health_multiplier: f32,
) {
    let base = kind.archetype();
    let mesh = match kind {
        EnemyKind::Skeleton => assets.hexagon.clone(),
        EnemyKind::Goblin | EnemyKind::Ogre => assets.circle.clone(),
    };
    commands.spawn((
        Enemy {
            kind,
            health: base.health * health_multiplier,
            speed: base.speed,
            radius: base.radius,
            contact_dps: base.contact_dps,
            xp: base.xp,
            hit_flash: 0.0,
        },
        Mesh2d(mesh),
        MeshMaterial2d(assets.enemy_material(kind)),
        Transform::from_xyz(position.x, position.y, 2.0).with_scale(Vec3::splat(base.radius)),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Enemies walk straight at the player and push each other apart so they form a
/// crowd instead of collapsing into a single blob.
fn move_enemies(
    time: Res<Time>,
    grid: Res<EnemyGrid>,
    player: Single<&Transform, (With<Player>, Without<Enemy>)>,
    mut enemies: Query<(Entity, &mut Transform, &Enemy)>,
) {
    let dt = time.delta_secs();
    let target = player.translation.truncate();

    for (entity, mut transform, enemy) in &mut enemies {
        let position = transform.translation.truncate();
        let to_player = (target - position).normalize_or_zero();

        let mut push = Vec2::ZERO;
        let neighbour_radius = enemy.radius + MAX_ENEMY_RADIUS;
        grid.0.for_each_in_radius(
            position.x,
            position.y,
            neighbour_radius,
            |other, ox, oy, d2| {
                if other == entity || d2 < 1e-6 {
                    return;
                }
                let away = position - Vec2::new(ox, oy);
                let distance = d2.sqrt();
                // Stronger the more the two overlap; neighbours' exact radius is not
                // known here, so the widest possible one is assumed.
                let overlap = (neighbour_radius - distance) / neighbour_radius;
                push += away / distance * overlap;
            },
        );

        let velocity = to_player * enemy.speed + push * enemy.speed * SEPARATION_STRENGTH * 0.25;
        let delta = velocity * dt;
        transform.translation.x += delta.x;
        transform.translation.y += delta.y;
    }
}

fn animate_hit_flash(time: Res<Time>, mut enemies: Query<(&mut Transform, &mut Enemy)>) {
    let dt = time.delta_secs();
    for (mut transform, mut enemy) in &mut enemies {
        if enemy.hit_flash <= 0.0 {
            continue;
        }
        enemy.hit_flash = (enemy.hit_flash - dt).max(0.0);
        let squash = 1.0 + 0.35 * (enemy.hit_flash / HIT_FLASH_SECS);
        transform.scale = Vec3::new(enemy.radius * squash, enemy.radius / squash, 1.0);
    }
}

/// Enemies the player outran are moved to the opposite side of the screen, so the
/// pressure stays on without having to despawn and respawn them.
fn wrap_far_enemies(
    player: Single<&Transform, (With<Player>, Without<Enemy>)>,
    mut enemies: Query<&mut Transform, With<Enemy>>,
) {
    let center = player.translation.truncate();
    for mut transform in &mut enemies {
        let offset = transform.translation.truncate() - center;
        if offset.length_squared() > WRAP_DISTANCE * WRAP_DISTANCE {
            let new_position = center - offset.normalize() * SPAWN_DISTANCE.0;
            transform.translation.x = new_position.x;
            transform.translation.y = new_position.y;
        }
    }
}
