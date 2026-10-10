//! Shared sprites, meshes and materials.
//!
//! Characters, projectiles, loot and decoration are pixel-art sprites (`assets/sprites`,
//! drawn by `tools/draw_sprites.py`). Effects that are just soft shapes (aura, particles,
//! impact rings) still use a shared unit circle mesh.

use bevy::prelude::*;

use crate::logic::enemies::EnemyKind;

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
    pub ground: Handle<Image>,
    pub enemy_bolt: Handle<Image>,
    pub rune: Handle<Image>,
    gems: [Handle<Image>; 3],
    /// Soft halo behind gems so they stand out on the dark ground.
    gem_glows: [Handle<ColorMaterial>; 3],
    /// Decoration sprites paired with their spawn weight.
    pub decor: Vec<(Handle<Image>, f32)>,

    pub aura: Handle<ColorMaterial>,
    /// Colors of the particles an enemy kind bursts into.
    particles: [Handle<ColorMaterial>; 6],
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

    /// The gem sprite for an experience value: small, medium or large.
    pub fn gem_image(&self, xp: u32) -> Handle<Image> {
        self.gems[gem_tier(xp)].clone()
    }

    pub fn gem_glow(&self, xp: u32) -> Handle<ColorMaterial> {
        self.gem_glows[gem_tier(xp)].clone()
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
        ground: sprite("ground"),
        enemy_bolt: sprite("enemy_bolt"),
        rune: sprite("rune"),
        gems: [
            sprite("gem_small"),
            sprite("gem_medium"),
            sprite("gem_large"),
        ],
        gem_glows: [
            mat(Color::srgba(0.38, 0.79, 0.75, 0.30)),
            mat(Color::srgba(1.0, 0.89, 0.48, 0.30)),
            mat(Color::srgba(0.56, 0.42, 0.94, 0.32)),
        ],
        decor: vec![
            (sprite("decor_bush"), 3.0),
            (sprite("decor_tuft"), 3.0),
            (sprite("decor_stone"), 2.0),
            (sprite("decor_mushroom"), 1.0),
            (sprite("decor_bones"), 1.0),
        ],

        aura: mat(Color::srgba(0.88, 0.66, 0.19, 0.16)),
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
