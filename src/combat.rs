//! Damage resolution, enemy deaths, contact damage and death particles.
//!
//! Weapons never touch enemy health directly. They write [`DamageEnemy`] messages,
//! and a single system applies them. That keeps kill handling (loot, particles,
//! statistics) in one place no matter how many weapons exist.

use bevy::prelude::*;

use crate::{
    AppState, GameSet,
    assets::GameAssets,
    enemy::{Enemy, EnemyGrid, HIT_FLASH_SECS, MAX_ENEMY_RADIUS},
    player::{Health, PLAYER_RADIUS, Player, flash_hurt},
    progression::spawn_gem,
    run::{GameRng, RunStats},
    skills::MovementSkills,
    world::CameraShake,
};

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DamageEnemy>()
            .add_systems(Update, contact_damage.in_set(GameSet::Combat))
            .add_systems(Update, apply_damage.in_set(GameSet::Damage))
            .add_systems(
                Update,
                (update_particles, update_damage_numbers).in_set(GameSet::Movement),
            );
    }
}

/// Request to damage an enemy. Written by weapons, applied in [`GameSet::Damage`].
#[derive(Message, Debug, Clone, Copy)]
pub struct DamageEnemy {
    pub entity: Entity,
    pub amount: f32,
}

#[derive(Component)]
struct Particle {
    velocity: Vec2,
    lifetime: f32,
    max_lifetime: f32,
    size: f32,
}

const DEATH_PARTICLES: usize = 6;

/// Floating number that rises from a hit enemy and fades out.
#[derive(Component)]
struct DamageNumber {
    lifetime: f32,
}

const NUMBER_SECS: f32 = 0.6;
const NUMBER_RISE_SPEED: f32 = 45.0;
/// Hits of at least this much damage are shown in gold.
const BIG_HIT: f32 = 30.0;
/// Never show more numbers than this at once; text is comparatively expensive.
const MAX_DAMAGE_NUMBERS: usize = 60;

fn apply_damage(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut run: ResMut<RunStats>,
    mut messages: MessageReader<DamageEnemy>,
    mut enemies: Query<(&Transform, &mut Enemy)>,
    numbers: Query<(), With<DamageNumber>>,
) {
    let mut visible_numbers = numbers.iter().count();
    for message in messages.read() {
        let Ok((transform, mut enemy)) = enemies.get_mut(message.entity) else {
            // Already despawned, e.g. killed earlier this frame by another weapon.
            continue;
        };
        if enemy.health <= 0.0 {
            continue;
        }

        enemy.health -= message.amount;
        enemy.hit_flash = HIT_FLASH_SECS;

        if visible_numbers < MAX_DAMAGE_NUMBERS && message.amount >= 1.0 {
            visible_numbers += 1;
            let jitter = rng.0.range_f32(-8.0, 8.0);
            let color = if message.amount >= BIG_HIT {
                Color::srgb(1.0, 0.89, 0.48)
            } else {
                Color::srgb(0.95, 0.93, 0.87)
            };
            commands.spawn((
                DamageNumber {
                    lifetime: NUMBER_SECS,
                },
                Text2d::new(format!("{:.0}", message.amount)),
                TextFont {
                    font_size: FontSize::Px(if message.amount >= BIG_HIT {
                        18.0
                    } else {
                        14.0
                    }),
                    ..default()
                },
                TextColor(color),
                Transform::from_xyz(
                    transform.translation.x + jitter,
                    transform.translation.y + enemy.radius + 6.0,
                    6.0,
                ),
                DespawnOnExit(AppState::InGame),
            ));
        }
        if enemy.health > 0.0 {
            continue;
        }

        let position = transform.translation.truncate();
        run.kills += 1;
        run.gold += enemy.kind.gold();
        spawn_gem(&mut commands, &assets, position, enemy.xp);
        let material = assets.enemy_material(enemy.kind);
        for _ in 0..DEATH_PARTICLES {
            let direction = Vec2::from_angle(rng.0.range_f32(0.0, std::f32::consts::TAU));
            let lifetime = rng.0.range_f32(0.25, 0.45);
            let size = enemy.radius * rng.0.range_f32(0.2, 0.4);
            commands.spawn((
                Particle {
                    velocity: direction * rng.0.range_f32(60.0, 160.0),
                    lifetime,
                    max_lifetime: lifetime,
                    size,
                },
                Mesh2d(assets.circle.clone()),
                MeshMaterial2d(material.clone()),
                Transform::from_xyz(position.x, position.y, 2.5).with_scale(Vec3::splat(size)),
                DespawnOnExit(AppState::InGame),
            ));
        }
        commands.entity(message.entity).try_despawn();
    }
}

/// Enemies touching the player deal damage over time.
fn contact_damage(
    time: Res<Time>,
    grid: Res<EnemyGrid>,
    enemies: Query<&Enemy>,
    skills: Res<MovementSkills>,
    mut shake: ResMut<CameraShake>,
    player: Single<(&Transform, &mut Health, &mut Player)>,
) {
    if skills.is_invulnerable() {
        return;
    }
    let (transform, mut health, mut player) = player.into_inner();
    let position = transform.translation.truncate();

    let mut dps = 0.0;
    grid.0.for_each_in_radius(
        position.x,
        position.y,
        PLAYER_RADIUS + MAX_ENEMY_RADIUS,
        |entity, _, _, d2| {
            if let Ok(enemy) = enemies.get(entity) {
                let reach = PLAYER_RADIUS + enemy.radius;
                if d2 <= reach * reach {
                    dps += enemy.contact_dps;
                }
            }
        },
    );

    if dps > 0.0 {
        health.current -= dps * time.delta_secs();
        flash_hurt(&mut player);
        shake.add_trauma(2.4 * time.delta_secs());
    }
}

fn update_damage_numbers(
    mut commands: Commands,
    time: Res<Time>,
    mut numbers: Query<(Entity, &mut Transform, &mut TextColor, &mut DamageNumber)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut color, mut number) in &mut numbers {
        number.lifetime -= dt;
        if number.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        transform.translation.y += NUMBER_RISE_SPEED * dt;
        color.0 = color.0.with_alpha((number.lifetime / NUMBER_SECS).min(1.0));
    }
}

fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut particles: Query<(Entity, &mut Transform, &mut Particle)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut particle) in &mut particles {
        particle.lifetime -= dt;
        if particle.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let delta = particle.velocity * dt;
        transform.translation.x += delta.x;
        transform.translation.y += delta.y;
        particle.velocity *= 1.0 - 4.0 * dt.min(0.25);
        let t = particle.lifetime / particle.max_lifetime;
        transform.scale = Vec3::splat(particle.size * t);
    }
}
