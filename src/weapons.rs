//! The wizard's spells: Arcane Missile, Holy Aura and Orbiting Runes.

use std::f32::consts::TAU;

use bevy::{prelude::*, window::PrimaryWindow};

use crate::{
    AppState, GameSet,
    assets::GameAssets,
    combat::DamageEnemy,
    enemy::{Enemy, EnemyGrid, MAX_ENEMY_RADIUS},
    logic::stats::{AuraStats, MissileStats, RuneStats},
    player::Player,
    run::Stats,
    world::cursor_world_position,
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
                    fade_bursts,
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
                    burn_enemies,
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
/// Angle in radians between the bolts a missile forks into.
const FORK_SPACING: f32 = 0.8;
/// Forked bolts burn out sooner than the original.
const FORK_LIFETIME: f32 = 1.0;
/// How far a chaining bolt looks for its next enemy.
const CHAIN_RANGE: f32 = 280.0;
/// A chained bolt lives at least this long, so it can reach its next target.
const CHAIN_LIFETIME: f32 = 1.2;
/// Seconds between a volley and each of its echoes.
const ECHO_DELAY: f32 = 0.2;
const BURN_DURATION: f32 = 3.0;
const BURN_TICK: f32 = 0.5;
const BURST_SECS: f32 = 0.25;

#[derive(Resource, Default)]
struct WeaponTimers {
    missile_cooldown: f32,
    /// Seconds until each queued echo volley is fired (Spell Echo).
    pending_echoes: Vec<f32>,
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
    /// Whether the bolt still splits on its next hit (Fork support).
    can_fork: bool,
    /// How many times the bolt may still jump to a new enemy (Chain support).
    chains_left: u32,
    /// Enemies already hit, so a piercing bolt damages each enemy only once.
    hit: Vec<Entity>,
}

/// Damage over time applied by Searing Touch.
#[derive(Component)]
struct Burning {
    dps: f32,
    remaining: f32,
    tick: f32,
}

/// The short expanding ring shown for Impact Burst.
#[derive(Component)]
struct BurstRing {
    lifetime: f32,
    radius: f32,
}

fn spawn_projectile(
    commands: &mut Commands,
    assets: &GameAssets,
    position: Vec2,
    projectile: Projectile,
) {
    commands.spawn((
        projectile,
        Mesh2d(assets.circle.clone()),
        MeshMaterial2d(assets.missile.clone()),
        Transform::from_xyz(position.x, position.y, 4.0).with_scale(Vec3::splat(MISSILE_RADIUS)),
        DespawnOnExit(AppState::InGame),
    ));
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

/// Fires a fan of bolts towards the mouse cursor whenever the cooldown is ready, plus the
/// echoes of earlier volleys (Spell Echo).
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
    let dt = time.delta_secs();
    timers.missile_cooldown -= dt;

    let mut due_echoes = 0;
    timers.pending_echoes.retain_mut(|delay| {
        *delay -= dt;
        if *delay <= 0.0 {
            due_echoes += 1;
            false
        } else {
            true
        }
    });
    let main_ready = timers.missile_cooldown <= 0.0;
    if !main_ready && due_echoes == 0 {
        return;
    }

    let (transform, player) = player.into_inner();
    let origin = transform.translation.truncate();

    // Aim at the cursor; keep the last move direction when the cursor is outside the window.
    let (camera, camera_transform) = *camera;
    let cursor_world = cursor_world_position(&window, camera, camera_transform);
    let aim = cursor_world
        .map(|target| (target - origin).normalize_or(player.facing))
        .unwrap_or(player.facing);

    if main_ready {
        timers.missile_cooldown = stats.missile.cooldown;
        spawn_volley(&mut commands, &assets, &stats, origin, aim, 1.0);
        for echo in 1..=stats.missile.echo_count() {
            timers.pending_echoes.push(echo as f32 * ECHO_DELAY);
        }
    }
    for _ in 0..due_echoes {
        let factor = MissileStats::ECHO_DAMAGE_FACTOR;
        spawn_volley(&mut commands, &assets, &stats, origin, aim, factor);
    }
}

/// Spawns one volley, spread symmetrically around `aim`.
fn spawn_volley(
    commands: &mut Commands,
    assets: &GameAssets,
    stats: &Stats,
    origin: Vec2,
    aim: Vec2,
    damage_factor: f32,
) {
    let count = stats.missile.count as usize;
    let middle = (count as f32 - 1.0) / 2.0;
    for i in 0..count {
        let direction = Vec2::from_angle((i as f32 - middle) * MISSILE_SPREAD).rotate(aim);
        spawn_projectile(
            commands,
            assets,
            origin,
            Projectile {
                velocity: direction * stats.missile.speed,
                damage: stats.missile.damage * damage_factor,
                pierce_left: stats.missile.pierce,
                lifetime: MISSILE_LIFETIME,
                can_fork: stats.missile.fork_count() > 0,
                chains_left: stats.missile.chain_count(),
                hit: Vec::new(),
            },
        );
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
    assets: Res<GameAssets>,
    stats: Res<Stats>,
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

            // Searing Touch: the enemy keeps taking damage for a few seconds.
            if stats.missile.burn_level > 0 {
                commands.entity(enemy_entity).try_insert(Burning {
                    dps: stats.missile.burn_dps(),
                    remaining: BURN_DURATION,
                    tick: BURN_TICK,
                });
            }

            // Impact Burst: weaker damage to everything around the hit.
            let burst_radius = stats.missile.burst_radius();
            if burst_radius > 0.0 {
                let amount = projectile.damage * MissileStats::BURST_DAMAGE_FACTOR;
                grid.0.for_each_in_radius(
                    position.x,
                    position.y,
                    burst_radius + MAX_ENEMY_RADIUS,
                    |other, _, _, d2| {
                        if other == enemy_entity {
                            return;
                        }
                        if let Ok(other_enemy) = enemies.get(other) {
                            let reach = burst_radius + other_enemy.radius;
                            if d2 <= reach * reach {
                                damage.write(DamageEnemy {
                                    entity: other,
                                    amount,
                                });
                            }
                        }
                    },
                );
                commands.spawn((
                    BurstRing {
                        lifetime: BURST_SECS,
                        radius: burst_radius,
                    },
                    Mesh2d(assets.circle.clone()),
                    MeshMaterial2d(assets.aura.clone()),
                    Transform::from_xyz(position.x, position.y, 3.0)
                        .with_scale(Vec3::splat(burst_radius * 0.4)),
                    DespawnOnExit(AppState::InGame),
                ));
            }

            // Fork: split into several weaker bolts fanning out from the hit.
            if projectile.can_fork {
                projectile.can_fork = false;
                fork_projectile(&mut commands, &assets, &stats, position, &projectile);
            }

            // Chain: redirect the bolt to the nearest enemy it has not touched yet.
            if projectile.chains_left > 0
                && let Some(target) = nearest_unhit(&grid, position, &projectile.hit)
            {
                projectile.chains_left -= 1;
                let speed = projectile.velocity.length();
                projectile.velocity = (target - position).normalize_or_zero() * speed;
                projectile.lifetime = projectile.lifetime.max(CHAIN_LIFETIME);
                break;
            }

            if projectile.pierce_left == 0 {
                commands.entity(entity).try_despawn();
                break;
            }
            projectile.pierce_left -= 1;
        }
    }
}

/// Position of the closest enemy within [`CHAIN_RANGE`] that is not in `hit`.
fn nearest_unhit(grid: &EnemyGrid, position: Vec2, hit: &[Entity]) -> Option<Vec2> {
    let mut best: Option<(f32, Vec2)> = None;
    grid.0
        .for_each_in_radius(position.x, position.y, CHAIN_RANGE, |entity, x, y, d2| {
            if hit.contains(&entity) {
                return;
            }
            if best.is_none_or(|(best_d2, _)| d2 < best_d2) {
                best = Some((d2, Vec2::new(x, y)));
            }
        });
    best.map(|(_, point)| point)
}

fn fade_bursts(
    mut commands: Commands,
    time: Res<Time>,
    mut rings: Query<(Entity, &mut Transform, &mut BurstRing)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut ring) in &mut rings {
        ring.lifetime -= dt;
        if ring.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let progress = 1.0 - ring.lifetime / BURST_SECS;
        transform.scale = Vec3::splat(ring.radius * (0.4 + 0.6 * progress));
    }
}

fn burn_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut burning: Query<(Entity, &mut Burning)>,
    mut damage: MessageWriter<DamageEnemy>,
) {
    let dt = time.delta_secs();
    for (entity, mut burn) in &mut burning {
        burn.remaining -= dt;
        burn.tick -= dt;
        if burn.tick <= 0.0 {
            burn.tick += BURN_TICK;
            damage.write(DamageEnemy {
                entity,
                amount: burn.dps * BURN_TICK,
            });
        }
        if burn.remaining <= 0.0 {
            commands.entity(entity).try_remove::<Burning>();
        }
    }
}

/// Spawns the forked bolts of `parent`, fanned symmetrically around its direction.
fn fork_projectile(
    commands: &mut Commands,
    assets: &GameAssets,
    stats: &Stats,
    position: Vec2,
    parent: &Projectile,
) {
    let count = stats.missile.fork_count();
    let direction = parent.velocity.normalize_or_zero();
    let speed = parent.velocity.length();
    let middle = (count as f32 - 1.0) / 2.0;
    for i in 0..count {
        let angle = (i as f32 - middle) * FORK_SPACING;
        spawn_projectile(
            commands,
            assets,
            position,
            Projectile {
                velocity: Vec2::from_angle(angle).rotate(direction) * speed,
                damage: parent.damage * MissileStats::FORK_DAMAGE_FACTOR,
                pierce_left: 0,
                lifetime: FORK_LIFETIME,
                can_fork: false,
                chains_left: 0,
                // The enemy that caused the fork must not be hit again.
                hit: parent.hit.clone(),
            },
        );
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
