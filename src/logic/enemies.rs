//! Enemy archetypes and which ones appear at which point of a run.
//!
//! The numbers live in `assets/data/enemies.ron`, which is embedded at compile time, so
//! balancing an enemy is a data change and the file also works in the browser build.

use std::sync::OnceLock;

use serde::Deserialize;

use super::rng::Rng;

/// The kinds of enemies the dark forest throws at the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum EnemyKind {
    /// Fast and fragile, arrives in swarms.
    Goblin,
    /// Average in every way, appears after 45 seconds.
    Skeleton,
    /// Slow, huge and tough. Drops a lot of experience.
    Ogre,
    /// Keeps its distance and shoots slow bolts that have to be dodged.
    Archer,
    /// Very fast and fragile.
    Bat,
    /// Keeps its distance and casts fans of bolts.
    Warlock,
}

fn one() -> u32 {
    1
}

/// How an enemy attacks from a distance.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct RangedAttack {
    /// Closer than this and the enemy backs away.
    pub min_range: f32,
    /// Farther than this and the enemy walks closer. It only shoots within this range.
    pub max_range: f32,
    /// Seconds between two shots.
    pub cooldown: f32,
    pub projectile_speed: f32,
    pub damage: f32,
    /// Bolts per shot, fanned out symmetrically.
    #[serde(default = "one")]
    pub projectiles: u32,
    /// Angle in radians between neighbouring bolts of a fan.
    #[serde(default)]
    pub spread: f32,
}

impl RangedAttack {
    /// Movement factor for a given distance to the player:
    /// `1.0` walk towards, `-1.0` back away, `0.0` hold position.
    pub fn approach_factor(&self, distance: f32) -> f32 {
        if distance > self.max_range {
            1.0
        } else if distance < self.min_range {
            -1.0
        } else {
            0.0
        }
    }
}

/// Base values of an enemy kind before difficulty scaling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnemyArchetype {
    pub health: f32,
    pub speed: f32,
    pub radius: f32,
    /// Damage per second dealt while touching the player.
    pub contact_dps: f32,
    pub xp: u32,
    /// `None` for enemies that only fight in melee.
    pub ranged: Option<RangedAttack>,
}

/// When and how often an enemy kind is picked.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
struct SpawnRule {
    /// Difficulty time (seconds) before the kind can appear.
    from: f32,
    base: f32,
    per_second: f32,
}

#[derive(Debug, Clone, Deserialize)]
struct EnemyDef {
    kind: EnemyKind,
    health: f32,
    speed: f32,
    radius: f32,
    contact_dps: f32,
    xp: u32,
    gold: u32,
    spawn: SpawnRule,
    ranged: Option<RangedAttack>,
}

const ENEMY_DATA: &str = include_str!("../../assets/data/enemies.ron");

fn parse_definitions(source: &str) -> Result<Vec<EnemyDef>, ron::error::SpannedError> {
    ron::from_str(source)
}

/// The embedded definitions, parsed once. A broken data file is a build-time mistake, so
/// failing loudly at startup is fine (a unit test guards it).
fn definitions() -> &'static [EnemyDef] {
    static DEFINITIONS: OnceLock<Vec<EnemyDef>> = OnceLock::new();
    DEFINITIONS.get_or_init(|| {
        parse_definitions(ENEMY_DATA).expect("assets/data/enemies.ron is not valid")
    })
}

impl EnemyKind {
    pub const ALL: [EnemyKind; 6] = [
        EnemyKind::Goblin,
        EnemyKind::Skeleton,
        EnemyKind::Ogre,
        EnemyKind::Archer,
        EnemyKind::Bat,
        EnemyKind::Warlock,
    ];

    fn definition(self) -> &'static EnemyDef {
        definitions()
            .iter()
            .find(|def| def.kind == self)
            .unwrap_or_else(|| panic!("no definition for {self:?} in enemies.ron"))
    }

    pub fn archetype(self) -> EnemyArchetype {
        let def = self.definition();
        EnemyArchetype {
            health: def.health,
            speed: def.speed,
            radius: def.radius,
            contact_dps: def.contact_dps,
            xp: def.xp,
            ranged: def.ranged,
        }
    }

    /// Gold dropped when this kind dies.
    pub fn gold(self) -> u32 {
        self.definition().gold
    }

    /// Relative spawn weight at a point of the run. Zero means "not yet".
    pub fn spawn_weight(self, elapsed_secs: f32) -> f32 {
        let rule = self.definition().spawn;
        if elapsed_secs >= rule.from {
            rule.base + rule.per_second * elapsed_secs
        } else {
            0.0
        }
    }
}

/// Picks the kind of the next enemy to spawn.
pub fn pick_enemy_kind(rng: &mut Rng, elapsed_secs: f32) -> EnemyKind {
    let weights = EnemyKind::ALL.map(|k| k.spawn_weight(elapsed_secs));
    let index = rng.weighted_index(&weights).unwrap_or(0);
    EnemyKind::ALL[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_file_parses_and_covers_every_kind() {
        let defs = parse_definitions(ENEMY_DATA).expect("valid RON");
        for kind in EnemyKind::ALL {
            let count = defs.iter().filter(|def| def.kind == kind).count();
            assert_eq!(count, 1, "{kind:?} must be defined exactly once");
        }
        for def in &defs {
            assert!(
                def.health > 0.0 && def.speed > 0.0 && def.radius > 0.0,
                "{:?}",
                def.kind
            );
        }
    }

    #[test]
    fn only_goblins_at_the_start() {
        let mut rng = Rng::new(5);
        for _ in 0..500 {
            assert_eq!(pick_enemy_kind(&mut rng, 10.0), EnemyKind::Goblin);
        }
    }

    #[test]
    fn all_kinds_appear_late_in_the_run() {
        let mut rng = Rng::new(5);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..5_000 {
            seen.insert(pick_enemy_kind(&mut rng, 600.0));
        }
        assert_eq!(seen.len(), EnemyKind::ALL.len());
    }

    #[test]
    fn ranged_enemies_keep_their_distance() {
        for kind in [EnemyKind::Archer, EnemyKind::Warlock] {
            let ranged = kind.archetype().ranged.expect("shoots");
            assert_eq!(ranged.approach_factor(ranged.max_range + 1.0), 1.0);
            assert_eq!(ranged.approach_factor(ranged.min_range - 1.0), -1.0);
            let middle = (ranged.min_range + ranged.max_range) / 2.0;
            assert_eq!(ranged.approach_factor(middle), 0.0);
            assert!(ranged.min_range < ranged.max_range);
        }
    }

    #[test]
    fn only_archers_and_warlocks_shoot() {
        for kind in EnemyKind::ALL {
            let shoots = kind.archetype().ranged.is_some();
            let expected = matches!(kind, EnemyKind::Archer | EnemyKind::Warlock);
            assert_eq!(shoots, expected, "{kind:?}");
        }
    }

    #[test]
    fn warlocks_fire_fans_and_archers_single_bolts() {
        let archer = EnemyKind::Archer.archetype().ranged.unwrap();
        let warlock = EnemyKind::Warlock.archetype().ranged.unwrap();
        assert_eq!(archer.projectiles, 1);
        assert!(warlock.projectiles > 1);
        assert!(warlock.spread > 0.0);
    }

    #[test]
    fn newer_kinds_appear_after_a_while() {
        assert_eq!(EnemyKind::Archer.spawn_weight(30.0), 0.0);
        assert!(EnemyKind::Archer.spawn_weight(200.0) > 0.0);
        assert_eq!(EnemyKind::Warlock.spawn_weight(100.0), 0.0);
        assert!(EnemyKind::Warlock.spawn_weight(300.0) > 0.0);
    }

    #[test]
    fn ogres_are_tougher_than_goblins() {
        let goblin = EnemyKind::Goblin.archetype();
        let ogre = EnemyKind::Ogre.archetype();
        assert!(ogre.health > goblin.health);
        assert!(ogre.speed < goblin.speed);
        assert!(ogre.xp > goblin.xp);
    }

    #[test]
    fn bats_are_fast_and_fragile() {
        let bat = EnemyKind::Bat.archetype();
        let goblin = EnemyKind::Goblin.archetype();
        assert!(bat.speed > goblin.speed);
        assert!(bat.health < goblin.health);
    }
}
