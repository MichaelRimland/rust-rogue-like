//! Camera and the endless forest floor.

use bevy::{
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter},
    prelude::*,
    window::PrimaryWindow,
};

use crate::{
    AppState, GameSet,
    assets::{GameAssets, SPRITE_SCALE},
    dungeon::{ARENA_HALF, Dungeon, GameMode},
    logic::dungeon::{Biome, biome_for_floor},
    player::Player,
    run::GameRng,
};

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraShake>()
            .init_resource::<CurrentBiome>()
            .add_systems(Startup, (spawn_camera, spawn_ground.after(spawn_camera)))
            .add_systems(
                OnEnter(AppState::InGame),
                (reset_camera, spawn_decor.after(crate::run::start_run)),
            )
            .add_systems(
                Update,
                (sync_biome, follow_player, recycle_decor, drift_motes)
                    .chain()
                    .after(GameSet::Movement)
                    .before(GameSet::Combat)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

/// Half extents of the area the decoration tiles wrap around in.
/// Must be larger than half the visible area so wrapping happens off screen.
const DECOR_HALF_EXTENT: Vec2 = Vec2::new(900.0, 600.0);
const DECOR_COUNT: usize = 420;
/// Radius of the light pool under glowing decoration, in sprite pixels.
const DECOR_LIGHT_RADIUS: f32 = 30.0;
/// How quickly the camera catches up with the player (higher = snappier).
const CAMERA_FOLLOW_SPEED: f32 = 8.0;

#[derive(Component)]
struct Decor;

/// A drifting speck: pollen in the forest, dust in the crypt, rising embers in the inferno.
#[derive(Component)]
struct Mote {
    velocity: Vec2,
    lifetime: f32,
    max_lifetime: f32,
    size: f32,
}

/// Most motes alive at the same time.
const MAX_MOTES: usize = 140;

/// The look the world currently has. Follows the dungeon floor; endless mode stays in
/// the forest.
#[derive(Resource, Default)]
pub struct CurrentBiome(pub Biome);

/// The tiled forest floor, kept centered on the camera in whole tiles.
#[derive(Component)]
struct Ground;

/// A sparse overlay on top of the ground. Its tile size differs from the base tile's, so
/// the combined pattern repeats only every few hundred pixels.
#[derive(Component)]
struct GroundDetail {
    index: usize,
}

/// Source tile sizes (in sprite pixels) of the two detail layers.
const DETAIL_TILE_PIXELS: [f32; 2] = [40.0, 56.0];

/// Brightness of the ground; lava floors get above 1.0 so their veins glow.
fn ground_tint(biome: Biome) -> Color {
    match biome {
        Biome::Inferno => Color::srgb(1.3, 1.22, 1.22),
        _ => Color::WHITE,
    }
}

/// Size of the floor sprite in world units; a multiple of the on-screen tile size so it
/// can be snapped to the tile grid, and larger than any window we expect.
const GROUND_SIZE: Vec2 = Vec2::new(3072.0, 1920.0);
const GROUND_TILE: f32 = 32.0 * SPRITE_SCALE;
/// Highest camera offset in world units at full trauma.
const SHAKE_MAX_OFFSET: f32 = 14.0;
/// How quickly trauma fades (per second).
const SHAKE_DECAY: f32 = 1.8;

/// Screen shake: events add trauma, the camera offset grows with its square.
#[derive(Resource, Default)]
pub struct CameraShake {
    trauma: f32,
    /// Where the camera would be without shake; shake is applied on top of it.
    base: Vec2,
}

impl CameraShake {
    pub fn add_trauma(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }
}

fn tiled_sprite(image: Handle<Image>, color: Color) -> Sprite {
    Sprite {
        image,
        color,
        custom_size: Some(GROUND_SIZE),
        image_mode: SpriteImageMode::Tiled {
            tile_x: true,
            tile_y: true,
            stretch_value: SPRITE_SCALE,
        },
        ..default()
    }
}

fn spawn_ground(mut commands: Commands, assets: Res<GameAssets>) {
    commands.spawn((
        Ground,
        tiled_sprite(assets.ground(Biome::Forest), ground_tint(Biome::Forest)),
        Transform::from_xyz(0.0, 0.0, -20.0),
    ));
    let details = assets.detail(Biome::Forest);
    for (index, image) in details.into_iter().enumerate() {
        commands.spawn((
            GroundDetail { index },
            tiled_sprite(image, Color::WHITE),
            Transform::from_xyz(0.0, 0.0, -19.5 + index as f32 * 0.1),
        ));
    }
}

/// Puts the ground and its detail overlays into the look of `biome`.
fn dress_ground(
    assets: &GameAssets,
    biome: Biome,
    ground: &mut Sprite,
    details: &mut Query<(&GroundDetail, &mut Sprite), Without<Ground>>,
) {
    ground.image = assets.ground(biome);
    ground.color = ground_tint(biome);
    let images = assets.detail(biome);
    for (detail, mut sprite) in details.iter_mut() {
        sprite.image = images[detail.index].clone();
    }
}

/// The mouse cursor in world coordinates, `None` while it is outside the window.
pub(crate) fn cursor_world_position(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
) -> Option<Vec2> {
    let cursor = window.cursor_position()?;
    camera.viewport_to_world_2d(camera_transform, cursor).ok()
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        // Colors brighter than 1.0 (spells, gems, lava, candles) glow; everything else is
        // left untouched, so the palette keeps its exact colors.
        Tonemapping::None,
        Bloom {
            intensity: 0.26,
            prefilter: BloomPrefilter {
                threshold: 1.0,
                threshold_softness: 0.5,
            },
            composite_mode: BloomCompositeMode::Additive,
            ..Bloom::NATURAL
        },
        DebandDither::Enabled,
    ));
}

fn reset_camera(
    mut camera: Single<&mut Transform, With<Camera2d>>,
    mut shake: ResMut<CameraShake>,
) {
    camera.translation.x = 0.0;
    camera.translation.y = 0.0;
    *shake = CameraShake::default();
}

/// Scatters the decoration of a biome so the player can see that they are moving.
fn scatter_decor(commands: &mut Commands, assets: &GameAssets, rng: &mut GameRng, biome: Biome) {
    let decor = assets.decor(biome);
    let weights: Vec<f32> = decor.iter().map(|def| def.weight).collect();
    for _ in 0..DECOR_COUNT {
        let x = rng.0.range_f32(-DECOR_HALF_EXTENT.x, DECOR_HALF_EXTENT.x);
        let y = rng.0.range_f32(-DECOR_HALF_EXTENT.y, DECOR_HALF_EXTENT.y);
        let def = &decor[rng.0.weighted_index(&weights).unwrap_or(0)];
        let mut sprite = Sprite::from_image(def.image.clone());
        sprite.flip_x = rng.0.chance(0.5);
        sprite.color = Color::srgb(def.emissive, def.emissive, def.emissive);
        let mut entity = commands.spawn((
            Decor,
            sprite,
            Transform::from_xyz(x, y, -10.0).with_scale(Vec3::splat(SPRITE_SCALE)),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(light) = &def.light {
            // A pool of light on the ground around candles, embers and lava.
            entity.with_children(|parent| {
                parent.spawn((
                    assets.light(*light, Vec2::splat(DECOR_LIGHT_RADIUS * 2.0)),
                    Transform::from_xyz(0.0, 0.0, -0.05),
                ));
            });
        }
    }
}

/// A new run starts in the forest.
fn spawn_decor(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut biome: ResMut<CurrentBiome>,
    mut ground: Single<&mut Sprite, (With<Ground>, Without<GroundDetail>)>,
    mut details: Query<(&GroundDetail, &mut Sprite), Without<Ground>>,
) {
    biome.0 = Biome::Forest;
    dress_ground(&assets, Biome::Forest, &mut ground, &mut details);
    scatter_decor(&mut commands, &assets, &mut rng, Biome::Forest);
}

/// Re-dresses the world when the dungeon moves on to a floor with another biome.
fn sync_biome(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mode: Res<GameMode>,
    dungeon: Res<Dungeon>,
    mut rng: ResMut<GameRng>,
    mut biome: ResMut<CurrentBiome>,
    mut ground: Single<&mut Sprite, (With<Ground>, Without<GroundDetail>)>,
    mut details: Query<(&GroundDetail, &mut Sprite), Without<Ground>>,
    decor: Query<Entity, With<Decor>>,
) {
    let wanted = match *mode {
        GameMode::Endless => Biome::Forest,
        GameMode::Dungeon => biome_for_floor(dungeon.floor),
    };
    if wanted == biome.0 {
        return;
    }
    biome.0 = wanted;
    dress_ground(&assets, wanted, &mut ground, &mut details);
    for entity in &decor {
        commands.entity(entity).despawn();
    }
    scatter_decor(&mut commands, &assets, &mut rng, wanted);
}

fn follow_player(
    time: Res<Time>,
    player: Single<&Transform, (With<Player>, Without<Camera2d>)>,
    mut camera: Single<&mut Transform, With<Camera2d>>,
    mut ground: Single<
        &mut Transform,
        (
            With<Ground>,
            Without<GroundDetail>,
            Without<Camera2d>,
            Without<Player>,
        ),
    >,
    mut details: Query<
        (&GroundDetail, &mut Transform),
        (Without<Ground>, Without<Camera2d>, Without<Player>),
    >,
    mut shake: ResMut<CameraShake>,
    mode: Res<GameMode>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    let dt = time.delta_secs();
    let target = match *mode {
        // The arena is bigger than the screen: follow the player, but stop at the walls so
        // the view never shows what lies beyond them.
        GameMode::Dungeon => {
            let reach = (ARENA_HALF - window.size() / 2.0).max(Vec2::ZERO);
            player.translation.truncate().clamp(-reach, reach)
        }
        GameMode::Endless => player.translation.truncate(),
    };
    // Frame-rate independent exponential smoothing.
    let t = 1.0 - (-CAMERA_FOLLOW_SPEED * dt).exp();
    shake.base = shake.base.lerp(target, t);

    // Cheap deterministic "noise" from the clock; no RNG needed for a visual effect.
    let clock = time.elapsed_secs();
    let noise = Vec2::new((clock * 61.0).sin(), (clock * 47.0 + 1.3).cos());
    let offset = noise * SHAKE_MAX_OFFSET * shake.trauma * shake.trauma;
    shake.trauma = (shake.trauma - SHAKE_DECAY * dt).max(0.0);

    let position = shake.base + offset;
    camera.translation.x = position.x;
    camera.translation.y = position.y;

    // The floor snaps to whole tiles, so the pattern never visibly slides.
    ground.translation.x = (shake.base.x / GROUND_TILE).round() * GROUND_TILE;
    ground.translation.y = (shake.base.y / GROUND_TILE).round() * GROUND_TILE;
    for (detail, mut transform) in &mut details {
        let tile = DETAIL_TILE_PIXELS[detail.index] * SPRITE_SCALE;
        transform.translation.x = (shake.base.x / tile).round() * tile;
        transform.translation.y = (shake.base.y / tile).round() * tile;
    }
}

/// Decoration that falls out of the area around the camera is moved to the opposite
/// side, which makes a few hundred entities look like an endless forest.
fn recycle_decor(
    camera: Single<&Transform, (With<Camera2d>, Without<Decor>)>,
    mut decor: Query<&mut Transform, With<Decor>>,
) {
    let center = camera.translation.truncate();
    let size = DECOR_HALF_EXTENT * 2.0;
    for mut transform in &mut decor {
        let offset = transform.translation.truncate() - center;
        if offset.x > DECOR_HALF_EXTENT.x {
            transform.translation.x -= size.x;
        } else if offset.x < -DECOR_HALF_EXTENT.x {
            transform.translation.x += size.x;
        }
        if offset.y > DECOR_HALF_EXTENT.y {
            transform.translation.y -= size.y;
        } else if offset.y < -DECOR_HALF_EXTENT.y {
            transform.translation.y += size.y;
        }
    }
}

/// A cheap random number in `[0, 1)` for purely cosmetic effects. It deliberately does not
/// touch the game's seeded RNG, so ambience never changes how a run plays out.
fn cosmetic_random(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (*state >> 8) as f32 / (1u32 << 24) as f32
}

/// Spawns, moves and fades the drifting motes around the camera.
fn drift_motes(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    biome: Res<CurrentBiome>,
    camera: Single<&Transform, (With<Camera2d>, Without<Mote>)>,
    mut motes: Query<(Entity, &mut Transform, &mut Mote)>,
    mut state: Local<u32>,
    mut owed: Local<f32>,
) {
    let dt = time.delta_secs();
    let mut count = 0;
    for (entity, mut transform, mut mote) in &mut motes {
        mote.lifetime -= dt;
        if mote.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        count += 1;
        let delta = mote.velocity * dt;
        transform.translation.x += delta.x;
        transform.translation.y += delta.y;
        // Grow in, shrink out: a gentle twinkle without needing per-entity alpha.
        let t = 1.0 - mote.lifetime / mote.max_lifetime;
        let size = mote.size * (t * std::f32::consts::PI).sin();
        transform.scale = Vec3::new(size, size, 1.0);
    }

    let rate = match biome.0 {
        Biome::Forest => 5.0,
        Biome::Crypt => 4.0,
        Biome::Inferno => 12.0,
    };
    *owed += rate * dt;
    while *owed >= 1.0 && count < MAX_MOTES {
        *owed -= 1.0;
        count += 1;
        let mut roll = || cosmetic_random(&mut state);
        let center = camera.translation.truncate();
        let position = center + Vec2::new((roll() - 0.5) * 1500.0, (roll() - 0.5) * 900.0);
        let (velocity, life, size) = match biome.0 {
            Biome::Forest => (
                Vec2::new((roll() - 0.5) * 18.0, 4.0 + roll() * 10.0),
                4.0 + roll() * 2.5,
                1.6 + roll() * 1.0,
            ),
            Biome::Crypt => (
                Vec2::new((roll() - 0.5) * 10.0, 3.0 + roll() * 8.0),
                5.0 + roll() * 3.0,
                1.2 + roll() * 0.9,
            ),
            Biome::Inferno => (
                Vec2::new((roll() - 0.5) * 26.0, 45.0 + roll() * 55.0),
                2.0 + roll() * 1.5,
                1.4 + roll() * 1.3,
            ),
        };
        commands.spawn((
            Mote {
                velocity,
                lifetime: life,
                max_lifetime: life,
                size,
            },
            Mesh2d(assets.circle.clone()),
            MeshMaterial2d(assets.mote(biome.0)),
            Transform::from_xyz(position.x, position.y, 4.5).with_scale(Vec3::ZERO),
            DespawnOnExit(AppState::InGame),
        ));
    }
}
