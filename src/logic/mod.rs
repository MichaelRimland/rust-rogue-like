//! Engine-independent game logic.
//!
//! Everything in this module is plain Rust without any Bevy dependency. That keeps
//! the core rules (progression, upgrades, difficulty, spatial queries, RNG) fast to
//! unit test and easy to reason about. The Bevy plugins in the rest of the crate are
//! thin adapters that feed ECS data into these functions.

pub mod dungeon;
pub mod enemies;
pub mod progression;
pub mod rng;
pub mod skills;
pub mod spatial;
pub mod stats;
