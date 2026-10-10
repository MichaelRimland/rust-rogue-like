//! The wizard's spells: Arcane Missile, Holy Aura and Orbiting Runes.

use std::f32::consts::TAU;

use bevy::{prelude::*, window::PrimaryWindow};

use crate::{
    AppState, GameSet,
    assets::GameAssets,
    combat::DamageEnemy,
    enemy::{Enemy, EnemyGrid, MAX_ENEMY_RADIUS},
    logic::stats::{AuraStats, RuneStats},
    player::Player,
    run::Stats,
};

pub struct WeaponsPlugin;

impl Plugin for WeaponsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeaponTimers>()
            .add_systems(OnEnter(AppState::InGame), (reset_weapons, spawn_aura))
            .add_systems(
                Update,
                (
                    move_projectiles,
                    update_aura_visual,
                    sync_runes,
                    orbit_runes,
                )
                    .after(crate::player::move_player)
                    .in_set(GameSet::Movement),
            )
            .add_systems(
                Update,
                (
                    (fire_missiles, projectile_hits).chain(),
                    aura_damage,
                    rune_damage,
                )
                    .in_set(GameSet::Combat),
            );
    }
}

/// Generous hit radius: bolts are aimed by hand, so they should forgive small misses.
const MISSILE_RADIUS: f32 = 8.0;
const MISSILE_LIFETIME: f32 = 2.0;
/// Angle in radians between neighbouring bolts of a volley.
const MISSILE_SPREAD: f32 = 0.25;

#[derive(Resource, Default)]
struct WeaponTimers {
    missile_cooldown: f32,
    aura_tick: f32,
    rune_tick: f32,
    rune_angle: f32,
}

#[derive(Component)]
struct Projectile {
    velocity: Vec2,
    damage: f32,
    pierce_left: u32,
    lifetime: f32,
    /// Enemies already hit, so a piercing bolt damages each enemy only once.
    hit: Vec<Entity>,
}

#[derive(Component)]
struct AuraVisual;

#[derive(Component)]
struct Rune {
    index: u32,
}

fn reset_weapons(mut timers: ResMut<WeaponTimers>) {
    *timers = WeaponTimers::default();
}

fn spawn_aura(mut commands: Commands, assets: Res<GameAssets>) {
    commands.spawn((
        AuraVisual,
        Mesh2d(assets.circle.clone()),
        MeshMaterial2d(assets.aura.clone()),
        Transform::from_xyz(0.0, 0.0, 0.5),
        Visibility::Hidden,
        DespawnOnExit(AppState::InGame),
    ));
}

// ---------------------------------------------------------------------------
// Arcane Missile
// ---------------------------------------------------------------------------

/// Fires a fan of bolts towards the mouse cursor whenever the cooldown is ready.
fn fire_missiles(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    stats: Res<Stats>,
    mut timers: ResMut<WeaponTimers>,
    player: Single<(&Transform, &Player)>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
) {
    timers.missile_cooldown -= time.delta_secs();
    if timers.missile_cooldown > 0.0 {
        return;
    }
    timers.missile_cooldown = stats.missile.cooldown;

    let (transform, player) = player.into_inner();
    let origin = transform.translation.truncate();

    // Aim at the cursor; keep the last move direction when the cursor is outside the window.
    let (camera, camera_transform) = *camera;
    let cursor_world = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world_2d(camera_transform, cursor).ok());
    let base = cursor_world
        .map(|target| (target - origin).normalize_or(player.facing))
        .unwrap_or(player.facing);

    // Spread the volley symmetrically around the aim direction.
    let count = stats.missile.count as usize;
    let middle = (count as f32 - 1.0) / 2.0;
    for i in 0..count {
        let direction = Vec2::from_angle((i as f32 - middle) * MISSILE_SPREAD).rotate(base);

        commands.spawn((
            Projectile {
                velocity: direction * stats.missile.speed,
                damage: stats.missile.damage,
                pierce_left: stats.missile.pierce,
                lifetime: MISSILE_LIFETIME,
                hit: Vec::new(),
            },
            Mesh2d(assets.circle.clone()),
            MeshMaterial2d(assets.missile.clone()),
            Transform::from_xyz(origin.x, origin.y, 4.0).with_scale(Vec3::splat(MISSILE_RADIUS)),
            DespawnOnExit(AppState::InGame),
        ));
    }
}

fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    mut projectiles: Query<(Entity, &mut Transform, &mut Projectile)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut projectile) in &mut projectiles {
        projectile.lifetime -= dt;
        if projectile.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let delta = projectile.velocity * dt;
        transform.translation.x += delta.x;
        transform.translation.y += delta.y;
    }
}

fn projectile_hits(
    mut commands: Commands,
    grid: Res<EnemyGrid>,
    enemies: Query<&Enemy>,
    mut projectiles: Query<(Entity, &Transform, &mut Projectile)>,
    mut damage: MessageWriter<DamageEnemy>,
) {
    let mut candidates = Vec::new();
    for (entity, transform, mut projectile) in &mut projectiles {
        let position = transform.translation.truncate();
        candidates.clear();
        grid.0.for_each_in_radius(
            position.x,
            position.y,
            MISSILE_RADIUS + MAX_ENEMY_RADIUS,
            |enemy, _, _, d2| candidates.push((enemy, d2)),
        );
        candidates.sort_by(|a, b| a.1.total_cmp(&b.1));

        for &(enemy_entity, d2) in &candidates {
            let Ok(enemy) = enemies.get(enemy_entity) else {
                continue;
            };
            let reach = MISSILE_RADIUS + enemy.radius;
            if d2 > reach * reach || projectile.hit.contains(&enemy_entity) {
                continue;
            }
            damage.write(DamageEnemy {
                entity: enemy_entity,
                amount: projectile.damage,
            });
            projectile.hit.push(enemy_entity);
            if projectile.pierce_left == 0 {
                commands.entity(entity).try_despawn();
                break;
            }
            projectile.pierce_left -= 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Holy Aura
// ---------------------------------------------------------------------------

fn update_aura_visual(
    stats: Res<Stats>,
    player: Single<&Transform, (With<Player>, Without<AuraVisual>)>,
    aura: Single<(&mut Transform, &mut Visibility), With<AuraVisual>>,
) {
    let (mut transform, mut visibility) = aura.into_inner();
    if stats.aura.level == 0 {
        *visibility = Visibility::Hidden;
        return;
    }
    *visibility = Visibility::Visible;
    transform.translation.x = player.translation.x;
    transform.translation.y = player.translation.y;
    transform.scale = Vec3::splat(stats.aura.radius());
}

fn aura_damage(
    time: Res<Time>,
    stats: Res<Stats>,
    grid: Res<EnemyGrid>,
    enemies: Query<&Enemy>,
    mut timers: ResMut<WeaponTimers>,
    player: Single<&Transform, With<Player>>,
    mut damage: MessageWriter<DamageEnemy>,
) {
    if stats.aura.level == 0 {
        return;
    }
    timers.aura_tick -= time.delta_secs();
    if timers.aura_tick > 0.0 {
        return;
    }
    timers.aura_tick += AuraStats::TICK;

    let radius = stats.aura.radius();
    let amount = stats.aura.damage_per_tick();
    let position = player.translation.truncate();
    grid.0.for_each_in_radius(
        position.x,
        position.y,
        radius + MAX_ENEMY_RADIUS,
        |entity, _, _, d2| {
            if let Ok(enemy) = enemies.get(entity) {
                let reach = radius + enemy.radius;
                if d2 <= reach * reach {
                    damage.write(DamageEnemy { entity, amount });
                }
            }
        },
    );
}

// ---------------------------------------------------------------------------
// Orbiting Runes
// ---------------------------------------------------------------------------

/// Spawns or despawns rune entities so their number matches the current rune level.
fn sync_runes(
    mut commands: Commands,
    assets: Res<GameAssets>,
    stats: Res<Stats>,
    runes: Query<(Entity, &Rune)>,
) {
    let wanted = stats.runes.count();
    let existing = runes.iter().count() as u32;
    if existing == wanted {
        return;
    }
    for (entity, _) in &runes {
        commands.entity(entity).try_despawn();
    }
    for index in 0..wanted {
        commands.spawn((
            Rune { index },
            Mesh2d(assets.hexagon.clone()),
            MeshMaterial2d(assets.rune.clone()),
            Transform::from_xyz(0.0, 0.0, 3.5).with_scale(Vec3::splat(RuneStats::HIT_RADIUS)),
            DespawnOnExit(AppState::InGame),
        ));
    }
}

fn orbit_runes(
    time: Res<Time>,
    stats: Res<Stats>,
    mut timers: ResMut<WeaponTimers>,
    player: Single<&Transform, (With<Player>, Without<Rune>)>,
    mut runes: Query<(&mut Transform, &Rune)>,
) {
    let count = stats.runes.count();
    if count == 0 {
        return;
    }
    timers.rune_angle = (timers.rune_angle + stats.runes.angular_speed() * time.delta_secs()) % TAU;
    let center = player.translation.truncate();
    for (mut transform, rune) in &mut runes {
        let angle = timers.rune_angle + rune.index as f32 * TAU / count as f32;
        let position = center + Vec2::from_angle(angle) * stats.runes.orbit_radius();
        transform.translation.x = position.x;
        transform.translation.y = position.y;
        transform.rotation = Quat::from_rotation_z(angle * 2.0);
    }
}

fn rune_damage(
    time: Res<Time>,
    stats: Res<Stats>,
    grid: Res<EnemyGrid>,
    enemies: Query<&Enemy>,
    mut timers: ResMut<WeaponTimers>,
    runes: Query<&Transform, With<Rune>>,
    mut damage: MessageWriter<DamageEnemy>,
) {
    if stats.runes.count() == 0 {
        return;
    }
    timers.rune_tick -= time.delta_secs();
    if timers.rune_tick > 0.0 {
        return;
    }
    timers.rune_tick += RuneStats::TICK;

    let amount = stats.runes.damage_per_tick();
    for transform in &runes {
        let position = transform.translation.truncate();
        grid.0.for_each_in_radius(
            position.x,
            position.y,
            RuneStats::HIT_RADIUS + MAX_ENEMY_RADIUS,
            |entity, _, _, d2| {
                if let Ok(enemy) = enemies.get(entity) {
                    let reach = RuneStats::HIT_RADIUS + enemy.radius;
                    if d2 <= reach * reach {
                        damage.write(DamageEnemy { entity, amount });
                    }
                }
            },
        );
    }
}
