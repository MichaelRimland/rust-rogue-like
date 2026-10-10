use bevy::prelude::*;

/// Top-level flow of the application.
#[derive(States, Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
pub enum AppState {
    #[default]
    MainMenu,
    InGame,
    GameOver,
    /// Spend soul shards on permanent upgrades.
    Shrine,
}

/// What is happening inside a run. Only exists while in [`AppState::InGame`].
#[derive(SubStates, Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
#[source(AppState = AppState::InGame)]
pub enum GamePhase {
    #[default]
    Running,
    /// The game is frozen while the player picks an upgrade.
    LevelUp,
    Paused,
}
