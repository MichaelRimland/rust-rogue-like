//! Sound effects and the ambient loop.
//!
//! Gameplay code never touches the audio engine; it writes a [`PlaySound`] message and
//! this module decides whether and how loudly to play it (rate limits keep a crowd of hits
//! from turning into noise). The sounds are synthesized by `tools/make_sounds.py`.

use bevy::{
    audio::{AudioPlayer, GlobalVolume, PlaybackSettings, Volume},
    prelude::*,
};

use crate::{
    GamePhase,
    ui::{MUTED_TEXT_COLOR, label},
};

pub struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySound>()
            .insert_resource(GlobalVolume::new(Volume::Linear(DEFAULT_VOLUME)))
            .add_systems(Startup, (load_sounds, spawn_volume_label))
            .add_systems(
                Update,
                (play_sounds, volume_keys, update_volume_label, start_ambient).chain(),
            )
            .add_systems(OnEnter(GamePhase::LevelUp), level_up_sound);
    }
}

const DEFAULT_VOLUME: f32 = 0.6;
const VOLUME_STEP: f32 = 0.1;
/// Level of the ambient loop relative to the sound effects.
const AMBIENT_VOLUME: f32 = 0.35;
/// More simultaneous sounds than this are dropped.
const MAX_VOICES: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sfx {
    Shoot,
    Hit,
    Kill,
    Pickup,
    LevelUp,
    Hurt,
    Dash,
    Blink,
    Clear,
    Boss,
    Coin,
    Door,
}

impl Sfx {
    const ALL: [Sfx; 12] = [
        Sfx::Shoot,
        Sfx::Hit,
        Sfx::Kill,
        Sfx::Pickup,
        Sfx::LevelUp,
        Sfx::Hurt,
        Sfx::Dash,
        Sfx::Blink,
        Sfx::Clear,
        Sfx::Boss,
        Sfx::Coin,
        Sfx::Door,
    ];

    fn file(self) -> &'static str {
        match self {
            Sfx::Shoot => "shoot",
            Sfx::Hit => "hit",
            Sfx::Kill => "kill",
            Sfx::Pickup => "pickup",
            Sfx::LevelUp => "levelup",
            Sfx::Hurt => "hurt",
            Sfx::Dash => "dash",
            Sfx::Blink => "blink",
            Sfx::Clear => "clear",
            Sfx::Boss => "boss",
            Sfx::Coin => "coin",
            Sfx::Door => "door",
        }
    }

    /// Volume of the effect and the shortest time between two plays of it.
    fn mix(self) -> (f32, f32) {
        match self {
            Sfx::Shoot => (0.5, 0.06),
            Sfx::Hit => (0.35, 0.05),
            Sfx::Kill => (0.5, 0.04),
            Sfx::Pickup => (0.4, 0.04),
            Sfx::LevelUp => (0.8, 0.0),
            Sfx::Hurt => (0.7, 0.3),
            Sfx::Dash => (0.6, 0.1),
            Sfx::Blink => (0.6, 0.1),
            Sfx::Clear => (0.7, 0.0),
            Sfx::Boss => (0.8, 0.2),
            Sfx::Coin => (0.6, 0.05),
            Sfx::Door => (0.7, 0.0),
        }
    }
}

/// Ask the audio module to play a sound effect.
#[derive(Message, Debug, Clone, Copy)]
pub struct PlaySound(pub Sfx);

impl From<Sfx> for PlaySound {
    fn from(sfx: Sfx) -> Self {
        PlaySound(sfx)
    }
}

#[derive(Resource)]
struct SoundBank {
    effects: Vec<Handle<AudioSource>>,
    ambient: Handle<AudioSource>,
}

#[derive(Component)]
struct AmbientLoop;

#[derive(Component)]
struct VolumeLabel;

fn load_sounds(mut commands: Commands, asset_server: Res<AssetServer>) {
    let effects = Sfx::ALL
        .iter()
        .map(|sfx| asset_server.load(format!("sounds/{}.wav", sfx.file())))
        .collect();
    commands.insert_resource(SoundBank {
        effects,
        ambient: asset_server.load("sounds/ambient.wav"),
    });
}

fn level_up_sound(mut sounds: MessageWriter<PlaySound>) {
    sounds.write(PlaySound(Sfx::LevelUp));
}

fn play_sounds(
    mut commands: Commands,
    time: Res<Time<Real>>,
    bank: Res<SoundBank>,
    mut messages: MessageReader<PlaySound>,
    voices: Query<(), With<AudioPlayer>>,
    mut last_played: Local<Vec<f32>>,
) {
    if last_played.is_empty() {
        last_played.resize(Sfx::ALL.len(), f32::NEG_INFINITY);
    }
    let now = time.elapsed_secs();
    let mut active = voices.iter().count();

    for PlaySound(sfx) in messages.read() {
        let index = *sfx as usize;
        let (volume, min_interval) = sfx.mix();
        if active >= MAX_VOICES || now - last_played[index] < min_interval {
            continue;
        }
        last_played[index] = now;
        active += 1;

        // A tiny pitch wobble keeps repeated sounds from feeling mechanical.
        let wobble = 1.0 + ((now * 997.0).sin() * 0.06);
        commands.spawn((
            AudioPlayer::<AudioSource>::new(bank.effects[index].clone()),
            PlaybackSettings::DESPAWN
                .with_volume(Volume::Linear(volume))
                .with_speed(wobble),
        ));
    }
}

/// Starts the ambient loop once its file has loaded.
fn start_ambient(
    mut commands: Commands,
    bank: Res<SoundBank>,
    sources: Res<Assets<AudioSource>>,
    existing: Query<(), With<AmbientLoop>>,
) {
    if existing.is_empty() && sources.contains(&bank.ambient) {
        commands.spawn((
            AmbientLoop,
            AudioPlayer::<AudioSource>::new(bank.ambient.clone()),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(AMBIENT_VOLUME)),
        ));
    }
}

/// `-` and `+` change the master volume, `M` mutes. Already playing sounds keep their level.
fn volume_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut global: ResMut<GlobalVolume>,
    mut before_mute: Local<Option<f32>>,
) {
    let current = global.volume.to_linear();
    let set = |value: f32, global: &mut GlobalVolume| {
        global.volume = Volume::Linear(value.clamp(0.0, 1.0));
    };
    if keys.any_just_pressed([KeyCode::Minus, KeyCode::NumpadSubtract]) {
        set(current - VOLUME_STEP, &mut global);
    }
    if keys.any_just_pressed([KeyCode::Equal, KeyCode::NumpadAdd]) {
        set(current + VOLUME_STEP, &mut global);
    }
    if keys.just_pressed(KeyCode::KeyM) {
        match before_mute.take() {
            Some(previous) => set(previous, &mut global),
            None => {
                *before_mute = Some(current);
                set(0.0, &mut global);
            }
        }
    }
}

fn spawn_volume_label(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: px(8.0),
            left: px(12.0),
            ..default()
        },
        children![(VolumeLabel, label("", 13.0, MUTED_TEXT_COLOR))],
    ));
}

fn update_volume_label(global: Res<GlobalVolume>, mut text: Single<&mut Text, With<VolumeLabel>>) {
    if !global.is_changed() && !text.0.is_empty() {
        return;
    }
    let percent = (global.volume.to_linear() * 100.0).round();
    text.0 = if percent <= 0.0 {
        "Sound off  (M / + / -)".to_string()
    } else {
        format!("Volume {percent:.0}%  (- / + / M)")
    };
}
