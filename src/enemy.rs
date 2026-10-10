//! Enemy spawning, steering and the spatial grid used for every hit test.

use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::{
    AppState, GameSet,
    animation::WalkAnimation,
    assets::{GameAssets, SPRITE_SCALE},
    dungeon::{ARENA_HALF, GameMode, endless_mode},
    logic::{
        enemies::{EnemyKind, RangedAttack, pick_enemy_kind},
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
            .add_systems(Update, tick_statuses.in_set(GameSet::Movement))
            .add_systems(
                Update,
                (move_enemies, wrap_far_enemies, animate_hit_flash)
                    .chain()
                    .after(crate::player::move_player)
                    .in_set(GameSet::Movement),
            )
            .add_systems(
                Update,
                spawn_enemies
                    .in_set(GameSet::Progression)
                    .run_if(endless_mode),
            );
    }
}

const GRID_CELL_SIZE: f32 = 64.0;
/// Radius of the largest enemy; used to widen grid queries so no overlap is missed.
pub const MAX_ENEMY_RADIUS: f32 = 48.0;
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
    /// How large the sprite is drawn, as a multiple of [`SPRITE_SCALE`].
    pub visual_scale: f32,
    /// The color the sprite returns to after a hit flash.
    pub tint: Color,
    /// Remaining seconds of the "got hit" squash animation.
    pub hit_flash: f32,
    pub ranged: Option<RangedAttack>,
    /// Seconds until the next shot (ranged enemies only).
    pub shoot_timer: f32,
}

/// All enemies bucketed by position. Rebuilt every frame in [`GameSet::Spatial`].
#[derive(Resource)]
pub struct EnemyGrid(pub SpatialGrid<Entity>);

/// Slowed by Frostbite or Frost Nova: movement speed is multiplied by `factor`.
#[derive(Component)]
pub struct Slowed {
    pub factor: f32,
    pub remaining: f32,
}

/// Pushed away by Concussive Force.
#[derive(Component)]
pub struct Knockback {
    pub velocity: Vec2,
    pub remaining: f32,
}

/// Shocked: takes `bonus` (a fraction) more damage.
#[derive(Component)]
pub struct Vulnerable {
    pub bonus: f32,
    pub remaining: f32,
}

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

/// Everything that can differ between spawned enemies.
pub(crate) struct EnemySpawn {
    pub kind: EnemyKind,
    pub position: Vec2,
    pub health_multiplier: f32,
    /// Scales the collision radius.
    pub radius_factor: f32,
    /// Scales the drawn sprite.
    pub visual_factor: f32,
    /// Multiplies the experience dropped.
    pub xp_factor: u32,
    pub tint: Color,
    /// Replaces the sprite frames of the enemy kind (used by the end boss).
    pub frames: Option<[Handle<Image>; 2]>,
    /// `false` strips the kind's ranged attack (bosses have their own).
    pub ranged: bool,
}

impl EnemySpawn {
    pub fn new(kind: EnemyKind, position: Vec2, health_multiplier: f32) -> Self {
        Self {
            kind,
            position,
            health_multiplier,
            radius_factor: 1.0,
            visual_factor: 1.0,
            xp_factor: 1,
            tint: Color::WHITE,
            frames: None,
            ranged: true,
        }
    }
}

pub(crate) fn spawn_enemy(
    commands: &mut Commands,
    assets: &GameAssets,
    kind: EnemyKind,
    position: Vec2,
    health_multiplier: f32,
) {
    spawn_enemy_with(
        commands,
        assets,
        EnemySpawn::new(kind, position, health_multiplier),
    );
}

pub(crate) fn spawn_enemy_with(
    commands: &mut Commands,
    assets: &GameAssets,
    spawn: EnemySpawn,
) -> Entity {
    let base = spawn.kind.archetype();
    let frames = spawn
        .frames
        .clone()
        .unwrap_or_else(|| assets.enemy_frames(spawn.kind));
    // Different phases keep a crowd from stepping in unison.
    let phase = spawn.position.x.abs() as u32;
    let visual_scale = SPRITE_SCALE * spawn.visual_factor;
    let mut sprite = Sprite::from_image(frames[0].clone());
    sprite.color = spawn.tint;
    commands
        .spawn((
            Enemy {
                kind: spawn.kind,
                health: base.health * spawn.health_multiplier,
                speed: base.speed,
                radius: base.radius * spawn.radius_factor,
                contact_dps: base.contact_dps,
                xp: base.xp * spawn.xp_factor,
                visual_scale,
                tint: spawn.tint,
                hit_flash: 0.0,
                ranged: base.ranged.filter(|_| spawn.ranged),
                // Stagger first shots so a group of archers does not fire in unison.
                shoot_timer: base.ranged.map_or(0.0, |ranged| {
                    ranged.cooldown * (0.4 + 0.1 * (phase % 6) as f32)
                }),
            },
            sprite,
            WalkAnimation::new(frames, phase),
            Transform::from_xyz(spawn.position.x, spawn.position.y, 2.0)
                .with_scale(Vec3::splat(visual_scale)),
            DespawnOnExit(AppState::InGame),
            // A soft shadow under the feet; the coordinates are in sprite pixels.
            children![(
                Mesh2d(assets.circle.clone()),
                MeshMaterial2d(assets.shadow.clone()),
                Transform::from_xyz(0.0, -13.0, -0.05).with_scale(Vec3::new(
                    (base.radius * 0.9).clamp(7.0, 13.0),
                    3.6,
                    1.0,
                )),
            )],
        ))
        .id()
}

/// Enemies walk straight at the player and push each other apart so they form a
/// crowd instead of collapsing into a single blob.
fn move_enemies(
    time: Res<Time>,
    grid: Res<EnemyGrid>,
    mode: Res<GameMode>,
    player: Single<&Transform, (With<Player>, Without<Enemy>)>,
    mut enemies: Query<(
        Entity,
        &mut Transform,
        &mut Sprite,
        &Enemy,
        Option<&Slowed>,
        Option<&Knockback>,
    )>,
) {
    let dt = time.delta_secs();
    let target = player.translation.truncate();

    for (entity, mut transform, mut sprite, enemy, slowed, knockback) in &mut enemies {
        let position = transform.translation.truncate();
        let offset = target - position;
        let to_player = offset.normalize_or_zero();
        // Ranged enemies hold their preferred distance instead of closing in.
        let approach = enemy
            .ranged
            .map_or(1.0, |ranged| ranged.approach_factor(offset.length()));

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

        let slow = slowed.map_or(1.0, |slowed| slowed.factor);
        let mut velocity = to_player * enemy.speed * approach * slow
            + push * enemy.speed * SEPARATION_STRENGTH * 0.25;
        if let Some(knockback) = knockback {
            velocity += knockback.velocity;
        }
        let delta = velocity * dt;
        transform.translation.x += delta.x;
        transform.translation.y += delta.y;
        if *mode == GameMode::Dungeon {
            // Rooms have walls: kiting archers must not retreat out of the arena.
            let limit = ARENA_HALF - Vec2::splat(enemy.radius.min(30.0));
            transform.translation.x = transform.translation.x.clamp(-limit.x, limit.x);
            transform.translation.y = transform.translation.y.clamp(-limit.y, limit.y);
        }
        // Enemies lower on screen are drawn in front of the ones behind them.
        transform.translation.z = 2.0 - transform.translation.y * 0.0001;
        // The sprites look to the right by default.
        let face_left = to_player.x < 0.0;
        if to_player.x != 0.0 && sprite.flip_x != face_left {
            sprite.flip_x = face_left;
        }
    }
}

/// Counts down slows, knockbacks and shocks and removes them when they run out.
fn tick_statuses(
    mut commands: Commands,
    time: Res<Time>,
    mut slowed: Query<(Entity, &mut Slowed)>,
    mut pushed: Query<(Entity, &mut Knockback)>,
    mut shocked: Query<(Entity, &mut Vulnerable)>,
) {
    let dt = time.delta_secs();
    for (entity, mut status) in &mut slowed {
        status.remaining -= dt;
        if status.remaining <= 0.0 {
            commands.entity(entity).try_remove::<Slowed>();
        }
    }
    for (entity, mut status) in &mut pushed {
        status.remaining -= dt;
        if status.remaining <= 0.0 {
            commands.entity(entity).try_remove::<Knockback>();
        }
    }
    for (entity, mut status) in &mut shocked {
        status.remaining -= dt;
        if status.remaining <= 0.0 {
            commands.entity(entity).try_remove::<Vulnerable>();
        }
    }
}

/// Sprite color of an enemy for a moment after it is hit.
const HIT_TINT: Color = Color::srgb(2.6, 2.6, 2.6);

fn animate_hit_flash(
    time: Res<Time>,
    mut enemies: Query<(&mut Transform, &mut Enemy, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    for (mut transform, mut enemy, mut sprite) in &mut enemies {
        if enemy.hit_flash <= 0.0 {
            continue;
        }
        enemy.hit_flash = (enemy.hit_flash - dt).max(0.0);
        sprite.color = if enemy.hit_flash > 0.0 {
            HIT_TINT
        } else {
            enemy.tint
        };
        let squash = 1.0 + 0.35 * (enemy.hit_flash / HIT_FLASH_SECS);
        transform.scale = Vec3::new(
            enemy.visual_scale * squash,
            enemy.visual_scale / squash,
            1.0,
        );
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
