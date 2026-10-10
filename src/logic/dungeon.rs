//! Dungeon structure: which rooms a floor consists of and how hard each one is.
//!
//! A floor is a short chain of rooms. After the first room the player chooses between two
//! doors per step; the last room of every floor is the boss.

use super::{enemies::EnemyKind, rng::Rng};

/// Number of floors before the run is won.
pub const FLOORS: u32 = 3;
/// Choice layers between the entry room and the boss.
const MIDDLE_LAYERS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoomKind {
    /// Several waves of enemies.
    Arena,
    /// One wave of tougher enemies; rewards a free upgrade.
    Elite,
    /// A chest with a free upgrade.
    Treasure,
    /// Spend gold on healing and upgrades.
    Shop,
    /// Heals a part of the health.
    Rest,
    Boss,
}

impl RoomKind {
    pub fn name(self) -> &'static str {
        match self {
            RoomKind::Arena => "Arena",
            RoomKind::Elite => "Elite",
            RoomKind::Treasure => "Treasure",
            RoomKind::Shop => "Shop",
            RoomKind::Rest => "Rest",
            RoomKind::Boss => "Boss",
        }
    }

    /// Rooms where enemies have to be defeated before the doors open.
    pub fn is_combat(self) -> bool {
        matches!(self, RoomKind::Arena | RoomKind::Elite | RoomKind::Boss)
    }

    /// Relative chance to be offered behind a door (the boss is never rolled).
    fn weight(self) -> f32 {
        match self {
            RoomKind::Arena => 5.0,
            RoomKind::Elite => 2.0,
            RoomKind::Treasure => 1.5,
            RoomKind::Shop => 1.5,
            RoomKind::Rest => 1.0,
            RoomKind::Boss => 0.0,
        }
    }

    const ROLLABLE: [RoomKind; 5] = [
        RoomKind::Arena,
        RoomKind::Elite,
        RoomKind::Treasure,
        RoomKind::Shop,
        RoomKind::Rest,
    ];
}

/// The rooms of one floor. `layers[i]` holds the rooms the player can choose from when
/// entering step `i`: one room for the entry and the boss, two rooms in between.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FloorPlan {
    pub layers: Vec<Vec<RoomKind>>,
}

impl FloorPlan {
    pub fn generate(rng: &mut Rng) -> Self {
        let mut layers = vec![vec![RoomKind::Arena]];
        for step in 0..MIDDLE_LAYERS {
            let first = Self::roll_room(rng, step == 0, &[]);
            let second = Self::roll_room(rng, step == 0, &[first]);
            let mut layer = vec![first, second];
            // The last step before the boss always offers a place to recover.
            if step == MIDDLE_LAYERS - 1 && !layer.contains(&RoomKind::Rest) {
                layer[1] = RoomKind::Rest;
            }
            layers.push(layer);
        }
        layers.push(vec![RoomKind::Boss]);
        Self { layers }
    }

    /// Number of rooms the player passes through on one floor.
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    fn roll_room(rng: &mut Rng, early: bool, exclude: &[RoomKind]) -> RoomKind {
        let weights: Vec<f32> = RoomKind::ROLLABLE
            .iter()
            .map(|&kind| {
                if exclude.contains(&kind) || (early && kind == RoomKind::Elite) {
                    0.0
                } else {
                    kind.weight()
                }
            })
            .collect();
        let index = rng.weighted_index(&weights).unwrap_or(0);
        RoomKind::ROLLABLE[index]
    }
}

/// Number of enemy waves in a combat room.
pub fn wave_count(kind: RoomKind, floor: u32, layer: usize) -> u32 {
    match kind {
        RoomKind::Arena => 2 + (layer as u32 + floor) / 3,
        RoomKind::Elite => 1,
        RoomKind::Boss => 1,
        _ => 0,
    }
}

/// Enemies in a wave. The boss room counts as a single enemy.
pub fn wave_size(kind: RoomKind, floor: u32, layer: usize, wave: u32) -> u32 {
    match kind {
        RoomKind::Arena => 4 + 2 * floor + layer as u32 + 2 * wave,
        RoomKind::Elite => 2 + floor,
        RoomKind::Boss => 1,
        _ => 0,
    }
}

/// A pseudo "seconds survived" that feeds the enemy mix and health curves of the endless
/// mode, so the dungeon reuses them: later rooms and floors bring tougher monsters.
pub fn difficulty_time(floor: u32, layer: usize) -> f32 {
    floor.saturating_sub(1) as f32 * 240.0 + layer as f32 * 50.0
}

/// Health multiplier for regular enemies. Gentler than in endless mode, because the
/// player has fewer chances to find upgrades.
pub fn enemy_health_multiplier(floor: u32, layer: usize) -> f32 {
    1.0 + difficulty_time(floor, layer) / 90.0
}

/// How a boss looks and attacks. Every floor has its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BossPattern {
    pub name: &'static str,
    /// Sprite and base stats come from this enemy kind.
    pub kind: EnemyKind,
    /// Sprite scale relative to a normal enemy.
    pub visual: f32,
    pub tint: [f32; 3],
    /// Bolts in the ring fired every few seconds, and when the boss is enraged.
    pub ring: u32,
    pub ring_enraged: u32,
    /// Bolts in an aimed fan fired together with the ring (0 means none).
    pub aimed: u32,
    /// What the boss calls to its side.
    pub summon: EnemyKind,
}

pub fn boss_pattern(floor: u32) -> BossPattern {
    match floor {
        0 | 1 => BossPattern {
            name: "Ogre Warlord",
            kind: EnemyKind::Ogre,
            visual: 3.0,
            tint: [1.0, 0.62, 0.72],
            ring: 10,
            ring_enraged: 16,
            aimed: 0,
            summon: EnemyKind::Goblin,
        },
        2 => BossPattern {
            name: "Bone Colossus",
            kind: EnemyKind::Skeleton,
            visual: 5.0,
            tint: [0.75, 1.0, 0.95],
            ring: 8,
            ring_enraged: 12,
            aimed: 3,
            summon: EnemyKind::Skeleton,
        },
        _ => BossPattern {
            name: "Archwarlock",
            kind: EnemyKind::Warlock,
            visual: 5.0,
            tint: [1.0, 0.8, 1.0],
            ring: 12,
            ring_enraged: 18,
            aimed: 5,
            summon: EnemyKind::Bat,
        },
    }
}

/// Health of the floor's boss.
pub fn boss_health(floor: u32) -> f32 {
    700.0 * (1.0 + 0.9 * floor.saturating_sub(1) as f32)
}

/// Gold awarded for clearing a room.
pub fn clear_gold(kind: RoomKind, floor: u32) -> u32 {
    let base = match kind {
        RoomKind::Arena => 15,
        RoomKind::Elite => 35,
        RoomKind::Boss => 80,
        RoomKind::Treasure => 25,
        _ => 0,
    };
    base + 5 * floor.saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_floor_starts_with_an_arena_and_ends_with_the_boss() {
        let mut rng = Rng::new(3);
        for _ in 0..200 {
            let plan = FloorPlan::generate(&mut rng);
            assert_eq!(plan.layers.first().unwrap(), &vec![RoomKind::Arena]);
            assert_eq!(plan.layers.last().unwrap(), &vec![RoomKind::Boss]);
            assert_eq!(plan.len(), MIDDLE_LAYERS + 2);
        }
    }

    #[test]
    fn middle_layers_offer_two_different_non_boss_rooms() {
        let mut rng = Rng::new(9);
        for _ in 0..200 {
            let plan = FloorPlan::generate(&mut rng);
            for layer in &plan.layers[1..plan.len() - 1] {
                assert_eq!(layer.len(), 2);
                assert_ne!(layer[0], layer[1]);
                assert!(!layer.contains(&RoomKind::Boss));
            }
        }
    }

    #[test]
    fn no_elite_right_after_the_entry_and_a_rest_before_the_boss() {
        let mut rng = Rng::new(21);
        for _ in 0..300 {
            let plan = FloorPlan::generate(&mut rng);
            assert!(!plan.layers[1].contains(&RoomKind::Elite));
            let before_boss = &plan.layers[plan.len() - 2];
            assert!(before_boss.contains(&RoomKind::Rest));
        }
    }

    #[test]
    fn plans_are_reproducible_from_the_seed() {
        let a = FloorPlan::generate(&mut Rng::new(77));
        let b = FloorPlan::generate(&mut Rng::new(77));
        assert_eq!(a, b);
    }

    #[test]
    fn combat_rooms_have_waves_and_peaceful_rooms_do_not() {
        for kind in [RoomKind::Arena, RoomKind::Elite, RoomKind::Boss] {
            assert!(kind.is_combat());
            assert!(wave_count(kind, 1, 1) > 0);
            assert!(wave_size(kind, 1, 1, 0) > 0);
        }
        for kind in [RoomKind::Treasure, RoomKind::Shop, RoomKind::Rest] {
            assert!(!kind.is_combat());
            assert_eq!(wave_count(kind, 1, 1), 0);
        }
    }

    #[test]
    fn every_floor_has_a_different_and_growing_boss() {
        let patterns: Vec<_> = (1..=FLOORS).map(boss_pattern).collect();
        for (i, a) in patterns.iter().enumerate() {
            for b in &patterns[i + 1..] {
                assert_ne!(a.name, b.name);
            }
        }
        for pair in patterns.windows(2) {
            assert!(pair[1].ring >= pair[0].ring - 4);
            assert!(pair[1].aimed >= pair[0].aimed);
        }
        for pattern in &patterns {
            assert!(pattern.ring_enraged > pattern.ring);
        }
    }

    #[test]
    fn difficulty_grows_with_layer_and_floor() {
        assert!(difficulty_time(1, 3) > difficulty_time(1, 0));
        assert!(difficulty_time(2, 0) > difficulty_time(1, 4));
        assert!(enemy_health_multiplier(3, 2) > enemy_health_multiplier(1, 0));
        assert!(boss_health(2) > boss_health(1));
        assert!(wave_size(RoomKind::Arena, 2, 3, 1) > wave_size(RoomKind::Arena, 1, 1, 0));
    }
}
