//! The wizard: spawning, movement, health.

use bevy::prelude::*;

use crate::{
    AppState, GameSet,
    animation::WalkAnimation,
    assets::{GameAssets, SPRITE_SCALE},
    run::Stats,
    skills::MovementSkills,
};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::InGame),
            spawn_player.after(crate::run::start_run),
        )
        .add_systems(Update, move_player.in_set(GameSet::Movement))
        .add_systems(
            Update,
            (regenerate, update_hurt_tint, check_death)
                .chain()
                .in_set(GameSet::Progression),
        );
    }
}

pub const PLAYER_RADIUS: f32 = 14.0;
const HURT_TINT_SECS: f32 = 0.15;

#[derive(Component)]
pub struct Player {
    /// Last non-zero movement direction, used e.g. as the fallback aim direction.
    pub facing: Vec2,
    /// Remaining seconds of the red "got hit" tint.
    pub hurt_timer: f32,
    /// Whether the robe currently shows the hurt tint.
    tinted: bool,
}

#[derive(Component, Debug)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn fraction(&self) -> f32 {
        (self.current / self.max).clamp(0.0, 1.0)
    }
}

const HURT_COLOR: Color = Color::srgb(1.0, 0.35, 0.35);

fn spawn_player(mut commands: Commands, assets: Res<GameAssets>, stats: Res<Stats>) {
    let mut walk = WalkAnimation::new(assets.wizard.clone(), 0);
    walk.playing = false;
    commands.spawn((
        Player {
            facing: Vec2::X,
            hurt_timer: 0.0,
            tinted: false,
        },
        Health {
            current: stats.max_health,
            max: stats.max_health,
        },
        Sprite::from_image(assets.wizard[0].clone()),
        walk,
        Transform::from_xyz(0.0, 0.0, 3.0).with_scale(Vec3::splat(SPRITE_SCALE)),
        DespawnOnExit(AppState::InGame),
        children![
            // A faint warm light on the ground around the wizard, and a shadow.
            (
                assets.light(Color::srgba(1.0, 0.82, 0.45, 0.42), Vec2::new(150.0, 120.0)),
                Transform::from_xyz(0.0, -4.0, -0.1),
            ),
            (
                assets.light(Color::srgba(0.0, 0.0, 0.02, 0.62), Vec2::new(24.0, 8.0)),
                Transform::from_xyz(0.0, -13.0, -0.05),
            ),
        ],
    ));
}

/// Normalized WASD / arrow key direction, `None` when no movement key is held.
pub(crate) fn input_direction(keys: &ButtonInput<KeyCode>) -> Option<Vec2> {
    let mut direction = Vec2::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        direction.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        direction.y -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        direction.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        direction.x += 1.0;
    }

    direction.try_normalize()
}

pub(crate) fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    stats: Res<Stats>,
    skills: Res<MovementSkills>,
    player: Single<(&mut Transform, &mut Player, &mut Sprite, &mut WalkAnimation)>,
) {
    let (mut transform, mut player, mut sprite, mut walk) = player.into_inner();

    // A dash replaces the normal movement until it ends.
    let movement = match skills.dash_velocity() {
        Some(velocity) => Some((velocity, velocity.normalize_or_zero())),
        None => input_direction(&keys).map(|direction| (direction * stats.move_speed, direction)),
    };
    walk.playing = movement.is_some();
    let Some((velocity, facing)) = movement else {
        return;
    };

    player.facing = facing;
    // The sprite looks to the right by default.
    if facing.x.abs() > 0.1 {
        sprite.flip_x = facing.x < 0.0;
    }
    let delta = velocity * time.delta_secs();
    transform.translation.x += delta.x;
    transform.translation.y += delta.y;
}

fn regenerate(time: Res<Time>, stats: Res<Stats>, mut health: Single<&mut Health, With<Player>>) {
    health.max = stats.max_health;
    health.current = (health.current + stats.regen_per_sec * time.delta_secs()).min(health.max);
}

fn update_hurt_tint(time: Res<Time>, player: Single<(&mut Player, &mut Sprite)>) {
    let (mut player, mut sprite) = player.into_inner();
    player.hurt_timer = (player.hurt_timer - time.delta_secs()).max(0.0);
    let is_hurt = player.hurt_timer > 0.0;
    if is_hurt == player.tinted {
        return;
    }
    player.tinted = is_hurt;
    sprite.color = if is_hurt { HURT_COLOR } else { Color::WHITE };
}

/// Marks the player as hurt (called by the combat code).
pub fn flash_hurt(player: &mut Player) {
    player.hurt_timer = HURT_TINT_SECS;
}

fn check_death(health: Single<&Health, With<Player>>, mut next: ResMut<NextState<AppState>>) {
    if health.current <= 0.0 {
        next.set(AppState::GameOver);
    }
}
