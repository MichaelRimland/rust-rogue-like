//! Projectiles shot by ranged enemies. They are slow enough to be dodged, and a dash
//! lets the player slip through them.

use bevy::prelude::*;

use crate::{
    AppState, GameSet,
    assets::{GameAssets, SPRITE_SCALE},
    dungeon::RoomEntity,
    enemy::Enemy,
    player::{Health, PLAYER_RADIUS, Player, flash_hurt},
    skills::MovementSkills,
    world::CameraShake,
};

pub struct EnemyFirePlugin;

impl Plugin for EnemyFirePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, move_bolts.in_set(GameSet::Movement))
            .add_systems(
                Update,
                (enemy_shooting, bolt_hits).chain().in_set(GameSet::Combat),
            );
    }
}

const BOLT_RADIUS: f32 = 7.0;
const BOLT_LIFETIME: f32 = 4.0;
/// Archers also shoot a little beyond the distance they like to hold.
const SHOOT_RANGE_SLACK: f32 = 60.0;
/// The archer lights up this long before it fires, so the shot can be anticipated.
const TELEGRAPH_SECS: f32 = 0.45;
const TELEGRAPH_COLOR: Color = Color::srgb(2.4, 2.0, 1.6);

#[derive(Component)]
struct EnemyBolt {
    velocity: Vec2,
    damage: f32,
    lifetime: f32,
}

/// Ranged enemies aim at where the player is *now*, so moving sideways dodges the bolt.
fn enemy_shooting(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    player: Single<&Transform, With<Player>>,
    mut enemies: Query<(&Transform, &mut Enemy, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    let target = player.translation.truncate();
    for (transform, mut enemy, mut sprite) in &mut enemies {
        let Some(ranged) = enemy.ranged else {
            continue;
        };
        enemy.shoot_timer -= dt;
        let telegraphing = enemy.shoot_timer > 0.0 && enemy.shoot_timer < TELEGRAPH_SECS;
        let color = if telegraphing {
            TELEGRAPH_COLOR
        } else {
            Color::WHITE
        };
        if sprite.color != color {
            sprite.color = color;
        }
        if enemy.shoot_timer > 0.0 {
            continue;
        }
        let origin = transform.translation.truncate();
        let offset = target - origin;
        if offset.length() > ranged.max_range + SHOOT_RANGE_SLACK {
            // Not in range yet; stay ready.
            enemy.shoot_timer = 0.0;
            continue;
        }
        enemy.shoot_timer = ranged.cooldown;

        let velocity = offset.normalize_or_zero() * ranged.projectile_speed;
        spawn_bolt(&mut commands, &assets, origin, velocity, ranged.damage);
    }
}

/// Spawns one enemy bolt. Bolts belong to the current room, so leaving a room clears them.
pub(crate) fn spawn_bolt(
    commands: &mut Commands,
    assets: &GameAssets,
    origin: Vec2,
    velocity: Vec2,
    damage: f32,
) {
    commands.spawn((
        EnemyBolt {
            velocity,
            damage,
            lifetime: BOLT_LIFETIME,
        },
        RoomEntity,
        Sprite::from_image(assets.enemy_bolt.clone()),
        Transform::from_xyz(origin.x, origin.y, 3.8)
            .with_rotation(Quat::from_rotation_z(velocity.to_angle()))
            .with_scale(Vec3::splat(SPRITE_SCALE)),
        DespawnOnExit(AppState::InGame),
    ));
}

fn move_bolts(
    mut commands: Commands,
    time: Res<Time>,
    mut bolts: Query<(Entity, &mut Transform, &mut EnemyBolt)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut bolt) in &mut bolts {
        bolt.lifetime -= dt;
        if bolt.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let delta = bolt.velocity * dt;
        transform.translation.x += delta.x;
        transform.translation.y += delta.y;
    }
}

fn bolt_hits(
    mut commands: Commands,
    skills: Res<MovementSkills>,
    mut shake: ResMut<CameraShake>,
    bolts: Query<(Entity, &Transform, &EnemyBolt)>,
    player: Single<(&Transform, &mut Health, &mut Player), Without<EnemyBolt>>,
) {
    // While invulnerable (dash, blink) bolts fly straight through.
    if skills.is_invulnerable() {
        return;
    }
    let (player_transform, mut health, mut player) = player.into_inner();
    let center = player_transform.translation.truncate();
    let reach = PLAYER_RADIUS + BOLT_RADIUS;
    for (entity, transform, bolt) in &bolts {
        if transform.translation.truncate().distance_squared(center) <= reach * reach {
            health.current -= bolt.damage;
            flash_hurt(&mut player);
            shake.add_trauma(0.35);
            commands.entity(entity).try_despawn();
        }
    }
}
