//! Per-run resources: seed, RNG, stats, timer, pausing.

use bevy::prelude::*;

use crate::{
    AppState, GamePhase, GameSet,
    logic::{progression::Experience, rng::Rng, stats::PlayerStats},
};

pub struct RunPlugin;

impl Plugin for RunPlugin {
    fn build(&self, app: &mut App) {
        // Insert defaults so systems and UI never see missing resources.
        app.insert_resource(GameRng(Rng::new(0)))
            .init_resource::<Stats>()
            .init_resource::<Xp>()
            .init_resource::<RunStats>()
            .add_systems(OnEnter(AppState::InGame), start_run)
            .add_systems(Update, tick_run_timer.in_set(GameSet::Progression))
            .add_systems(Update, toggle_pause.run_if(in_state(AppState::InGame)));
    }
}

/// The single source of randomness for a run.
#[derive(Resource)]
pub struct GameRng(pub Rng);

/// The player's current (upgraded) stats.
#[derive(Resource, Default, Deref, DerefMut)]
pub struct Stats(pub PlayerStats);

/// The player's experience and level.
#[derive(Resource, Default, Deref, DerefMut)]
pub struct Xp(pub Experience);

/// Bookkeeping shown in the HUD and on the game over screen.
#[derive(Resource, Default)]
pub struct RunStats {
    pub seed: u64,
    pub elapsed: f32,
    pub kills: u32,
}

/// Resets everything for a fresh run.
pub fn start_run(mut commands: Commands, real_time: Res<Time<Real>>) {
    // The moment the player pressed "start" is unpredictable enough for a seed and,
    // unlike the system clock, also works in the browser.
    let seed = real_time.elapsed().as_nanos() as u64 ^ 0x0005_EED0_FA11_DA7A;
    info!("Starting run with seed {seed}");
    commands.insert_resource(GameRng(Rng::new(seed)));
    commands.insert_resource(Stats::default());
    commands.insert_resource(Xp::default());
    commands.insert_resource(RunStats { seed, ..default() });
}

fn tick_run_timer(time: Res<Time>, mut run: ResMut<RunStats>) {
    run.elapsed += time.delta_secs();
}

fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<State<GamePhase>>,
    mut next: ResMut<NextState<GamePhase>>,
) {
    if !keys.any_just_pressed([KeyCode::Escape, KeyCode::KeyP]) {
        return;
    }
    match phase.get() {
        GamePhase::Running => next.set(GamePhase::Paused),
        GamePhase::Paused => next.set(GamePhase::Running),
        GamePhase::LevelUp => {}
    }
}
