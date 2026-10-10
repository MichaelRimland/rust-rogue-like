//! Experience gems, level-ups and upgrade selection.

use bevy::prelude::*;

use crate::{
    AppState, GamePhase, GameSet,
    assets::{GameAssets, SPRITE_SCALE},
    audio::{PlaySound, Sfx},
    logic::stats::{Upgrade, roll_upgrades},
    player::{Health, PLAYER_RADIUS, Player},
    run::{GameRng, Stats, Xp},
};

pub struct ProgressionPlugin;

impl Plugin for ProgressionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ChooseUpgrade>()
            .init_resource::<PendingLevelUps>()
            .init_resource::<LevelUpChoices>()
            .add_systems(OnEnter(AppState::InGame), reset_progression)
            .add_systems(Update, pulse_gems.in_set(GameSet::Movement))
            .add_systems(
                Update,
                (collect_gems, start_level_up)
                    .chain()
                    .in_set(GameSet::Progression),
            )
            .add_systems(OnEnter(GamePhase::LevelUp), roll_choices)
            .add_systems(Update, choose_upgrade.run_if(in_state(GamePhase::LevelUp)));
    }
}

/// A level-up card was clicked with the mouse; carries the index of the card.
#[derive(Message, Debug, Clone, Copy)]
pub struct ChooseUpgrade(pub usize);

/// Number of upgrade cards offered per level-up.
const CHOICES_PER_LEVEL: usize = 3;
const GEM_ATTRACT_SPEED: f32 = 480.0;
/// World radius of the halo behind a gem.
const GEM_GLOW_RADIUS: f32 = 13.0;
/// How strongly (fraction of size) and how fast gems pulse.
const GEM_PULSE_AMOUNT: f32 = 0.12;
const GEM_PULSE_SPEED: f32 = 4.0;

#[derive(Component)]
pub struct XpGem {
    pub value: u32,
    /// Once a gem was pulled in by the pickup radius it keeps flying to the player.
    pub attracted: bool,
}

/// Level-ups that still wait for an upgrade choice (several can stack up at once).
#[derive(Resource, Default)]
pub struct PendingLevelUps(pub u32);

/// The upgrades currently offered. The level-up screen is rebuilt whenever this changes.
#[derive(Resource, Default)]
pub struct LevelUpChoices(pub Vec<Upgrade>);

pub fn spawn_gem(commands: &mut Commands, assets: &GameAssets, position: Vec2, value: u32) {
    commands.spawn((
        XpGem {
            value,
            attracted: false,
        },
        {
            let mut sprite = Sprite::from_image(assets.gem_image(value));
            sprite.color = Color::srgb(1.6, 1.6, 1.6);
            sprite
        },
        Transform::from_xyz(position.x, position.y, 1.0).with_scale(Vec3::splat(SPRITE_SCALE)),
        DespawnOnExit(AppState::InGame),
        // A soft halo behind the gem. The child inherits the parent's scale.
        children![(
            assets.light(
                assets.gem_glow(value),
                Vec2::splat(GEM_GLOW_RADIUS * 2.0 / SPRITE_SCALE)
            ),
            Transform::from_xyz(0.0, 0.0, -0.1),
        )],
    ));
}

/// Gems pulse gently so they catch the eye on the ground.
fn pulse_gems(time: Res<Time>, mut gems: Query<&mut Transform, With<XpGem>>) {
    let t = time.elapsed_secs() * GEM_PULSE_SPEED;
    for mut transform in &mut gems {
        // The position doubles as a phase offset so gems do not pulse in unison.
        let phase = transform.translation.x * 0.05;
        let scale = SPRITE_SCALE * (1.0 + GEM_PULSE_AMOUNT * (t + phase).sin());
        transform.scale = Vec3::new(scale, scale, 1.0);
    }
}

fn reset_progression(mut pending: ResMut<PendingLevelUps>, mut choices: ResMut<LevelUpChoices>) {
    pending.0 = 0;
    choices.0.clear();
}

fn collect_gems(
    mut commands: Commands,
    time: Res<Time>,
    stats: Res<Stats>,
    mut xp: ResMut<Xp>,
    mut pending: ResMut<PendingLevelUps>,
    mut sounds: MessageWriter<PlaySound>,
    player: Single<&Transform, (With<Player>, Without<XpGem>)>,
    mut gems: Query<(Entity, &mut Transform, &mut XpGem)>,
) {
    let target = player.translation.truncate();
    let step = GEM_ATTRACT_SPEED * time.delta_secs();
    let pickup_radius_sq = stats.pickup_radius * stats.pickup_radius;
    let collect_radius = PLAYER_RADIUS + 4.0;

    for (entity, mut transform, mut gem) in &mut gems {
        let position = transform.translation.truncate();
        let offset = target - position;
        let distance_sq = offset.length_squared();

        if distance_sq <= collect_radius * collect_radius {
            pending.0 += xp.add(gem.value);
            sounds.write(PlaySound(Sfx::Pickup));
            commands.entity(entity).try_despawn();
            continue;
        }
        if distance_sq <= pickup_radius_sq {
            gem.attracted = true;
        }
        if gem.attracted {
            let distance = distance_sq.sqrt();
            let movement = offset / distance * step.min(distance);
            transform.translation.x += movement.x;
            transform.translation.y += movement.y;
        }
    }
}

fn start_level_up(pending: Res<PendingLevelUps>, mut next: ResMut<NextState<GamePhase>>) {
    if pending.0 > 0 {
        next.set(GamePhase::LevelUp);
    }
}

fn roll_choices(
    mut rng: ResMut<GameRng>,
    stats: Res<Stats>,
    mut choices: ResMut<LevelUpChoices>,
    mut pending: ResMut<PendingLevelUps>,
    mut next: ResMut<NextState<GamePhase>>,
    mut health: Single<&mut Health, With<Player>>,
) {
    choices.0 = roll_upgrades(&mut rng.0, &stats, CHOICES_PER_LEVEL);
    if choices.0.is_empty() {
        // Everything is maxed out: reward the level with a full heal instead.
        pending.0 = 0;
        health.current = health.max;
        next.set(GamePhase::Running);
    }
}

fn choose_upgrade(
    keys: Res<ButtonInput<KeyCode>>,
    mut clicks: MessageReader<ChooseUpgrade>,
    mut rng: ResMut<GameRng>,
    mut stats: ResMut<Stats>,
    mut choices: ResMut<LevelUpChoices>,
    mut pending: ResMut<PendingLevelUps>,
    mut next: ResMut<NextState<GamePhase>>,
    mut health: Single<&mut Health, With<Player>>,
) {
    const KEYS: [[KeyCode; 2]; CHOICES_PER_LEVEL] = [
        [KeyCode::Digit1, KeyCode::Numpad1],
        [KeyCode::Digit2, KeyCode::Numpad2],
        [KeyCode::Digit3, KeyCode::Numpad3],
    ];
    // Either a click on a card or the number keys picks the upgrade.
    let clicked = clicks.read().last().map(|click| click.0);
    let Some(index) = clicked.or_else(|| KEYS.iter().position(|pair| keys.any_just_pressed(*pair)))
    else {
        return;
    };
    let Some(&upgrade) = choices.0.get(index) else {
        return;
    };

    upgrade.apply(&mut stats);
    health.max = stats.max_health;
    if upgrade == Upgrade::MaxHealth {
        health.current = (health.current + Upgrade::MAX_HEALTH_HEAL).min(health.max);
    }

    pending.0 = pending.0.saturating_sub(1);
    if pending.0 > 0 {
        // More level-ups are queued: offer a fresh set right away.
        choices.0 = roll_upgrades(&mut rng.0, &stats, CHOICES_PER_LEVEL);
        if !choices.0.is_empty() {
            return;
        }
        pending.0 = 0;
    }
    next.set(GamePhase::Running);
}
