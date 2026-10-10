//! Shared meshes and materials.
//!
//! Milestone 1 uses simple vector shapes instead of sprites, so every entity of a kind
//! shares the same mesh and material handle. Sizes are applied through `Transform::scale`
//! on unit-sized meshes. Swapping in pixel-art sprites later only touches this file and
//! the spawn functions.

use bevy::prelude::*;

use crate::logic::enemies::EnemyKind;

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
    /// Hexagon with circumradius 1.
    pub hexagon: Handle<Mesh>,
    /// Rhombus with diagonals 1 x 1.6.
    pub diamond: Handle<Mesh>,
    /// Triangle pointing up, roughly 1 unit tall.
    pub triangle: Handle<Mesh>,

    pub player_robe: Handle<ColorMaterial>,
    pub player_hat: Handle<ColorMaterial>,
    pub goblin: Handle<ColorMaterial>,
    pub skeleton: Handle<ColorMaterial>,
    pub ogre: Handle<ColorMaterial>,
    pub missile: Handle<ColorMaterial>,
    pub aura: Handle<ColorMaterial>,
    pub rune: Handle<ColorMaterial>,
    pub gem_small: Handle<ColorMaterial>,
    pub gem_medium: Handle<ColorMaterial>,
    pub gem_large: Handle<ColorMaterial>,
    pub decor: Vec<Handle<ColorMaterial>>,
}

impl GameAssets {
    pub fn enemy_material(&self, kind: EnemyKind) -> Handle<ColorMaterial> {
        match kind {
            EnemyKind::Goblin => self.goblin.clone(),
            EnemyKind::Skeleton => self.skeleton.clone(),
            EnemyKind::Ogre => self.ogre.clone(),
        }
    }

    pub fn gem_material(&self, xp: u32) -> Handle<ColorMaterial> {
        match xp {
            0..=1 => self.gem_small.clone(),
            2..=4 => self.gem_medium.clone(),
            _ => self.gem_large.clone(),
        }
    }
}

fn create_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut mat = |color: Color| materials.add(color);

    let assets = GameAssets {
        circle: meshes.add(Circle::new(1.0)),
        hexagon: meshes.add(RegularPolygon::new(1.0, 6)),
        diamond: meshes.add(Rhombus::new(1.0, 1.6)),
        triangle: meshes.add(Triangle2d::new(
            Vec2::new(0.0, 0.7),
            Vec2::new(-0.5, -0.3),
            Vec2::new(0.5, -0.3),
        )),

        player_robe: mat(crate::player::ROBE_COLOR),
        player_hat: mat(Color::srgb(0.20, 0.22, 0.60)),
        goblin: mat(Color::srgb(0.35, 0.70, 0.25)),
        skeleton: mat(Color::srgb(0.88, 0.86, 0.78)),
        ogre: mat(Color::srgb(0.60, 0.30, 0.22)),
        missile: mat(Color::srgb(0.75, 0.55, 1.0)),
        aura: mat(Color::srgba(1.0, 0.85, 0.35, 0.16)),
        rune: mat(Color::srgb(0.55, 0.85, 0.95)),
        gem_small: mat(Color::srgb(0.30, 0.75, 1.0)),
        gem_medium: mat(Color::srgb(0.30, 1.0, 0.55)),
        gem_large: mat(Color::srgb(1.0, 0.35, 0.45)),
        decor: vec![
            mat(Color::srgb(0.10, 0.15, 0.11)),
            mat(Color::srgb(0.12, 0.17, 0.10)),
            mat(Color::srgb(0.14, 0.14, 0.13)),
        ],
    };
    commands.insert_resource(assets);
}
