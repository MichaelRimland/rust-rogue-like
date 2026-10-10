//! rust-rogue-like (working title): a fantasy survivor-roguelike.
//!
//! The crate is split into
//! - [`logic`]: engine-independent rules (stats, upgrades, progression, spatial hashing, RNG),
//!   fully unit tested,
//! - one Bevy plugin per gameplay feature, wired together by [`GamePlugin`].

pub mod logic;

mod animation;
mod assets;
mod audio;
mod autoplay;
mod bench;
mod combat;
mod dungeon;
mod enemy;
mod enemy_fire;
mod player;
mod progression;
mod run;
mod save;
mod skills;
mod states;
mod ui;
mod weapons;
mod world;

use bevy::prelude::*;

pub use states::{AppState, GamePhase};

/// Execution order of the gameplay systems inside `Update`.
///
/// All sets only run while a run is active and not paused or waiting for a level-up choice.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameSet {
    /// Rebuild spatial acceleration structures from last frame's positions.
    Spatial,
    /// Player input, enemy steering, projectiles flying.
    Movement,
    /// Weapons fire and detect hits.
    Combat,
    /// Damage is applied, enemies die, loot drops.
    Damage,
    /// Experience pickup, level-ups, run timer, game over check.
    Progression,
}

/// Bundles every plugin of the game.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.07, 0.10, 0.08)))
            .init_state::<AppState>()
            .add_sub_state::<GamePhase>()
            .configure_sets(
                Update,
                (
                    GameSet::Spatial,
                    GameSet::Movement,
                    GameSet::Combat,
                    GameSet::Damage,
                    GameSet::Progression,
                )
                    .chain()
                    .run_if(in_state(GamePhase::Running)),
            )
            .add_plugins((
                assets::AssetsPlugin,
                animation::AnimationPlugin,
                audio::GameAudioPlugin,
                world::WorldPlugin,
                run::RunPlugin,
                player::PlayerPlugin,
            ))
            .add_plugins((
                save::SavePlugin,
                dungeon::DungeonPlugin,
                enemy::EnemyPlugin,
                enemy_fire::EnemyFirePlugin,
                weapons::WeaponsPlugin,
                combat::CombatPlugin,
                progression::ProgressionPlugin,
                skills::SkillsPlugin,
                ui::UiPlugin,
                bench::BenchPlugin,
                autoplay::AutoplayPlugin,
            ));
    }
}
