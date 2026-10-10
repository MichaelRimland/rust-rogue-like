//! Dungeon mode: floors made of rooms with doors, an arena to fight in, shops, treasure,
//! resting places and a boss at the end of every floor.
//!
//! The structure (which rooms exist, how many waves, how hard) lives in
//! [`crate::logic::dungeon`]. This module spawns the rooms and runs the room flow.

use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::{
    AppState, GameSet,
    assets::{GameAssets, SPRITE_SCALE},
    enemy::{Enemy, EnemySpawn, spawn_enemy, spawn_enemy_with},
    enemy_fire::spawn_bolt,
    logic::{
        dungeon::{
            FLOORS, FloorPlan, RoomKind, boss_health, clear_gold, difficulty_time,
            enemy_health_multiplier, wave_count, wave_size,
        },
        enemies::{EnemyKind, pick_enemy_kind},
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
                    wave_spawner,
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

/// Half extents of the playable arena in world units.
pub const ARENA_HALF: Vec2 = Vec2::new(520.0, 300.0);
/// Distance the player keeps from the walls.
const WALL_MARGIN: f32 = 20.0;
const DOOR_REACH: f32 = 46.0;
const PROP_REACH: f32 = 44.0;
/// Enemies of a wave appear at least this far from the player.
const SPAWN_MIN_DISTANCE: f32 = 240.0;
const HEAL_PRICE: u32 = 30;
const UPGRADE_PRICE: u32 = 70;
const REST_HEAL_FRACTION: f32 = 0.4;

/// Which mode the next run is played in. Chosen in the main menu.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameMode {
    /// Survive as long as possible in an endless forest.
    Endless,
    /// Fight through rooms and floors, ending with a boss.
    #[default]
    Dungeon,
}

pub fn dungeon_mode(mode: Res<GameMode>) -> bool {
    *mode == GameMode::Dungeon
}

pub fn endless_mode(mode: Res<GameMode>) -> bool {
    *mode == GameMode::Endless
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
    wave: u32,
    waves: u32,
    last_wave_size: usize,
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
            wave: 0,
            waves: 0,
            last_wave_size: 0,
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

#[derive(Component)]
struct Boss {
    max_health: f32,
    bolt_timer: f32,
    summon_timer: f32,
    spin: f32,
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
    player: Single<(&mut Transform, &mut Health), With<Player>>,
) {
    if dungeon.initialized {
        return;
    }
    dungeon.initialized = true;
    dungeon.plan = FloorPlan::generate(&mut rng.0);
    let kind = dungeon.plan.layers[0][0];
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
    dungeon.wave = 0;
    dungeon.waves = wave_count(kind, dungeon.floor, dungeon.layer);
    dungeon.last_wave_size = 0;
    run.floor = dungeon.floor;
    run.room = dungeon.layer as u32 + 1;

    player_transform.translation.x = 0.0;
    player_transform.translation.y = -ARENA_HALF.y + 90.0;

    spawn_walls(commands);
    spawn_banner(
        commands,
        format!("Floor {} - {}", dungeon.floor, kind.name()),
        Vec2::new(0.0, 40.0),
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
            Vec2::new(0.0, hy + 500.0),
            Vec2::new(5000.0, 1000.0),
            void_color,
            -3.0,
        ),
        (
            Vec2::new(0.0, -hy - 500.0),
            Vec2::new(5000.0, 1000.0),
            void_color,
            -3.0,
        ),
        (
            Vec2::new(-hx - 1500.0, 0.0),
            Vec2::new(3000.0, 4000.0),
            void_color,
            -3.0,
        ),
        (
            Vec2::new(hx + 1500.0, 0.0),
            Vec2::new(3000.0, 4000.0),
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
            Transform::from_xyz(x, ARENA_HALF.y - 34.0, 0.5),
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
// Combat rooms
// ---------------------------------------------------------------------------

fn random_spawn_position(rng: &mut GameRng, player: Vec2) -> Vec2 {
    let mut position = Vec2::ZERO;
    for _ in 0..12 {
        position = Vec2::new(
            rng.0.range_f32(-ARENA_HALF.x + 40.0, ARENA_HALF.x - 40.0),
            rng.0.range_f32(-ARENA_HALF.y + 40.0, ARENA_HALF.y - 60.0),
        );
        if position.distance(player) >= SPAWN_MIN_DISTANCE {
            break;
        }
    }
    position
}

fn wave_spawner(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut dungeon: ResMut<Dungeon>,
    mut run: ResMut<RunStats>,
    mut pending: ResMut<PendingLevelUps>,
    mut next: ResMut<NextState<AppState>>,
    enemies: Query<(), With<Enemy>>,
    mut gems: Query<&mut XpGem>,
    player: Single<(&Transform, &mut Health), With<Player>>,
) {
    if dungeon.state != RoomState::Busy || !dungeon.kind.is_combat() || !dungeon.initialized {
        return;
    }
    let (player_transform, mut health) = player.into_inner();
    let alive = enemies.iter().count();

    if dungeon.wave < dungeon.waves {
        // The next wave arrives when the current one is nearly beaten, which keeps the
        // pressure on without waiting for the last straggler.
        if alive > dungeon.last_wave_size / 4 {
            return;
        }
        let (floor, layer) = (dungeon.floor, dungeon.layer);
        let player_position = player_transform.translation.truncate();
        if dungeon.kind == RoomKind::Boss {
            spawn_boss(&mut commands, &assets, floor);
            dungeon.last_wave_size = 1;
        } else {
            let size = wave_size(dungeon.kind, floor, layer, dungeon.wave) as usize;
            let time = difficulty_time(floor, layer);
            let multiplier = enemy_health_multiplier(floor, layer);
            for _ in 0..size {
                let kind = pick_enemy_kind(&mut rng.0, time);
                let position = random_spawn_position(&mut rng, player_position);
                if dungeon.kind == RoomKind::Elite {
                    let mut spawn = EnemySpawn::new(kind, position, multiplier * 3.5);
                    spawn.radius_factor = 1.3;
                    spawn.visual_factor = 1.4;
                    spawn.xp_factor = 3;
                    spawn.tint = Color::srgb(1.0, 0.85, 0.45);
                    spawn_enemy_with(&mut commands, &assets, spawn);
                } else {
                    spawn_enemy(&mut commands, &assets, kind, position, multiplier);
                }
            }
            dungeon.last_wave_size = size;
        }
        dungeon.wave += 1;
        return;
    }
    if alive > 0 {
        return;
    }

    // Room cleared.
    dungeon.state = RoomState::Cleared;
    let gold = clear_gold(dungeon.kind, dungeon.floor);
    run.gold += gold;
    spawn_banner(
        &mut commands,
        format!("Room cleared  +{gold} gold"),
        Vec2::new(0.0, 0.0),
        ACCENT_COLOR,
        26.0,
    );
    // Rewards: tougher rooms grant a free upgrade choice.
    match dungeon.kind {
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

    if dungeon.kind == RoomKind::Boss && dungeon.floor >= FLOORS {
        run.victory = true;
        next.set(AppState::GameOver);
        return;
    }
    spawn_doors(&mut commands, &dungeon);
}

fn spawn_boss(commands: &mut Commands, assets: &GameAssets, floor: u32) {
    let base_health = EnemyKind::Ogre.archetype().health;
    let max_health = boss_health(floor);
    let mut spawn = EnemySpawn::new(
        EnemyKind::Ogre,
        Vec2::new(0.0, 120.0),
        max_health / base_health,
    );
    spawn.radius_factor = 1.5;
    spawn.visual_factor = 3.0;
    spawn.xp_factor = 25;
    spawn.tint = Color::srgb(1.0, 0.62, 0.72);
    let entity = spawn_enemy_with(commands, assets, spawn);
    commands.entity(entity).insert((
        Boss {
            max_health,
            bolt_timer: 2.5,
            summon_timer: 7.0,
            spin: 0.0,
        },
        RoomEntity,
    ));

    // Health bar below the HUD line, centered at the top of the screen.
    commands.spawn((
        BossBarRoot,
        RoomEntity,
        Node {
            position_type: PositionType::Absolute,
            top: px(74.0),
            width: percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        DespawnOnExit(AppState::InGame),
        children![(
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
        )],
    ));
}

fn boss_logic(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    dungeon: Res<Dungeon>,
    mut bosses: Query<(&Transform, &mut Boss, &Enemy)>,
) {
    let dt = time.delta_secs();
    for (transform, mut boss, enemy) in &mut bosses {
        let origin = transform.translation.truncate();
        let enraged = enemy.health < boss.max_health * 0.5;

        boss.bolt_timer -= dt;
        if boss.bolt_timer <= 0.0 {
            boss.bolt_timer = if enraged { 1.9 } else { 2.8 };
            boss.spin += 0.37;
            let count = if enraged { 16 } else { 10 };
            for i in 0..count {
                let angle = boss.spin + i as f32 * TAU / count as f32;
                spawn_bolt(
                    &mut commands,
                    &assets,
                    origin,
                    Vec2::from_angle(angle) * 190.0,
                    14.0,
                );
            }
        }

        boss.summon_timer -= dt;
        if boss.summon_timer <= 0.0 {
            boss.summon_timer = 9.0;
            let multiplier = enemy_health_multiplier(dungeon.floor, dungeon.layer);
            for i in 0..3 {
                let offset = Vec2::from_angle(i as f32 * TAU / 3.0) * 90.0;
                spawn_enemy(
                    &mut commands,
                    &assets,
                    EnemyKind::Goblin,
                    origin + offset,
                    multiplier,
                );
            }
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
            format!(
                "  -  wave {}/{}",
                dungeon.wave.max(1).min(dungeon.waves),
                dungeon.waves
            )
        }
    } else if dungeon.state == RoomState::Cleared {
        "  -  choose a door".to_string()
    } else {
        String::new()
    };
    text.0 = format!(
        "Floor {}/{}   Room {}/{}  {}{}\nGold {}",
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
