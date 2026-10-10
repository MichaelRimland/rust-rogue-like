//! Shared sprites, meshes and materials.
//!
//! Characters, projectiles, loot and decoration are pixel-art sprites (`assets/sprites`,
//! drawn by `tools/draw_sprites.py`). Effects that are just soft shapes (aura, particles,
//! impact rings) still use a shared unit circle mesh.

use bevy::{
    image::{ImageLoaderSettings, ImageSampler},
    prelude::*,
};

use crate::logic::{dungeon::Biome, enemies::EnemyKind};

/// Sprites are shown at this multiple of their pixel size (an integer keeps pixels square).
pub const SPRITE_SCALE: f32 = 2.0;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, create_assets);
    }
}

#[derive(Resource)]
pub struct GameAssets {
    /// Circle with radius 1.
    pub circle: Handle<Mesh>,

    /// Two walk frames per character.
    pub wizard: [Handle<Image>; 2],
    goblin: [Handle<Image>; 2],
    skeleton: [Handle<Image>; 2],
    ogre: [Handle<Image>; 2],
    archer: [Handle<Image>; 2],
    bat: [Handle<Image>; 2],
    warlock: [Handle<Image>; 2],

    pub missile: Handle<Image>,
    /// The end boss.
    pub king: [Handle<Image>; 2],
    /// Archway with a swirling portal; tinted in the color of the room behind it.
    pub portal: [Handle<Image>; 2],
    /// Ground tiles per biome, in the order of [`Biome::ALL`].
    ground: [Handle<Image>; 3],
    pub enemy_bolt: Handle<Image>,
    pub rune: Handle<Image>,
    gems: [Handle<Image>; 3],
    /// Soft halo behind gems so they stand out on the dark ground.
    /// Decoration per biome.
    decor: [Vec<DecorDef>; 3],
    /// Two sparse detail overlays per biome that hide the repetition of the ground tile.
    detail: [[Handle<Image>; 2]; 3],
    /// Colored light pools of projectiles, and the sparks of a hit (HDR, so they glow).
    pub spark: Handle<ColorMaterial>,

    pub aura: Handle<ColorMaterial>,
    /// The expanding ring of Frost Nova.
    pub ice: Handle<ColorMaterial>,
    /// Soft ellipse under characters, a warm light under the wizard and the bolt trail.
    /// A soft round gradient (linear sampling) for light pools and shadows.
    light: Handle<Image>,
    pub trail: Handle<ColorMaterial>,
    /// Drifting specks of the biomes (pollen, dust, embers), in the order of [`Biome::ALL`].
    motes: [Handle<ColorMaterial>; 3],
    /// Colors of the particles an enemy kind bursts into.
    particles: [Handle<ColorMaterial>; 6],
}

/// A kind of ground decoration.
pub struct DecorDef {
    pub image: Handle<Image>,
    /// Relative spawn weight.
    pub weight: f32,
    /// Color multiplier above 1.0 makes the sprite glow (bloom).
    pub emissive: f32,
    /// A light pool drawn under the decoration, for candles, embers and lava.
    pub light: Option<Color>,
}

impl GameAssets {
    pub fn enemy_frames(&self, kind: EnemyKind) -> [Handle<Image>; 2] {
        match kind {
            EnemyKind::Goblin => self.goblin.clone(),
            EnemyKind::Skeleton => self.skeleton.clone(),
            EnemyKind::Ogre => self.ogre.clone(),
            EnemyKind::Archer => self.archer.clone(),
            EnemyKind::Bat => self.bat.clone(),
            EnemyKind::Warlock => self.warlock.clone(),
        }
    }

    pub fn enemy_material(&self, kind: EnemyKind) -> Handle<ColorMaterial> {
        match kind {
            EnemyKind::Goblin => self.particles[0].clone(),
            EnemyKind::Skeleton => self.particles[1].clone(),
            EnemyKind::Ogre => self.particles[2].clone(),
            EnemyKind::Archer => self.particles[3].clone(),
            EnemyKind::Bat => self.particles[4].clone(),
            EnemyKind::Warlock => self.particles[5].clone(),
        }
    }

    pub fn mote(&self, biome: Biome) -> Handle<ColorMaterial> {
        self.motes[biome as usize].clone()
    }

    pub fn ground(&self, biome: Biome) -> Handle<Image> {
        self.ground[biome as usize].clone()
    }

    pub fn decor(&self, biome: Biome) -> &[DecorDef] {
        &self.decor[biome as usize]
    }

    pub fn detail(&self, biome: Biome) -> [Handle<Image>; 2] {
        self.detail[biome as usize].clone()
    }

    /// The gem sprite for an experience value: small, medium or large.
    pub fn gem_image(&self, xp: u32) -> Handle<Image> {
        self.gems[gem_tier(xp)].clone()
    }

    /// A soft light pool or shadow: a tinted, round gradient of the given size.
    pub fn light(&self, color: Color, size: Vec2) -> Sprite {
        Sprite {
            image: self.light.clone(),
            color,
            custom_size: Some(size),
            ..default()
        }
    }

    pub fn gem_glow(&self, xp: u32) -> Color {
        match gem_tier(xp) {
            0 => Color::srgba(0.38, 0.95, 0.90, 0.7),
            1 => Color::srgba(1.0, 0.89, 0.48, 0.7),
            _ => Color::srgba(0.66, 0.50, 1.0, 0.75),
        }
    }
}

fn gem_tier(xp: u32) -> usize {
    match xp {
        0..=3 => 0,
        4..=9 => 1,
        _ => 2,
    }
}

fn create_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let sprite = |name: &str| asset_server.load(format!("sprites/{name}.png"));
    let frames = |name: &str| [sprite(&format!("{name}_0")), sprite(&format!("{name}_1"))];
    let mut mat = |color: Color| materials.add(color);

    let warm = Color::srgba(1.0, 0.74, 0.32, 0.55);
    let ember = Color::srgba(1.0, 0.42, 0.14, 0.42);

    let assets = GameAssets {
        circle: meshes.add(Circle::new(1.0)),

        wizard: frames("wizard"),
        goblin: frames("goblin"),
        skeleton: frames("skeleton"),
        ogre: frames("ogre"),
        archer: frames("archer"),
        bat: frames("bat"),
        warlock: frames("warlock"),

        missile: sprite("missile"),
        king: frames("king"),
        portal: frames("portal"),
        ground: [
            sprite("ground"),
            sprite("ground_crypt"),
            sprite("ground_inferno"),
        ],
        enemy_bolt: sprite("enemy_bolt"),
        rune: sprite("rune"),
        gems: [
            sprite("gem_small"),
            sprite("gem_medium"),
            sprite("gem_large"),
        ],
        decor: {
            let plain = |image: Handle<Image>, weight: f32| DecorDef {
                image,
                weight,
                emissive: 1.0,
                light: None,
            };
            let lit = |image: Handle<Image>, weight: f32, emissive: f32, light| DecorDef {
                image,
                weight,
                emissive,
                light: Some(light),
            };
            [
                vec![
                    plain(sprite("decor_bush"), 3.0),
                    plain(sprite("decor_tuft"), 3.0),
                    plain(sprite("decor_stone"), 2.0),
                    plain(sprite("decor_mushroom"), 1.0),
                    plain(sprite("decor_bones"), 1.0),
                ],
                vec![
                    plain(sprite("decor_rubble"), 3.0),
                    plain(sprite("decor_skull"), 2.0),
                    plain(sprite("decor_pillar"), 1.5),
                    lit(sprite("decor_candle"), 1.1, 1.5, warm),
                    plain(sprite("decor_bones"), 2.0),
                ],
                vec![
                    lit(sprite("decor_ember"), 1.2, 1.6, ember),
                    plain(sprite("decor_obsidian"), 2.0),
                    lit(sprite("decor_lava"), 0.8, 1.6, ember),
                    plain(sprite("decor_charred"), 2.0),
                    plain(sprite("decor_rubble"), 1.0),
                ],
            ]
        },
        detail: [
            [sprite("detail_forest_a"), sprite("detail_forest_b")],
            [sprite("detail_crypt_a"), sprite("detail_crypt_b")],
            [sprite("detail_inferno_a"), sprite("detail_inferno_b")],
        ],
        spark: mat(Color::srgb(2.6, 2.2, 1.3)),

        aura: mat(Color::srgba(0.88, 0.66, 0.19, 0.16)),
        ice: mat(Color::srgba(0.38, 0.79, 0.75, 0.30)),
        light: asset_server
            .load_builder()
            .with_settings(|settings: &mut ImageLoaderSettings| {
                settings.sampler = ImageSampler::linear();
            })
            .load("sprites/light.png"),
        trail: mat(Color::srgba(0.66, 0.50, 1.0, 0.55)),
        motes: [
            mat(Color::srgb(1.6, 1.9, 0.8)),
            mat(Color::srgb(0.9, 1.0, 1.4)),
            mat(Color::srgb(3.0, 1.3, 0.4)),
        ],
        particles: [
            mat(Color::srgb(0.30, 0.50, 0.27)),
            mat(Color::srgb(0.83, 0.80, 0.82)),
            mat(Color::srgb(0.48, 0.29, 0.16)),
            mat(Color::srgb(0.48, 0.11, 0.15)),
            mat(Color::srgb(0.36, 0.23, 0.62)),
            mat(Color::srgb(0.16, 0.48, 0.50)),
        ],
    };
    commands.insert_resource(assets);
}
