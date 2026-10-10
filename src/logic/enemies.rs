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
    /// Keeps its distance and shoots slow bolts that have to be dodged.
    Archer,
}

/// How an enemy attacks from a distance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangedAttack {
    /// Closer than this and the enemy backs away.
    pub min_range: f32,
    /// Farther than this and the enemy walks closer. It only shoots within this range.
    pub max_range: f32,
    /// Seconds between two shots.
    pub cooldown: f32,
    pub projectile_speed: f32,
    pub damage: f32,
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

impl EnemyKind {
    pub const ALL: [EnemyKind; 4] = [
        EnemyKind::Goblin,
        EnemyKind::Skeleton,
        EnemyKind::Ogre,
        EnemyKind::Archer,
    ];

    pub fn archetype(self) -> EnemyArchetype {
        match self {
            EnemyKind::Goblin => EnemyArchetype {
                health: 16.0,
                speed: 95.0,
                radius: 10.0,
                contact_dps: 12.0,
                xp: 3,
                ranged: None,
            },
            EnemyKind::Skeleton => EnemyArchetype {
                health: 40.0,
                speed: 70.0,
                radius: 12.0,
                contact_dps: 18.0,
                xp: 5,
                ranged: None,
            },
            EnemyKind::Ogre => EnemyArchetype {
                health: 160.0,
                speed: 45.0,
                radius: 22.0,
                contact_dps: 34.0,
                xp: 18,
                ranged: None,
            },
            EnemyKind::Archer => EnemyArchetype {
                health: 24.0,
                speed: 65.0,
                radius: 11.0,
                contact_dps: 6.0,
                xp: 6,
                ranged: Some(RangedAttack {
                    min_range: 230.0,
                    max_range: 380.0,
                    cooldown: 2.4,
                    projectile_speed: 230.0,
                    damage: 12.0,
                }),
            },
        }
    }

    /// Gold dropped when this kind dies.
    pub fn gold(self) -> u32 {
        match self {
            EnemyKind::Goblin => 1,
            EnemyKind::Skeleton => 2,
            EnemyKind::Archer => 3,
            EnemyKind::Ogre => 5,
        }
    }

    /// Relative spawn weight at a point of the run. Zero means "not yet".
    pub fn spawn_weight(self, elapsed_secs: f32) -> f32 {
        match self {
            EnemyKind::Goblin => 1.0,
            EnemyKind::Skeleton if elapsed_secs >= 45.0 => 0.6 + elapsed_secs / 300.0,
            EnemyKind::Ogre if elapsed_secs >= 120.0 => 0.15 + elapsed_secs / 1_200.0,
            EnemyKind::Archer if elapsed_secs >= 75.0 => 0.3 + elapsed_secs / 500.0,
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
    fn archers_keep_their_distance() {
        let ranged = EnemyKind::Archer.archetype().ranged.expect("archers shoot");
        assert_eq!(ranged.approach_factor(ranged.max_range + 1.0), 1.0);
        assert_eq!(ranged.approach_factor(ranged.min_range - 1.0), -1.0);
        let middle = (ranged.min_range + ranged.max_range) / 2.0;
        assert_eq!(ranged.approach_factor(middle), 0.0);
        assert!(ranged.min_range < ranged.max_range);
    }

    #[test]
    fn only_archers_shoot() {
        for kind in EnemyKind::ALL {
            let shoots = kind.archetype().ranged.is_some();
            assert_eq!(shoots, kind == EnemyKind::Archer, "{kind:?}");
        }
    }

    #[test]
    fn archers_appear_after_a_while() {
        assert_eq!(EnemyKind::Archer.spawn_weight(30.0), 0.0);
        assert!(EnemyKind::Archer.spawn_weight(200.0) > 0.0);
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
