//! The wizard: spawning, movement, health.

use bevy::prelude::*;

use crate::{AppState, GameSet, assets::GameAssets, run::Stats, skills::MovementSkills};

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

pub const ROBE_COLOR: Color = Color::srgb(0.30, 0.35, 0.85);
const HURT_COLOR: Color = Color::srgb(1.0, 0.35, 0.35);

fn spawn_player(
    mut commands: Commands,
    assets: Res<GameAssets>,
    stats: Res<Stats>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    // The previous run may have ended while the robe was tinted red.
    if let Some(mut robe) = materials.get_mut(&assets.player_robe) {
        robe.color = ROBE_COLOR;
    }

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
        Mesh2d(assets.circle.clone()),
        MeshMaterial2d(assets.player_robe.clone()),
        Transform::from_xyz(0.0, 0.0, 3.0).with_scale(Vec3::splat(PLAYER_RADIUS)),
        DespawnOnExit(AppState::InGame),
        // The pointy wizard hat. Child transforms are relative to the (scaled) parent,
        // so these values are in "player radii".
        children![(
            Mesh2d(assets.triangle.clone()),
            MeshMaterial2d(assets.player_hat.clone()),
            Transform::from_xyz(0.0, 0.55, 0.1).with_scale(Vec3::splat(1.3)),
        )],
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
    player: Single<(&mut Transform, &mut Player)>,
) {
    let (mut transform, mut player) = player.into_inner();

    // A dash replaces the normal movement until it ends.
    let (velocity, facing) = match skills.dash_velocity() {
        Some(velocity) => (velocity, velocity.normalize_or_zero()),
        None => match input_direction(&keys) {
            Some(direction) => (direction * stats.move_speed, direction),
            None => return,
        },
    };
    player.facing = facing;
    let delta = velocity * time.delta_secs();
    transform.translation.x += delta.x;
    transform.translation.y += delta.y;
}

fn regenerate(time: Res<Time>, stats: Res<Stats>, mut health: Single<&mut Health, With<Player>>) {
    health.max = stats.max_health;
    health.current = (health.current + stats.regen_per_sec * time.delta_secs()).min(health.max);
}

fn update_hurt_tint(
    time: Res<Time>,
    assets: Res<GameAssets>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut player: Single<&mut Player>,
) {
    player.hurt_timer = (player.hurt_timer - time.delta_secs()).max(0.0);
    let is_hurt = player.hurt_timer > 0.0;
    if is_hurt == player.tinted {
        return;
    }
    player.tinted = is_hurt;
    // The robe material is used by the player only, so tinting it is safe.
    if let Some(mut material) = materials.get_mut(&assets.player_robe) {
        material.color = if is_hurt { HURT_COLOR } else { ROBE_COLOR };
    }
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
