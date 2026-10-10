//! Enemy archetypes and which ones appear at which point of a run.

use super::rng::Rng;

/// The kinds of enemies the dark forest throws at the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnemyKind {
    /// Fast and fragile, arrives in swarms.
    Goblin,
    /// Average in every way, appears after 45 seconds.
    Skeleton,
    /// Slow, huge and tough. Drops a lot of experience.
    Ogre,
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
}

impl EnemyKind {
    pub const ALL: [EnemyKind; 3] = [EnemyKind::Goblin, EnemyKind::Skeleton, EnemyKind::Ogre];

    pub fn archetype(self) -> EnemyArchetype {
        match self {
            EnemyKind::Goblin => EnemyArchetype {
                health: 16.0,
                speed: 95.0,
                radius: 10.0,
                contact_dps: 12.0,
                xp: 3,
            },
            EnemyKind::Skeleton => EnemyArchetype {
                health: 40.0,
                speed: 70.0,
                radius: 12.0,
                contact_dps: 18.0,
                xp: 5,
            },
            EnemyKind::Ogre => EnemyArchetype {
                health: 160.0,
                speed: 45.0,
                radius: 22.0,
                contact_dps: 34.0,
                xp: 18,
            },
        }
    }

    /// Relative spawn weight at a point of the run. Zero means "not yet".
    pub fn spawn_weight(self, elapsed_secs: f32) -> f32 {
        match self {
            EnemyKind::Goblin => 1.0,
            EnemyKind::Skeleton if elapsed_secs >= 45.0 => 0.6 + elapsed_secs / 300.0,
            EnemyKind::Ogre if elapsed_secs >= 120.0 => 0.15 + elapsed_secs / 1_200.0,
            _ => 0.0,
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
        for _ in 0..2_000 {
            seen.insert(pick_enemy_kind(&mut rng, 400.0));
        }
        assert_eq!(seen.len(), EnemyKind::ALL.len());
    }

    #[test]
    fn ogres_are_tougher_than_goblins() {
        let goblin = EnemyKind::Goblin.archetype();
        let ogre = EnemyKind::Ogre.archetype();
        assert!(ogre.health > goblin.health);
        assert!(ogre.speed < goblin.speed);
        assert!(ogre.xp > goblin.xp);
    }
}
