//! The wizard's spells: Arcane Missile, Holy Aura and Orbiting Runes.

use std::f32::consts::TAU;

use bevy::{prelude::*, window::PrimaryWindow};

use crate::{
    AppState, GameSet,
    assets::{GameAssets, SPRITE_SCALE},
    audio::{PlaySound, Sfx},
    combat::DamageEnemy,
    enemy::{Enemy, EnemyGrid, Knockback, MAX_ENEMY_RADIUS, Slowed, Vulnerable},
    logic::stats::{AuraStats, MissileStats, NovaStats, RuneStats, StormStats},
    player::{Health, Player},
    run::{GameRng, Stats},
    world::cursor_world_position,
};

pub struct WeaponsPlugin;

impl Plugin for WeaponsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeaponTimers>()
            .init_resource::<AimOverride>()
            .add_systems(OnEnter(AppState::InGame), (reset_weapons, spawn_aura))
            .add_systems(
                Update,
                (
                    move_projectiles,
                    fade_bursts,
                    fade_flashes,
                    projectile_trails,
                    fade_trail_dots,
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
                    thunderstorm,
                    frost_nova,
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
/// Returning bolts turn around after this long and come back to the player.
const RETURN_AFTER: f32 = 0.55;
/// A returning bolt is caught (and disappears) this close to the player.
const RETURN_CATCH_DISTANCE: f32 = 22.0;
/// How far a seeking bolt looks for a target.
const SEEK_RANGE: f32 = 320.0;
/// Seconds a knocked back enemy is pushed.
const KNOCKBACK_SECS: f32 = 0.15;
/// How long a lightning strike stays visible.
const FLASH_SECS: f32 = 0.18;
/// Bolts leave a dot this often, and this long-lived; only the first few bolts do.
const TRAIL_INTERVAL: f32 = 0.035;
const TRAIL_SECS: f32 = 0.22;
const MAX_TRAILING_BOLTS: usize = 60;

#[derive(Resource, Default)]
struct WeaponTimers {
    missile_cooldown: f32,
    /// Seconds until each queued echo volley is fired (Spell Echo).
    pending_echoes: Vec<f32>,
    aura_tick: f32,
    rune_tick: f32,
    rune_angle: f32,
    storm_timer: f32,
    nova_timer: f32,
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
    /// Collision radius; Magnified Bolts make it larger.
    radius: f32,
    /// Seconds since the bolt was fired.
    age: f32,
    /// Returning Bolts: damage factor of the way back (`0.0` for bolts that do not return).
    returning: f32,
    /// Whether a returning bolt is already on its way back.
    returned: bool,
    /// Seeking Bolts: how fast the bolt turns towards enemies, in radians per second.
    seek_turn: f32,
    /// Enemies already hit, so a piercing bolt damages each enemy only once.
    hit: Vec<Entity>,
}

/// A fading dot left behind by a flying bolt.
#[derive(Component)]
struct TrailDot {
    lifetime: f32,
}

/// A short-lived rectangle, used for lightning strikes.
#[derive(Component)]
struct Flash {
    lifetime: f32,
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

/// Bolts are brighter than 1.0 so they glow, and a violet light pool travels with them.
const MISSILE_COLOR: Color = Color::srgb(2.0, 1.7, 2.5);

fn spawn_projectile(
    commands: &mut Commands,
    assets: &GameAssets,
    position: Vec2,
    projectile: Projectile,
) {
    // The sprite points to the right, so rotating by the velocity angle aims it.
    let rotation = Quat::from_rotation_z(projectile.velocity.to_angle());
    let scale = SPRITE_SCALE * projectile.radius / MISSILE_RADIUS;
    let mut sprite = Sprite::from_image(assets.missile.clone());
    sprite.color = MISSILE_COLOR;
    commands.spawn((
        projectile,
        sprite,
        // Stretched along the flight direction for a sense of speed.
        Transform::from_xyz(position.x, position.y, 4.0)
            .with_rotation(rotation)
            .with_scale(Vec3::new(scale * 1.25, scale * 0.85, 1.0)),
        DespawnOnExit(AppState::InGame),
        children![(
            assets.light(Color::srgba(0.66, 0.46, 1.0, 0.7), Vec2::splat(30.0)),
            Transform::from_xyz(0.0, 0.0, -0.1),
        )],
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

/// World position to aim at instead of the mouse cursor. Only the autoplay bot sets it.
#[derive(Resource, Default)]
pub(crate) struct AimOverride(pub Option<Vec2>);

/// Fires a fan of bolts towards the mouse cursor whenever the cooldown is ready, plus the
/// echoes of earlier volleys (Spell Echo).
fn fire_missiles(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    stats: Res<Stats>,
    mut rng: ResMut<GameRng>,
    mut timers: ResMut<WeaponTimers>,
    mut sounds: MessageWriter<PlaySound>,
    aim_override: Res<AimOverride>,
    player: Single<(&Transform, &Player, &Health)>,
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

    let (transform, player, health) = player.into_inner();
    let origin = transform.translation.truncate();

    // Aim at the cursor; keep the last move direction when the cursor is outside the window.
    let (camera, camera_transform) = *camera;
    let cursor_world = aim_override
        .0
        .or_else(|| cursor_world_position(&window, camera, camera_transform));
    let aim = cursor_world
        .map(|target| (target - origin).normalize_or(player.facing))
        .unwrap_or(player.facing);
    // Bloodrage: hurt wizards hit harder.
    let rage = stats.rage_multiplier(health.fraction());

    if main_ready {
        timers.missile_cooldown = stats.missile.cooldown;
        sounds.write(PlaySound(Sfx::Shoot));
        spawn_volley(&mut commands, &assets, &stats, &mut rng, origin, aim, rage);
        for echo in 1..=stats.missile.echo_count() {
            timers.pending_echoes.push(echo as f32 * ECHO_DELAY);
        }
    }
    for _ in 0..due_echoes {
        let factor = stats.missile.echo_damage_factor() * rage;
        spawn_volley(
            &mut commands,
            &assets,
            &stats,
            &mut rng,
            origin,
            aim,
            factor,
        );
    }
}

/// One bolt of the current stats, flying in `direction`.
fn new_projectile(stats: &Stats, direction: Vec2, damage_factor: f32) -> Projectile {
    let returning = stats.missile.return_damage_factor();
    Projectile {
        velocity: direction * stats.missile.speed,
        damage: stats.missile.damage * damage_factor,
        // Returning bolts pass through everything on the way out and back.
        pierce_left: if returning > 0.0 {
            99
        } else {
            stats.missile.pierce
        },
        lifetime: if returning > 0.0 {
            MISSILE_LIFETIME + RETURN_AFTER + 1.0
        } else {
            MISSILE_LIFETIME
        },
        can_fork: stats.missile.fork_count() > 0,
        chains_left: stats.missile.chain_count(),
        radius: MISSILE_RADIUS * stats.missile.size_factor(),
        age: 0.0,
        returning,
        returned: false,
        seek_turn: stats.missile.seek_turn_rate(),
        hit: Vec::new(),
    }
}

/// Spawns one volley, spread symmetrically around `aim`. With Surpassing Chance there
/// may be extra bolts behind and beside the wizard.
fn spawn_volley(
    commands: &mut Commands,
    assets: &GameAssets,
    stats: &Stats,
    rng: &mut GameRng,
    origin: Vec2,
    aim: Vec2,
    damage_factor: f32,
) {
    let count = stats.missile.count as usize;
    let middle = (count as f32 - 1.0) / 2.0;
    for i in 0..count {
        let direction = Vec2::from_angle((i as f32 - middle) * MISSILE_SPREAD).rotate(aim);
        let projectile = new_projectile(stats, direction, damage_factor);
        spawn_projectile(commands, assets, origin, projectile);
    }

    if rng.0.chance(stats.missile.backfire_chance()) {
        for &angle in stats.missile.backfire_angles() {
            let direction = Vec2::from_angle(angle).rotate(aim);
            let projectile = new_projectile(stats, direction, damage_factor);
            spawn_projectile(commands, assets, origin, projectile);
        }
    }
}

fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<EnemyGrid>,
    player: Single<&Transform, (With<Player>, Without<Projectile>)>,
    mut projectiles: Query<(Entity, &mut Transform, &mut Projectile)>,
) {
    let dt = time.delta_secs();
    let home = player.translation.truncate();
    for (entity, mut transform, mut projectile) in &mut projectiles {
        projectile.age += dt;
        projectile.lifetime -= dt;
        if projectile.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let position = transform.translation.truncate();

        // Returning Bolts: after a short flight the bolt turns around and comes home,
        // hitting everything again on the way back.
        if projectile.returning > 0.0 {
            if !projectile.returned && projectile.age >= RETURN_AFTER {
                projectile.returned = true;
                projectile.hit.clear();
                projectile.damage *= projectile.returning;
            }
            if projectile.returned {
                let to_home = home - position;
                if to_home.length() <= RETURN_CATCH_DISTANCE {
                    commands.entity(entity).try_despawn();
                    continue;
                }
                let speed = projectile.velocity.length().max(300.0);
                projectile.velocity = to_home.normalize_or_zero() * speed;
            }
        }

        // Seeking Bolts: steer towards the nearest enemy that has not been hit yet.
        if projectile.seek_turn > 0.0 && !projectile.returned {
            let mut best: Option<(f32, Vec2)> = None;
            grid.0
                .for_each_in_radius(position.x, position.y, SEEK_RANGE, |other, x, y, d2| {
                    if projectile.hit.contains(&other) {
                        return;
                    }
                    if best.is_none_or(|(best_d2, _)| d2 < best_d2) {
                        best = Some((d2, Vec2::new(x, y)));
                    }
                });
            if let Some((_, target)) = best {
                let speed = projectile.velocity.length();
                let current = projectile.velocity.normalize_or_zero();
                let wanted = (target - position).normalize_or_zero();
                let max_turn = projectile.seek_turn * dt;
                let angle = current.angle_to(wanted).clamp(-max_turn, max_turn);
                projectile.velocity = Vec2::from_angle(angle).rotate(current) * speed;
            }
        }

        let delta = projectile.velocity * dt;
        transform.translation.x += delta.x;
        transform.translation.y += delta.y;
        // Chaining, seeking and returning change a bolt's direction mid-flight.
        transform.rotation = Quat::from_rotation_z(projectile.velocity.to_angle());
    }
}

fn projectile_hits(
    mut commands: Commands,
    assets: Res<GameAssets>,
    stats: Res<Stats>,
    grid: Res<EnemyGrid>,
    mut rng: ResMut<GameRng>,
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
            projectile.radius + MAX_ENEMY_RADIUS,
            |enemy, _, _, d2| candidates.push((enemy, d2)),
        );
        candidates.sort_by(|a, b| a.1.total_cmp(&b.1));

        for &(enemy_entity, d2) in &candidates {
            let Ok(enemy) = enemies.get(enemy_entity) else {
                continue;
            };
            let reach = projectile.radius + enemy.radius;
            if d2 > reach * reach || projectile.hit.contains(&enemy_entity) {
                continue;
            }
            // Precision: some hits are critical.
            let crit = rng.0.chance(stats.missile.crit_chance());
            let amount = projectile.damage
                * if crit {
                    MissileStats::CRIT_MULTIPLIER
                } else {
                    1.0
                };
            damage.write(DamageEnemy {
                entity: enemy_entity,
                amount,
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

            // Frostbite, Concussive Force and Shock: status effects on the enemy.
            let slow = stats.missile.slow_factor();
            if slow < 1.0 {
                commands.entity(enemy_entity).try_insert(Slowed {
                    factor: slow,
                    remaining: MissileStats::SLOW_SECS,
                });
            }
            let push = stats.missile.knockback_distance();
            if push > 0.0 {
                let direction = projectile.velocity.normalize_or_zero();
                commands.entity(enemy_entity).try_insert(Knockback {
                    velocity: direction * push / KNOCKBACK_SECS,
                    remaining: KNOCKBACK_SECS,
                });
            }
            let vulnerability = stats.missile.vulnerability();
            if vulnerability > 0.0 {
                commands.entity(enemy_entity).try_insert(Vulnerable {
                    bonus: vulnerability,
                    remaining: MissileStats::SHOCK_SECS,
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
                damage: parent.damage * stats.missile.fork_damage_factor(),
                pierce_left: 0,
                lifetime: FORK_LIFETIME,
                can_fork: false,
                chains_left: stats.missile.fork_chains(),
                radius: parent.radius,
                age: 0.0,
                returning: 0.0,
                returned: false,
                seek_turn: parent.seek_turn,
                // The enemy that caused the fork must not be hit again.
                hit: parent.hit.clone(),
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Thunderstorm and Frost Nova
// ---------------------------------------------------------------------------

/// Lightning strikes a few random enemies around the player.
fn thunderstorm(
    mut commands: Commands,
    time: Res<Time>,
    stats: Res<Stats>,
    grid: Res<EnemyGrid>,
    enemies: Query<&Enemy>,
    mut rng: ResMut<GameRng>,
    mut timers: ResMut<WeaponTimers>,
    mut sounds: MessageWriter<PlaySound>,
    mut damage: MessageWriter<DamageEnemy>,
    player: Single<&Transform, With<Player>>,
) {
    if stats.storm.level == 0 {
        return;
    }
    timers.storm_timer -= time.delta_secs();
    if timers.storm_timer > 0.0 {
        return;
    }
    let center = player.translation.truncate();
    let mut targets: Vec<Vec2> = Vec::new();
    grid.0
        .for_each_in_radius(center.x, center.y, StormStats::RANGE, |_, x, y, _| {
            targets.push(Vec2::new(x, y));
        });
    if targets.is_empty() {
        // Wait for enemies instead of wasting the strike.
        timers.storm_timer = 0.3;
        return;
    }
    timers.storm_timer = stats.storm.interval();
    sounds.write(PlaySound(Sfx::Blink));

    let strikes = (stats.storm.strikes() as usize).min(targets.len());
    for _ in 0..strikes {
        let target = targets.swap_remove(rng.0.below(targets.len()));
        grid.0.for_each_in_radius(
            target.x,
            target.y,
            StormStats::RADIUS + MAX_ENEMY_RADIUS,
            |entity, _, _, d2| {
                if let Ok(enemy) = enemies.get(entity) {
                    let reach = StormStats::RADIUS + enemy.radius;
                    if d2 <= reach * reach {
                        damage.write(DamageEnemy {
                            entity,
                            amount: stats.storm.damage(),
                        });
                    }
                }
            },
        );
        // A bolt from the sky and a flash where it lands.
        for (size, offset, color) in [
            (
                Vec2::new(8.0, 460.0),
                Vec2::new(0.0, 230.0),
                Color::srgba(3.0, 2.8, 1.6, 0.95),
            ),
            (
                Vec2::splat(StormStats::RADIUS * 1.6),
                Vec2::ZERO,
                Color::srgba(2.6, 2.1, 1.0, 0.5),
            ),
        ] {
            commands.spawn((
                Flash {
                    lifetime: FLASH_SECS,
                },
                Sprite::from_color(color, size),
                Transform::from_xyz(target.x + offset.x, target.y + offset.y, 5.0),
                DespawnOnExit(AppState::InGame),
            ));
        }
    }
}

/// Leaves a short trail of fading dots behind flying bolts.
fn projectile_trails(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    mut cooldown: Local<f32>,
    projectiles: Query<(&Transform, &Projectile)>,
) {
    *cooldown -= time.delta_secs();
    if *cooldown > 0.0 {
        return;
    }
    *cooldown = TRAIL_INTERVAL;
    for (transform, projectile) in projectiles.iter().take(MAX_TRAILING_BOLTS) {
        let size = 2.6 * projectile.radius / MISSILE_RADIUS;
        commands.spawn((
            TrailDot {
                lifetime: TRAIL_SECS,
            },
            Mesh2d(assets.circle.clone()),
            MeshMaterial2d(assets.trail.clone()),
            Transform::from_xyz(transform.translation.x, transform.translation.y, 3.9)
                .with_scale(Vec3::splat(size)),
            DespawnOnExit(AppState::InGame),
        ));
    }
}

fn fade_trail_dots(
    mut commands: Commands,
    time: Res<Time>,
    mut dots: Query<(Entity, &mut Transform, &mut TrailDot)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut dot) in &mut dots {
        dot.lifetime -= dt;
        if dot.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        transform.scale *= 1.0 - 4.0 * dt;
    }
}

fn fade_flashes(mut commands: Commands, time: Res<Time>, mut flashes: Query<(Entity, &mut Flash)>) {
    let dt = time.delta_secs();
    for (entity, mut flash) in &mut flashes {
        flash.lifetime -= dt;
        if flash.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

/// An icy blast around the player: damage and a slow for everything inside.
fn frost_nova(
    mut commands: Commands,
    time: Res<Time>,
    stats: Res<Stats>,
    assets: Res<GameAssets>,
    grid: Res<EnemyGrid>,
    enemies: Query<&Enemy>,
    mut timers: ResMut<WeaponTimers>,
    mut sounds: MessageWriter<PlaySound>,
    mut damage: MessageWriter<DamageEnemy>,
    player: Single<&Transform, With<Player>>,
) {
    if stats.nova.level == 0 {
        return;
    }
    timers.nova_timer -= time.delta_secs();
    if timers.nova_timer > 0.0 {
        return;
    }
    timers.nova_timer = stats.nova.interval();
    sounds.write(PlaySound(Sfx::Dash));

    let center = player.translation.truncate();
    let radius = stats.nova.radius();
    grid.0.for_each_in_radius(
        center.x,
        center.y,
        radius + MAX_ENEMY_RADIUS,
        |entity, _, _, d2| {
            if let Ok(enemy) = enemies.get(entity) {
                let reach = radius + enemy.radius;
                if d2 <= reach * reach {
                    damage.write(DamageEnemy {
                        entity,
                        amount: stats.nova.damage(),
                    });
                    commands.entity(entity).try_insert(Slowed {
                        factor: NovaStats::SLOW_FACTOR,
                        remaining: NovaStats::SLOW_SECS,
                    });
                }
            }
        },
    );
    commands.spawn((
        BurstRing {
            lifetime: BURST_SECS,
            radius,
        },
        Mesh2d(assets.circle.clone()),
        MeshMaterial2d(assets.ice.clone()),
        Transform::from_xyz(center.x, center.y, 3.2).with_scale(Vec3::splat(radius * 0.4)),
        DespawnOnExit(AppState::InGame),
    ));
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
            {
                let mut sprite = Sprite::from_image(assets.rune.clone());
                sprite.color = Color::srgb(1.7, 1.5, 1.1);
                sprite
            },
            Transform::from_xyz(0.0, 0.0, 3.5).with_scale(Vec3::splat(SPRITE_SCALE)),
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
