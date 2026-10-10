//! Screen-space atmosphere: a soft vignette and a color wash that follows the biome.

use bevy::{
    prelude::*,
    ui::{BackgroundGradient, ColorStop, RadialGradient, RadialGradientShape, UiPosition},
};

use crate::{logic::dungeon::Biome, world::CurrentBiome};

pub struct AmbiencePlugin;

impl Plugin for AmbiencePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ambience)
            .add_systems(Update, tint_for_biome);
    }
}

#[derive(Component)]
struct BiomeTint;

fn full_screen() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: percent(100.0),
        height: percent(100.0),
        ..default()
    }
}

fn spawn_ambience(mut commands: Commands) {
    // Both layers sit below the HUD (negative z index) and never take mouse input.
    commands.spawn((
        full_screen(),
        BackgroundGradient::from(RadialGradient::new(
            UiPosition::CENTER,
            RadialGradientShape::FarthestCorner,
            vec![
                ColorStop::percent(Color::srgba(0.0, 0.0, 0.02, 0.0), 42.0),
                ColorStop::percent(Color::srgba(0.0, 0.0, 0.02, 0.62), 100.0),
            ],
        )),
        GlobalZIndex(-10),
        Pickable::IGNORE,
    ));
    commands.spawn((
        BiomeTint,
        full_screen(),
        BackgroundColor(biome_tint(Biome::Forest)),
        GlobalZIndex(-11),
        Pickable::IGNORE,
    ));
}

/// A faint wash over the whole screen that gives each world its own mood.
///
/// Blending happens in linear light, so even a low alpha with a bright color would lift
/// the dark ground noticeably; the colors are kept dark on purpose.
fn biome_tint(biome: Biome) -> Color {
    match biome {
        Biome::Forest => Color::srgba(0.03, 0.20, 0.08, 0.10),
        Biome::Crypt => Color::srgba(0.06, 0.10, 0.34, 0.10),
        Biome::Inferno => Color::srgba(0.30, 0.03, 0.01, 0.10),
    }
}

fn tint_for_biome(
    biome: Res<CurrentBiome>,
    mut tint: Single<&mut BackgroundColor, With<BiomeTint>>,
) {
    if biome.is_changed() {
        tint.0 = biome_tint(biome.0);
    }
}
