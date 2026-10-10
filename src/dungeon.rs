//! Dungeon mode: floors made of rounds in a large arena, with doors between them, shops,
//! treasure, resting places and a boss at the end of every floor. The last floor ends
//! with the end boss.
//!
//! The structure (which rooms exist, how long a round is, which bosses) lives in
//! [`crate::logic::dungeon`]. This module spawns the rooms and runs the room flow.

use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::{
    AppState, GameSet,
    assets::{GameAssets, SPRITE_SCALE},
    audio::{PlaySound, Sfx},
    combat::DamageEnemy,
    enemy::{Enemy, EnemySpawn, spawn_enemy, spawn_enemy_with},
    enemy_fire::spawn_bolt,
    logic::{
        dungeon::{
            BossKind, BossPattern, FLOORS, FloorPlan, RoomKind, boss_health, boss_pattern,
            boss_phase, clear_gold, difficulty_time, enemy_health_multiplier, pick_boss,
            round_alive_cap, round_batch_size, round_quota, round_spawn_interval,
        },
        enemies::pick_enemy_kind,
        stats::{Upgrade, roll_upgrades},
    },
    player::{Health, Player},
    progression::{PendingLevelUps, XpGem},
    run::{GameRng, RunStats, Stats},
    ui::{ACCENT_COLOR, MUTED_TEXT_COLOR, TEXT_COLOR, label},
};

pub struct DungeonPlugin;

impl Plugin for DungeonPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameMode>()
            .init_resource::<Dungeon>()
            .add_systems(
                OnEnter(AppState::InGame),
                (reset_dungeon, spawn_dungeon_hud).run_if(dungeon_mode),
            )
            .add_systems(
                Update,
                (
                    dungeon_flow,
                    round_director,
                    use_doors,
                    interact_props,
                    boss_logic,
                )
                    .chain()
                    .in_set(GameSet::Progression)
                    .run_if(dungeon_mode),
            )
            .add_systems(
                Update,
                clamp_to_arena
                    .after(crate::player::move_player)
                    .in_set(GameSet::Movement)
                    .run_if(dungeon_mode),
            )
            .add_systems(
                Update,
                (update_dungeon_hud, update_boss_bar, fade_banners)
                    .run_if(in_state(AppState::InGame))
                    .run_if(dungeon_mode),
            );
    }
}

/// Half extents of the playable arena in world units. Several screens wide, so there is
/// room to kite; the camera follows the player and stops at the walls.
pub const ARENA_HALF: Vec2 = Vec2::new(1100.0, 700.0);
/// Distance the player keeps from the walls.
const WALL_MARGIN: f32 = 20.0;
const DOOR_REACH: f32 = 46.0;
const PROP_REACH: f32 = 44.0;
/// Where the player starts a room, and where doors, chests and shop items appear.
const PLAYER_START: Vec2 = Vec2::new(0.0, -150.0);
const DOOR_Y: f32 = 260.0;
/// Enemies of a round enter the arena in a ring this far from the player, just off screen.
const SPAWN_RING: (f32, f32) = (620.0, 780.0);
const HEAL_PRICE: u32 = 30;
const UPGRADE_PRICE: u32 = 70;
const REST_HEAL_FRACTION: f32 = 0.4;
/// Boss bolts.
const RING_BOLT_SPEED: f32 = 190.0;
const AIMED_BOLT_SPEED: f32 = 250.0;
const SPIRAL_BOLT_SPEED: f32 = 170.0;
const SPIRAL_INTERVAL: f32 = 0.22;
/// Charge attack: telegraph, dash and pause between charges (shorter in later phases).
const CHARGE_WINDUP: f32 = 0.7;
const CHARGE_DASH: f32 = 0.45;
const CHARGE_SPEED: f32 = 650.0;
const CHARGE_COOLDOWN: f32 = 4.0;
const TELEGRAPH_COLOR: Color = Color::srgb(2.4, 2.0, 1.6);

/// Which mode the next run is played in. Chosen in the main menu.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameMode {
    /// Survive as long as possible in an endless forest.
    Endless,
    /// Fight through rounds and floors, ending with the end boss.
    #[default]
    Dungeon,
}

pub fn dungeon_mode(mode: Res<GameMode>) -> bool {
    *mode == GameMode::Dungeon
}

pub fn endless_mode(mode: Res<GameMode>) -> bool {
    *mode == GameMode::Endless
}

/// Developer override: start the dungeon at a given floor, optionally in its boss room.
/// Used by the autoplay tool to look at the late game without playing it.
#[derive(Resource, Debug, Clone, Copy)]
pub struct DebugStart {
    pub floor: u32,
    pub boss_room: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomState {
    /// Enemies to defeat or a chest to open.
    Busy,
    /// The doors are open.
    Cleared,
}

#[derive(Resource)]
pub struct Dungeon {
    pub floor: u32,
    pub layer: usize,
    plan: FloorPlan,
    pub kind: RoomKind,
    pub state: RoomState,
    /// Enemies to defeat in this round, and the kill count the round started at.
    quota: u32,
    kills_at_start: u32,
    spawn_timer: f32,
    boss_spawned: bool,
    initialized: bool,
}

impl Default for Dungeon {
    fn default() -> Self {
        Self {
            floor: 1,
            layer: 0,
            plan: FloorPlan::default(),
            kind: RoomKind::Arena,
            state: RoomState::Busy,
            quota: 0,
            kills_at_start: 0,
            spawn_timer: 0.0,
            boss_spawned: false,
            initialized: false,
        }
    }
}

impl Dungeon {
    pub fn is_cleared(&self) -> bool {
        self.state == RoomState::Cleared
    }
}

/// Everything that belongs to the current room and disappears when leaving it.
#[derive(Component)]
pub(crate) struct RoomEntity;

#[derive(Debug, Clone, Copy)]
enum DoorTarget {
    Room(RoomKind),
    NextFloor,
}

#[derive(Component)]
pub(crate) struct Door {
    target: DoorTarget,
}

#[derive(Component)]
struct Chest;

#[derive(Debug, Clone, Copy)]
enum ShopOffer {
    Heal,
    Upgrade(Upgrade),
}

#[derive(Component)]
struct ShopItem {
    offer: ShopOffer,
    price: u32,
    /// Seconds until the "not enough gold" message may be shown again.
    denied: f32,
}

#[derive(Debug, Clone, Copy)]
enum Charge {
    Idle,
    WindUp { left: f32, direction: Vec2 },
    Dash { left: f32, direction: Vec2 },
}

#[derive(Component)]
struct Boss {
    pattern: BossPattern,
    max_health: f32,
    tint: Color,
    bolt_timer: f32,
    spiral_timer: f32,
    summon_timer: f32,
    charge_timer: f32,
    charge: Charge,
    spin: f32,
    spiral_spin: f32,
}

/// Short floating message, e.g. "Room cleared".
#[derive(Component)]
struct Banner {
    lifetime: f32,
}

#[derive(Component)]
struct DungeonHudText;

#[derive(Component)]
struct BossBarRoot;

#[derive(Component)]
struct BossBarFill;

const BANNER_SECS: f32 = 1.8;

fn reset_dungeon(mut dungeon: ResMut<Dungeon>) {
    *dungeon = Dungeon::default();
}

// ---------------------------------------------------------------------------
// Room flow
// ---------------------------------------------------------------------------

/// Enters the first room of a fresh run.
fn dungeon_flow(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    stats: Res<Stats>,
    mut dungeon: ResMut<Dungeon>,
    mut run: ResMut<RunStats>,
    rooms: Query<Entity, With<RoomEntity>>,
    debug_start: Option<Res<DebugStart>>,
    player: Single<(&mut Transform, &mut Health), With<Player>>,
) {
    if dungeon.initialized {
        return;
    }
    dungeon.initialized = true;
    dungeon.plan = FloorPlan::generate(&mut rng.0);
    if let Some(start) = debug_start {
        dungeon.floor = start.floor.max(1);
        if start.boss_room {
            dungeon.layer = dungeon.plan.len() - 1;
        }
    }
    let kind = dungeon.plan.layers[dungeon.layer][0];
    let (mut transform, mut health) = player.into_inner();
    enter_room(
        &mut commands,
        &assets,
        &mut rng,
        &stats,
        &mut dungeon,
        &mut run,
        &rooms,
        kind,
        &mut transform,
        &mut health,
    );
}

fn enter_room(
    commands: &mut Commands,
    assets: &GameAssets,
    rng: &mut GameRng,
    stats: &Stats,
    dungeon: &mut Dungeon,
    run: &mut RunStats,
    rooms: &Query<Entity, With<RoomEntity>>,
    kind: RoomKind,
    player_transform: &mut Transform,
    health: &mut Health,
) {
    for entity in rooms {
        commands.entity(entity).try_despawn();
    }
    dungeon.kind = kind;
    dungeon.state = RoomState::Busy;
    dungeon.quota = round_quota(kind, dungeon.floor, dungeon.layer);
    dungeon.kills_at_start = run.kills;
    dungeon.spawn_timer = 1.0;
    dungeon.boss_spawned = false;
    run.floor = dungeon.floor;
    run.room = dungeon.layer as u32 + 1;

    player_transform.translation.x = PLAYER_START.x;
    player_transform.translation.y = PLAYER_START.y;

    spawn_walls(commands);
    let title = match kind {
        RoomKind::Arena | RoomKind::Elite => format!(
            "Floor {} - Round {}: {}",
            dungeon.floor,
            dungeon.layer + 1,
            kind.name()
        ),
        _ => format!("Floor {} - {}", dungeon.floor, kind.name()),
    };
    spawn_banner(
        commands,
        title,
        PLAYER_START + Vec2::new(0.0, 160.0),
        ACCENT_COLOR,
        30.0,
    );

    match kind {
        RoomKind::Treasure => {
            commands.spawn((
                RoomEntity,
                Chest,
                Sprite::from_image(assets.rune.clone()),
                Transform::from_xyz(0.0, 40.0, 1.0).with_scale(Vec3::splat(SPRITE_SCALE * 1.8)),
                DespawnOnExit(AppState::InGame),
                children![(
                    Text2d::new("Treasure"),
                    TextFont {
                        font_size: FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(ACCENT_COLOR),
                    Transform::from_xyz(0.0, -14.0, 1.0)
                        .with_scale(Vec3::splat(1.0 / (SPRITE_SCALE * 1.8))),
                )],
            ));
        }
        RoomKind::Shop => spawn_shop(commands, assets, rng, stats),
        RoomKind::Rest => {
            health.current = (health.current + health.max * REST_HEAL_FRACTION).min(health.max);
            spawn_banner(
                commands,
                "You rest and recover".into(),
                Vec2::new(0.0, 0.0),
                Color::srgb(0.53, 0.78, 0.40),
                22.0,
            );
            dungeon.state = RoomState::Cleared;
            spawn_doors(commands, dungeon);
        }
        _ => {}
    }
    if kind == RoomKind::Shop {
        dungeon.state = RoomState::Cleared;
        spawn_doors(commands, dungeon);
    }
}

fn spawn_walls(commands: &mut Commands) {
    let wall_color = Color::srgb(0.24, 0.21, 0.31);
    let void_color = Color::srgb(0.03, 0.03, 0.05);
    let thickness = 28.0;
    let (hx, hy) = (ARENA_HALF.x, ARENA_HALF.y);
    // Walls frame the arena; the void panels hide the endless floor behind them.
    for (center, size, color, z) in [
        (
            Vec2::new(0.0, hy + thickness / 2.0),
            Vec2::new(hx * 2.0 + thickness * 2.0, thickness),
            wall_color,
            -4.0,
        ),
        (
            Vec2::new(0.0, -hy - thickness / 2.0),
            Vec2::new(hx * 2.0 + thickness * 2.0, thickness),
            wall_color,
            -4.0,
        ),
        (
            Vec2::new(-hx - thickness / 2.0, 0.0),
            Vec2::new(thickness, hy * 2.0),
            wall_color,
            -4.0,
        ),
        (
            Vec2::new(hx + thickness / 2.0, 0.0),
            Vec2::new(thickness, hy * 2.0),
            wall_color,
            -4.0,
        ),
        (
            Vec2::new(0.0, hy + 1000.0),
            Vec2::new(9000.0, 2000.0),
            void_color,
            -3.0,
        ),
        (
            Vec2::new(0.0, -hy - 1000.0),
            Vec2::new(9000.0, 2000.0),
            void_color,
            -3.0,
        ),
        (
            Vec2::new(-hx - 2000.0, 0.0),
            Vec2::new(4000.0, 8000.0),
            void_color,
            -3.0,
        ),
        (
            Vec2::new(hx + 2000.0, 0.0),
            Vec2::new(4000.0, 8000.0),
            void_color,
            -3.0,
        ),
    ] {
        commands.spawn((
            RoomEntity,
            Sprite::from_color(color, size),
            Transform::from_xyz(center.x, center.y, z),
            DespawnOnExit(AppState::InGame),
        ));
    }
}

fn spawn_shop(commands: &mut Commands, assets: &GameAssets, rng: &mut GameRng, stats: &Stats) {
    let upgrades = roll_upgrades(&mut rng.0, stats, 2);
    let mut offers = vec![(ShopOffer::Heal, HEAL_PRICE, "Healing Draught".to_string())];
    for upgrade in upgrades {
        offers.push((
            ShopOffer::Upgrade(upgrade),
            UPGRADE_PRICE,
            upgrade.title(stats),
        ));
    }
    let count = offers.len() as f32;
    for (i, (offer, price, title)) in offers.into_iter().enumerate() {
        let x = (i as f32 - (count - 1.0) / 2.0) * 220.0;
        let image = match offer {
            ShopOffer::Heal => assets.gem_image(5),
            ShopOffer::Upgrade(_) => assets.gem_image(20),
        };
        commands.spawn((
            RoomEntity,
            ShopItem {
                offer,
                price,
                denied: 0.0,
            },
            Sprite::from_image(image),
            Transform::from_xyz(x, 20.0, 1.0).with_scale(Vec3::splat(SPRITE_SCALE * 1.8)),
            DespawnOnExit(AppState::InGame),
            children![(
                Text2d::new(format!("{title}\n{price} gold")),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(TEXT_COLOR),
                Transform::from_xyz(0.0, -16.0, 1.0)
                    .with_scale(Vec3::splat(1.0 / (SPRITE_SCALE * 1.8))),
            )],
        ));
    }
}

fn door_color(target: DoorTarget) -> Color {
    match target {
        DoorTarget::NextFloor => Color::srgb(0.95, 0.93, 0.87),
        DoorTarget::Room(kind) => match kind {
            RoomKind::Arena => Color::srgb(0.75, 0.18, 0.18),
            RoomKind::Elite => Color::srgb(0.88, 0.66, 0.19),
            RoomKind::Treasure => Color::srgb(0.38, 0.79, 0.75),
            RoomKind::Shop => Color::srgb(0.56, 0.42, 0.94),
            RoomKind::Rest => Color::srgb(0.30, 0.50, 0.27),
            RoomKind::Boss => Color::srgb(0.93, 0.42, 0.29),
        },
    }
}

/// Opens the doors to the next rooms (or the stairs down after the boss).
fn spawn_doors(commands: &mut Commands, dungeon: &Dungeon) {
    let targets: Vec<DoorTarget> = match dungeon.plan.layers.get(dungeon.layer + 1) {
        Some(layer) => layer.iter().map(|&kind| DoorTarget::Room(kind)).collect(),
        None => vec![DoorTarget::NextFloor],
    };
    let count = targets.len() as f32;
    for (i, target) in targets.into_iter().enumerate() {
        let x = (i as f32 - (count - 1.0) / 2.0) * 240.0;
        let name = match target {
            DoorTarget::Room(kind) => kind.name(),
            DoorTarget::NextFloor => "Descend",
        };
        commands.spawn((
            RoomEntity,
            Door { target },
            Sprite::from_color(door_color(target), Vec2::new(56.0, 72.0)),
            Transform::from_xyz(x, DOOR_Y, 0.5),
            DespawnOnExit(AppState::InGame),
            children![(
                Text2d::new(name),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(TEXT_COLOR),
                Transform::from_xyz(0.0, -52.0, 1.0),
            )],
        ));
    }
}

fn spawn_banner(commands: &mut Commands, text: String, position: Vec2, color: Color, size: f32) {
    commands.spawn((
        Banner {
            lifetime: BANNER_SECS,
        },
        Text2d::new(text),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
        Transform::from_xyz(position.x, position.y, 8.0),
        DespawnOnExit(AppState::InGame),
    ));
}

fn fade_banners(
    mut commands: Commands,
    time: Res<Time>,
    mut banners: Query<(Entity, &mut Transform, &mut TextColor, &mut Banner)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut color, mut banner) in &mut banners {
        banner.lifetime -= dt;
        if banner.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        transform.translation.y += 14.0 * dt;
        color.0 = color
            .0
            .with_alpha((banner.lifetime / BANNER_SECS * 2.0).min(1.0));
    }
}

// ---------------------------------------------------------------------------
// Rounds and bosses
// ---------------------------------------------------------------------------

/// A spawn point in a ring around the player, kept inside the walls. Standing at a wall
/// can push the point closer, so a few angles are tried to stay off screen if possible.
fn ring_spawn_position(rng: &mut GameRng, player: Vec2) -> Vec2 {
    let limit = ARENA_HALF - Vec2::splat(60.0);
    let mut best = Vec2::ZERO;
    let mut best_distance = -1.0;
    for _ in 0..8 {
        let angle = rng.0.range_f32(0.0, TAU);
        let distance = rng.0.range_f32(SPAWN_RING.0, SPAWN_RING.1);
        let candidate = (player + Vec2::from_angle(angle) * distance).clamp(-limit, limit);
        let actual = candidate.distance(player);
        if actual > best_distance {
            best = candidate;
            best_distance = actual;
        }
        if actual >= SPAWN_RING.0 * 0.9 {
            break;
        }
    }
    best
}

/// Streams enemies into combat rounds, finishes a round once enough of them are defeated
/// and starts the boss fight in boss rooms.
fn round_director(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut dungeon: ResMut<Dungeon>,
    mut run: ResMut<RunStats>,
    mut pending: ResMut<PendingLevelUps>,
    mut next: ResMut<NextState<AppState>>,
    mut sounds: MessageWriter<PlaySound>,
    mut damage: MessageWriter<DamageEnemy>,
    enemies: Query<Entity, With<Enemy>>,
    bosses: Query<(), With<Boss>>,
    mut gems: Query<&mut XpGem>,
    player: Single<(&Transform, &mut Health), With<Player>>,
) {
    if dungeon.state != RoomState::Busy || !dungeon.kind.is_combat() || !dungeon.initialized {
        return;
    }
    let (player_transform, mut health) = player.into_inner();
    let alive = enemies.iter().count();
    let (floor, layer, kind) = (dungeon.floor, dungeon.layer, dungeon.kind);

    let finished = if kind == RoomKind::Boss {
        if !dungeon.boss_spawned {
            dungeon.boss_spawned = true;
            let boss = pick_boss(&mut rng.0, floor);
            spawn_boss(&mut commands, &assets, floor, boss);
            return;
        }
        // The fight ends with the boss; whatever it summoned falls with it.
        if bosses.is_empty() {
            kill_all(&enemies, &mut damage)
        } else {
            false
        }
    } else {
        let defeated = run.kills.saturating_sub(dungeon.kills_at_start);
        if defeated < dungeon.quota {
            dungeon.spawn_timer -= time.delta_secs();
            if dungeon.spawn_timer <= 0.0 && alive < round_alive_cap(kind, floor) {
                dungeon.spawn_timer = round_spawn_interval(kind, floor, layer);
                let player_position = player_transform.translation.truncate();
                let batch = round_batch_size(kind, floor, layer);
                let difficulty = difficulty_time(floor, layer);
                let multiplier = enemy_health_multiplier(floor, layer);
                for _ in 0..batch {
                    let enemy_kind = pick_enemy_kind(&mut rng.0, difficulty);
                    let position = ring_spawn_position(&mut rng, player_position);
                    if kind == RoomKind::Elite {
                        let mut spawn = EnemySpawn::new(enemy_kind, position, multiplier * 3.5);
                        spawn.radius_factor = 1.3;
                        spawn.visual_factor = 1.4;
                        spawn.xp_factor = 3;
                        spawn.tint = Color::srgb(1.0, 0.85, 0.45);
                        spawn_enemy_with(&mut commands, &assets, spawn);
                    } else {
                        spawn_enemy(&mut commands, &assets, enemy_kind, position, multiplier);
                    }
                }
            }
            false
        } else {
            // Quota reached: whatever is still alive falls with the end of the round.
            kill_all(&enemies, &mut damage)
        }
    };
    if !finished {
        return;
    }

    // Round cleared.
    dungeon.state = RoomState::Cleared;
    sounds.write(PlaySound(Sfx::Clear));
    let gold = clear_gold(kind, floor);
    run.gold += gold;
    spawn_banner(
        &mut commands,
        format!("Round cleared  +{gold} gold"),
        player_transform.translation.truncate() + Vec2::new(0.0, 80.0),
        ACCENT_COLOR,
        26.0,
    );
    // Rewards: tougher rooms grant a free upgrade choice.
    match kind {
        RoomKind::Elite => pending.0 += 1,
        RoomKind::Boss => {
            pending.0 += 2;
            health.current = health.max;
        }
        _ => health.current = (health.current + health.max * 0.1).min(health.max),
    }
    for mut gem in &mut gems {
        gem.attracted = true;
    }

    if kind == RoomKind::Boss && floor >= FLOORS {
        run.victory = true;
        next.set(AppState::GameOver);
        return;
    }
    spawn_doors(&mut commands, &dungeon);
}

/// Sends a killing blow to every enemy. Returns `true` once none are left.
///
/// The blows are applied in the next frame's damage step, so the round is only finished
/// when a later call finds the arena empty; that also keeps the messages from being lost
/// if a level-up pauses the game in between.
fn kill_all(enemies: &Query<Entity, With<Enemy>>, damage: &mut MessageWriter<DamageEnemy>) -> bool {
    let mut any = false;
    for entity in enemies {
        any = true;
        damage.write(DamageEnemy {
            entity,
            amount: 1.0e9,
        });
    }
    !any
}

fn spawn_boss(commands: &mut Commands, assets: &GameAssets, floor: u32, boss: BossKind) {
    let pattern = boss_pattern(boss);
    let max_health = boss_health(floor, boss);
    let base = pattern.kind.archetype();
    let mut spawn = EnemySpawn::new(
        pattern.kind,
        Vec2::new(0.0, 300.0),
        max_health / base.health,
    );
    spawn.radius_factor = pattern.radius / base.radius;
    spawn.ranged = false;
    spawn.visual_factor = pattern.visual;
    spawn.xp_factor = 25;
    let tint = Color::srgb(pattern.tint[0], pattern.tint[1], pattern.tint[2]);
    spawn.tint = tint;
    if pattern.is_end_boss {
        spawn.frames = Some(assets.king.clone());
    }
    let entity = spawn_enemy_with(commands, assets, spawn);
    commands.entity(entity).insert((
        Boss {
            pattern,
            max_health,
            tint,
            bolt_timer: 2.5,
            spiral_timer: 1.0,
            summon_timer: 7.0,
            charge_timer: CHARGE_COOLDOWN,
            charge: Charge::Idle,
            spin: 0.0,
            spiral_spin: 0.0,
        },
        RoomEntity,
    ));
    spawn_banner(
        commands,
        pattern.name.to_string(),
        PLAYER_START + Vec2::new(0.0, 200.0),
        Color::srgb(0.93, 0.42, 0.29),
        36.0,
    );

    // Name and health bar below the HUD lines, centered at the top of the screen.
    commands.spawn((
        BossBarRoot,
        RoomEntity,
        Node {
            position_type: PositionType::Absolute,
            top: px(96.0),
            width: percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(4.0),
            ..default()
        },
        DespawnOnExit(AppState::InGame),
        children![
            label(pattern.name, 20.0, Color::srgb(0.93, 0.42, 0.29)),
            (
                Node {
                    width: px(460.0),
                    height: px(16.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.07, 0.10, 0.92)),
                children![(
                    BossBarFill,
                    Node {
                        width: percent(100.0),
                        height: percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.75, 0.18, 0.18)),
                )],
            ),
        ],
    ));
}

/// Boss attacks: a bolt ring (with an aimed fan), a bolt spiral, summons and charges.
/// Every phase (two thirds and one third of the health) makes them denser and faster.
fn boss_logic(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    dungeon: Res<Dungeon>,
    mut sounds: MessageWriter<PlaySound>,
    player: Single<&Transform, (With<Player>, Without<Boss>)>,
    mut bosses: Query<(&mut Transform, &mut Sprite, &mut Boss, &Enemy)>,
) {
    let dt = time.delta_secs();
    let target = player.translation.truncate();
    for (mut transform, mut sprite, mut boss, enemy) in &mut bosses {
        let origin = transform.translation.truncate();
        let pattern = boss.pattern;
        let phase = boss_phase(enemy.health / boss.max_health);

        // Ring of bolts, plus an aimed fan on top: the part that has to be dodged sideways.
        boss.bolt_timer -= dt;
        if boss.bolt_timer <= 0.0 {
            boss.bolt_timer = (2.8 - 0.4 * phase as f32).max(1.4);
            boss.spin += 0.37;
            sounds.write(PlaySound(Sfx::Boss));
            let count = pattern.ring + 2 * phase;
            for i in 0..count {
                let angle = boss.spin + i as f32 * TAU / count as f32;
                spawn_bolt(
                    &mut commands,
                    &assets,
                    origin,
                    Vec2::from_angle(angle) * RING_BOLT_SPEED,
                    14.0,
                );
            }
            if pattern.aimed > 0 {
                let aim = (target - origin).normalize_or_zero();
                let middle = (pattern.aimed as f32 - 1.0) / 2.0;
                for i in 0..pattern.aimed {
                    let direction = Vec2::from_angle((i as f32 - middle) * 0.28).rotate(aim);
                    spawn_bolt(
                        &mut commands,
                        &assets,
                        origin,
                        direction * AIMED_BOLT_SPEED,
                        12.0,
                    );
                }
            }
        }

        // A slowly turning spiral from the second phase on.
        if pattern.spiral_arms > 0 && phase >= 1 {
            boss.spiral_timer -= dt;
            if boss.spiral_timer <= 0.0 {
                boss.spiral_timer = SPIRAL_INTERVAL;
                boss.spiral_spin += 0.42;
                for arm in 0..pattern.spiral_arms {
                    let angle = boss.spiral_spin + arm as f32 * TAU / pattern.spiral_arms as f32;
                    spawn_bolt(
                        &mut commands,
                        &assets,
                        origin,
                        Vec2::from_angle(angle) * SPIRAL_BOLT_SPEED,
                        10.0,
                    );
                }
            }
        }

        boss.summon_timer -= dt;
        if boss.summon_timer <= 0.0 {
            boss.summon_timer = (9.0 - 1.5 * phase as f32).max(5.0);
            let multiplier = enemy_health_multiplier(dungeon.floor, dungeon.layer);
            let count = 3 + phase;
            for i in 0..count {
                let offset = Vec2::from_angle(i as f32 * TAU / count as f32) * 110.0;
                spawn_enemy(
                    &mut commands,
                    &assets,
                    pattern.summon,
                    origin + offset,
                    multiplier,
                );
            }
        }

        // Charge: light up, then rush towards where the player stood.
        let charges = pattern.charge_from_phase.is_some_and(|from| phase >= from);
        let mut color = boss.tint;
        match boss.charge {
            Charge::Idle => {
                if charges {
                    boss.charge_timer -= dt;
                    if boss.charge_timer <= 0.0 {
                        let direction = (target - origin).normalize_or_zero();
                        boss.charge = Charge::WindUp {
                            left: CHARGE_WINDUP,
                            direction,
                        };
                    }
                }
            }
            Charge::WindUp { left, direction } => {
                color = TELEGRAPH_COLOR;
                let left = left - dt;
                boss.charge = if left <= 0.0 {
                    Charge::Dash {
                        left: CHARGE_DASH,
                        direction,
                    }
                } else {
                    Charge::WindUp { left, direction }
                };
            }
            Charge::Dash { left, direction } => {
                transform.translation.x += direction.x * CHARGE_SPEED * dt;
                transform.translation.y += direction.y * CHARGE_SPEED * dt;
                let left = left - dt;
                if left <= 0.0 {
                    boss.charge = Charge::Idle;
                    boss.charge_timer = (CHARGE_COOLDOWN - 0.8 * phase as f32).max(1.8);
                } else {
                    boss.charge = Charge::Dash { left, direction };
                }
            }
        }
        if sprite.color != color {
            sprite.color = color;
        }
    }
}

// ---------------------------------------------------------------------------
// Doors, chests and shops
// ---------------------------------------------------------------------------

fn use_doors(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    stats: Res<Stats>,
    mut dungeon: ResMut<Dungeon>,
    mut run: ResMut<RunStats>,
    mut sounds: MessageWriter<PlaySound>,
    rooms: Query<Entity, With<RoomEntity>>,
    doors: Query<(&Transform, &Door)>,
    player: Single<(&mut Transform, &mut Health), (With<Player>, Without<Door>)>,
) {
    if dungeon.state != RoomState::Cleared {
        return;
    }
    let (mut transform, mut health) = player.into_inner();
    let position = transform.translation.truncate();
    let Some(target) = doors
        .iter()
        .find(|(door_transform, _)| {
            door_transform.translation.truncate().distance(position) <= DOOR_REACH
        })
        .map(|(_, door)| door.target)
    else {
        return;
    };

    sounds.write(PlaySound(Sfx::Door));
    let kind = match target {
        DoorTarget::Room(kind) => {
            dungeon.layer += 1;
            kind
        }
        DoorTarget::NextFloor => {
            dungeon.floor += 1;
            dungeon.layer = 0;
            dungeon.plan = FloorPlan::generate(&mut rng.0);
            dungeon.plan.layers[0][0]
        }
    };
    enter_room(
        &mut commands,
        &assets,
        &mut rng,
        &stats,
        &mut dungeon,
        &mut run,
        &rooms,
        kind,
        &mut transform,
        &mut health,
    );
}

fn interact_props(
    mut commands: Commands,
    time: Res<Time>,
    mut stats: ResMut<Stats>,
    mut dungeon: ResMut<Dungeon>,
    mut run: ResMut<RunStats>,
    mut pending: ResMut<PendingLevelUps>,
    mut sounds: MessageWriter<PlaySound>,
    chests: Query<(Entity, &Transform), With<Chest>>,
    mut items: Query<(Entity, &Transform, &mut ShopItem)>,
    player: Single<(&Transform, &mut Health), (With<Player>, Without<Chest>, Without<ShopItem>)>,
) {
    let (player_transform, mut health) = player.into_inner();
    let position = player_transform.translation.truncate();
    let near =
        |transform: &Transform| transform.translation.truncate().distance(position) <= PROP_REACH;

    for (entity, transform) in &chests {
        if !near(transform) {
            continue;
        }
        commands.entity(entity).try_despawn();
        pending.0 += 1;
        sounds.write(PlaySound(Sfx::Coin));
        let gold = clear_gold(RoomKind::Treasure, dungeon.floor);
        run.gold += gold;
        spawn_banner(
            &mut commands,
            format!("Treasure!  +{gold} gold and a free upgrade"),
            Vec2::new(0.0, 0.0),
            ACCENT_COLOR,
            24.0,
        );
        dungeon.state = RoomState::Cleared;
        spawn_doors(&mut commands, &dungeon);
    }

    let dt = time.delta_secs();
    for (entity, transform, mut item) in &mut items {
        item.denied = (item.denied - dt).max(0.0);
        if !near(transform) {
            continue;
        }
        if run.gold < item.price {
            if item.denied <= 0.0 {
                item.denied = 1.2;
                spawn_banner(
                    &mut commands,
                    "Not enough gold".into(),
                    transform.translation.truncate() + Vec2::new(0.0, 70.0),
                    Color::srgb(0.90, 0.30, 0.30),
                    18.0,
                );
            }
            continue;
        }
        run.gold -= item.price;
        sounds.write(PlaySound(Sfx::Coin));
        match item.offer {
            ShopOffer::Heal => {
                health.current = (health.current + health.max * 0.5).min(health.max);
            }
            ShopOffer::Upgrade(upgrade) => {
                upgrade.apply(&mut stats);
                health.max = stats.max_health;
                if upgrade == Upgrade::MaxHealth {
                    health.current = (health.current + Upgrade::MAX_HEALTH_HEAL).min(health.max);
                }
            }
        }
        spawn_banner(
            &mut commands,
            "Purchased".into(),
            transform.translation.truncate() + Vec2::new(0.0, 70.0),
            Color::srgb(0.53, 0.78, 0.40),
            18.0,
        );
        commands.entity(entity).try_despawn();
    }
}

fn clamp_to_arena(mut player: Single<&mut Transform, With<Player>>) {
    let limit = ARENA_HALF - Vec2::splat(WALL_MARGIN);
    player.translation.x = player.translation.x.clamp(-limit.x, limit.x);
    player.translation.y = player.translation.y.clamp(-limit.y, limit.y);
}

// ---------------------------------------------------------------------------
// HUD
// ---------------------------------------------------------------------------

fn spawn_dungeon_hud(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(44.0),
            right: px(16.0),
            ..default()
        },
        DespawnOnExit(AppState::InGame),
        children![(DungeonHudText, label("", 18.0, MUTED_TEXT_COLOR))],
    ));
}

fn update_dungeon_hud(
    dungeon: Res<Dungeon>,
    run: Res<RunStats>,
    mut text: Single<&mut Text, With<DungeonHudText>>,
) {
    let progress = if dungeon.state == RoomState::Busy && dungeon.kind.is_combat() {
        if dungeon.kind == RoomKind::Boss {
            "  -  defeat the boss".to_string()
        } else {
            let defeated = run
                .kills
                .saturating_sub(dungeon.kills_at_start)
                .min(dungeon.quota);
            format!("  -  defeated {}/{}", defeated, dungeon.quota)
        }
    } else if dungeon.state == RoomState::Cleared {
        "  -  choose a door".to_string()
    } else {
        String::new()
    };
    text.0 = format!(
        "Floor {}/{}   Round {}/{}  {}{}\nGold {}",
        dungeon.floor,
        FLOORS,
        dungeon.layer + 1,
        dungeon.plan.len().max(1),
        dungeon.kind.name(),
        progress,
        run.gold,
    );
}

fn update_boss_bar(
    bosses: Query<(&Enemy, &Boss)>,
    mut root: Query<&mut Node, (With<BossBarRoot>, Without<BossBarFill>)>,
    mut fill: Query<&mut Node, (With<BossBarFill>, Without<BossBarRoot>)>,
) {
    let fraction = bosses
        .iter()
        .next()
        .map(|(enemy, boss)| (enemy.health / boss.max_health).clamp(0.0, 1.0));
    for mut node in &mut root {
        node.display = if fraction.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Some(fraction) = fraction {
        for mut node in &mut fill {
            node.width = percent(fraction * 100.0);
        }
    }
}
