//! Two-frame walk cycles for the character sprites.

use bevy::prelude::*;

use crate::GameSet;

pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, animate_walkers.in_set(GameSet::Movement));
    }
}

/// Seconds each walk frame is shown.
const FRAME_SECS: f32 = 0.2;

/// Alternates between two sprite images while `playing`.
#[derive(Component)]
pub struct WalkAnimation {
    frames: [Handle<Image>; 2],
    /// Offsets the cycle so a crowd does not step in unison.
    phase: u32,
    pub playing: bool,
}

impl WalkAnimation {
    pub fn new(frames: [Handle<Image>; 2], phase: u32) -> Self {
        Self {
            frames,
            phase,
            playing: true,
        }
    }
}

fn animate_walkers(time: Res<Time>, mut walkers: Query<(&mut Sprite, &WalkAnimation)>) {
    let tick = (time.elapsed_secs() / FRAME_SECS) as u32;
    for (mut sprite, walk) in &mut walkers {
        let index = if walk.playing {
            ((tick + walk.phase) % 2) as usize
        } else {
            0
        };
        // Compare first so unchanged sprites are not flagged as modified.
        if sprite.image != walk.frames[index] {
            sprite.image = walk.frames[index].clone();
        }
    }
}
