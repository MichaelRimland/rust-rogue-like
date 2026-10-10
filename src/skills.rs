//! Movement skills: Dash (`Space`) and Blink (right mouse button).
//!
//! - Dash: a short, fast burst in the movement direction with brief invulnerability.
//! - Blink: instantly teleports towards the mouse cursor, limited to a maximum range.

use bevy::{prelude::*, window::PrimaryWindow};

use crate::{
    AppState, GameSet,
    assets::{GameAssets, SPRITE_SCALE},
    logic::{
        skills::{Cooldown, limit_distance},
        stats::DashStats,
    },
    player::{Player, input_direction},
    run::Stats,
    world::cursor_world_position,
};

pub struct SkillsPlugin;

impl Plugin for SkillsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MovementSkills>()
            .add_systems(OnEnter(AppState::InGame), reset_skills)
            .add_systems(
                Update,
                (tick_skills, trigger_skills, dash_trail)
                    .chain()
                    .before(crate::player::move_player)
                    .in_set(GameSet::Movement),
            )
            .add_systems(Update, fade_afterimages.in_set(GameSet::Movement));
    }
}

/// Invulnerability granted after a blink, in seconds.
const BLINK_INVULNERABLE_SECS: f32 = 0.3;
/// Distance between the afterimages left along a blink path.
const AFTERIMAGE_SPACING: f32 = 36.0;
const AFTERIMAGE_SECS: f32 = 0.25;
/// Tint of the translucent afterimages (arcane purple).
const AFTERIMAGE_COLOR: Color = Color::srgba(0.56, 0.42, 0.94, 0.6);

#[derive(Resource)]
pub struct MovementSkills {
    pub dash: Cooldown,
    pub blink: Cooldown,
    dash_remaining: f32,
    dash_direction: Vec2,
    invulnerable: f32,
}

impl Default for MovementSkills {
    fn default() -> Self {
        Self {
            dash: Cooldown::new(),
            blink: Cooldown::new(),
            dash_remaining: 0.0,
            dash_direction: Vec2::X,
            invulnerable: 0.0,
        }
    }
}

impl MovementSkills {
    /// Velocity while a dash is in progress; it replaces the player's normal movement.
    pub fn dash_velocity(&self) -> Option<Vec2> {
        (self.dash_remaining > 0.0).then(|| self.dash_direction * DashStats::SPEED)
    }

    /// Whether contact damage is currently ignored.
    pub fn is_invulnerable(&self) -> bool {
        self.invulnerable > 0.0
    }
}

/// A fading copy of the wizard left behind by dash and blink.
#[derive(Component)]
struct Afterimage {
    lifetime: f32,
}

fn reset_skills(mut skills: ResMut<MovementSkills>) {
    *skills = MovementSkills::default();
}

fn tick_skills(time: Res<Time>, mut skills: ResMut<MovementSkills>) {
    let dt = time.delta_secs();
    skills.dash.tick(dt);
    skills.blink.tick(dt);
    skills.dash_remaining = (skills.dash_remaining - dt).max(0.0);
    skills.invulnerable = (skills.invulnerable - dt).max(0.0);
}

fn trigger_skills(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    assets: Res<GameAssets>,
    stats: Res<Stats>,
    mut skills: ResMut<MovementSkills>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    player: Single<(&mut Transform, &Player)>,
) {
    let (mut transform, player) = player.into_inner();

    // Skills are unlocked through level-up upgrades.
    if stats.dash.level > 0
        && keys.just_pressed(KeyCode::Space)
        && skills.dash.trigger(stats.dash.cooldown())
    {
        let duration = stats.dash.duration();
        skills.dash_direction = input_direction(&keys).unwrap_or(player.facing);
        skills.dash_remaining = duration;
        skills.invulnerable = skills.invulnerable.max(duration);
    }

    if stats.blink.level > 0
        && mouse.just_pressed(MouseButton::Right)
        && skills.blink.trigger(stats.blink.cooldown())
    {
        let range = stats.blink.range();
        let origin = transform.translation.truncate();
        let (camera, camera_transform) = *camera;
        let aim = cursor_world_position(&window, camera, camera_transform)
            .unwrap_or(origin + player.facing * range);
        let target = Vec2::from(limit_distance(origin.into(), aim.into(), range));

        let steps = (origin.distance(target) / AFTERIMAGE_SPACING) as u32;
        for step in 0..=steps {
            let t = step as f32 / steps.max(1) as f32;
            spawn_afterimage(
                &mut commands,
                &assets,
                origin.lerp(target, t),
                player.facing.x < 0.0,
            );
        }
        transform.translation.x = target.x;
        transform.translation.y = target.y;
        skills.invulnerable = skills.invulnerable.max(BLINK_INVULNERABLE_SECS);
    }
}

/// Leaves an afterimage every frame while dashing.
fn dash_trail(
    mut commands: Commands,
    assets: Res<GameAssets>,
    skills: Res<MovementSkills>,
    player: Single<(&Transform, &Sprite), With<Player>>,
) {
    if skills.dash_velocity().is_some() {
        let (transform, sprite) = *player;
        spawn_afterimage(
            &mut commands,
            &assets,
            transform.translation.truncate(),
            sprite.flip_x,
        );
    }
}

fn spawn_afterimage(commands: &mut Commands, assets: &GameAssets, position: Vec2, flip: bool) {
    let mut sprite = Sprite::from_image(assets.wizard[0].clone());
    sprite.flip_x = flip;
    sprite.color = AFTERIMAGE_COLOR;
    commands.spawn((
        Afterimage {
            lifetime: AFTERIMAGE_SECS,
        },
        sprite,
        Transform::from_xyz(position.x, position.y, 2.9).with_scale(Vec3::splat(SPRITE_SCALE)),
        DespawnOnExit(AppState::InGame),
    ));
}

fn fade_afterimages(
    mut commands: Commands,
    time: Res<Time>,
    mut images: Query<(Entity, &mut Sprite, &mut Afterimage)>,
) {
    let dt = time.delta_secs();
    for (entity, mut sprite, mut image) in &mut images {
        image.lifetime -= dt;
        if image.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let t = image.lifetime / AFTERIMAGE_SECS;
        sprite.color = AFTERIMAGE_COLOR.with_alpha(AFTERIMAGE_COLOR.alpha() * t);
    }
}
