//! Persistent meta-progression: records, soul shards and shrine upgrades.
//!
//! On native builds the data is written to `savegame.ron` in the working directory. The
//! browser build keeps it in memory only (no file system), so progress lasts for the tab.

use bevy::prelude::*;

use crate::{
    AppState,
    dungeon::GameMode,
    logic::meta::{MetaProgress, RunSummary},
    run::{RunStats, Xp},
    ui::{ACCENT_COLOR, MUTED_TEXT_COLOR, label},
};

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Meta(load()))
            .add_systems(OnEnter(AppState::GameOver), record_run);
    }
}

/// Add this resource to run without touching the save file (tests, benchmarks).
#[derive(Resource)]
pub struct SaveDisabled;

#[derive(Resource, Default)]
pub struct Meta(pub MetaProgress);

#[cfg(not(target_arch = "wasm32"))]
const SAVE_FILE: &str = "savegame.ron";

#[cfg(not(target_arch = "wasm32"))]
fn load() -> MetaProgress {
    std::fs::read_to_string(SAVE_FILE)
        .ok()
        .and_then(|text| ron::from_str(&text).ok())
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
fn load() -> MetaProgress {
    MetaProgress::default()
}

/// Writes the meta-progression to disk (native builds only).
pub fn persist(meta: &MetaProgress, disabled: bool) {
    #[cfg(not(target_arch = "wasm32"))]
    if !disabled {
        match ron::ser::to_string_pretty(meta, ron::ser::PrettyConfig::default()) {
            Ok(text) => {
                if let Err(error) = std::fs::write(SAVE_FILE, text) {
                    warn!("could not write {SAVE_FILE}: {error}");
                }
            }
            Err(error) => warn!("could not serialize the save game: {error}"),
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (meta, disabled);
}

/// Banks the shards of a finished run and shows what it earned.
fn record_run(
    mut commands: Commands,
    mut meta: ResMut<Meta>,
    run: Res<RunStats>,
    xp: Res<Xp>,
    mode: Res<GameMode>,
    disabled: Option<Res<SaveDisabled>>,
) {
    let summary = RunSummary {
        dungeon: *mode == GameMode::Dungeon,
        floor: run.floor,
        kills: run.kills,
        level: xp.level,
        seconds: run.elapsed,
        victory: run.victory,
    };
    let shards = meta.0.record_run(&summary);
    persist(&meta.0, disabled.is_some());

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: px(40.0),
            width: percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(4.0),
            ..default()
        },
        DespawnOnExit(AppState::GameOver),
        children![
            label(format!("+{shards} soul shards"), 24.0, ACCENT_COLOR),
            label(
                format!(
                    "{} banked  -  spend them at the shrine in the main menu",
                    meta.0.shards
                ),
                16.0,
                MUTED_TEXT_COLOR
            ),
        ],
    ));
}
