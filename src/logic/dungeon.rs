//! Dungeon structure: which rounds a floor consists of, how hard each one is and which
//! bosses guard the end of a floor.
//!
//! A floor is a short chain of rooms. After the first round the player chooses between
//! two doors per step; the last room of every floor is a boss fight. The final floor ends
//! with the end boss.

use super::{enemies::EnemyKind, rng::Rng};

/// Number of floors before the run is won. The last floor's boss is the end boss.
pub const FLOORS: u32 = 3;
/// Choice layers between the entry round and the boss.
const MIDDLE_LAYERS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoomKind {
    /// A round: enemies stream in until enough of them are defeated.
    Arena,
    /// A round against tougher enemies; rewards a free upgrade.
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

// ---------------------------------------------------------------------------
// Biomes
// ---------------------------------------------------------------------------

/// The look of a floor: ground, decoration and walls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Biome {
    /// Mossy forest floor with bushes and mushrooms.
    #[default]
    Forest,
    /// Cold flagstones with skulls, pillars and candles.
    Crypt,
    /// Scorched obsidian with lava veins and embers.
    Inferno,
}

impl Biome {
    pub const ALL: [Biome; 3] = [Biome::Forest, Biome::Crypt, Biome::Inferno];

    pub fn name(self) -> &'static str {
        match self {
            Biome::Forest => "Cursed Forest",
            Biome::Crypt => "The Crypt",
            Biome::Inferno => "The Inferno",
        }
    }
}

/// Every floor gets its own look, so a run travels through different places.
pub fn biome_for_floor(floor: u32) -> Biome {
    match floor {
        0 | 1 => Biome::Crypt,
        2 => Biome::Forest,
        _ => Biome::Inferno,
    }
}

// ---------------------------------------------------------------------------
// Rounds
// ---------------------------------------------------------------------------

/// Enemies that have to be defeated to finish a combat round. Elite rounds count elites.
pub fn round_quota(kind: RoomKind, floor: u32, layer: usize) -> u32 {
    match kind {
        RoomKind::Arena => 24 + 6 * floor + 4 * layer as u32,
        RoomKind::Elite => 4 + floor,
        _ => 0,
    }
}

/// The very first room of a run: the wizard has one weak bolt and no upgrades yet, so this
/// round is a warm-up with smaller and slower waves (autoplay `--mortal` died here in ~30 s).
fn is_warmup(floor: u32, layer: usize) -> bool {
    floor <= 1 && layer == 0
}

/// Seconds between two batches of enemies entering the arena during a round.
pub fn round_spawn_interval(kind: RoomKind, floor: u32, layer: usize) -> f32 {
    match kind {
        RoomKind::Elite => 3.5,
        _ => {
            let base = (2.2 - 0.12 * layer as f32 - 0.2 * floor as f32).clamp(0.8, 2.2);
            if is_warmup(floor, layer) {
                base + WARMUP_EXTRA_INTERVAL
            } else {
                base
            }
        }
    }
}

/// Extra seconds between waves in the warm-up room.
const WARMUP_EXTRA_INTERVAL: f32 = 0.8;

/// Enemies per batch.
pub fn round_batch_size(kind: RoomKind, floor: u32, layer: usize) -> u32 {
    match kind {
        RoomKind::Elite => 1,
        _ if is_warmup(floor, layer) => 2,
        _ => 2 + floor + layer as u32 / 2,
    }
}

/// Most enemies alive at the same time.
pub fn round_alive_cap(kind: RoomKind, floor: u32) -> usize {
    match kind {
        RoomKind::Elite => 3,
        _ => 40 + 15 * floor as usize,
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

// ---------------------------------------------------------------------------
// Bosses
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BossKind {
    /// Heavy brute that summons goblins and charges when hurt.
    OgreWarlord,
    /// Fast and erratic: charges at the player and calls swarms of bats.
    BatMatron,
    /// Slow giant with bolt rings and aimed volleys.
    BoneColossus,
    /// Caster with dense volleys and a spiral of bolts once wounded.
    Archwarlock,
    /// The end boss: every attack, three phases.
    HollowKing,
}

/// How a boss looks and attacks. Every boss has its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BossPattern {
    pub name: &'static str,
    /// Sprite and base stats come from this enemy kind.
    pub kind: EnemyKind,
    /// Collision radius in world units.
    pub radius: f32,
    /// Sprite scale relative to a normal enemy.
    pub visual: f32,
    pub tint: [f32; 3],
    /// Bolts in the ring fired every few seconds; every phase adds two.
    pub ring: u32,
    /// Bolts in an aimed fan fired together with the ring (0 means none).
    pub aimed: u32,
    /// Arms of the bolt spiral, active from the second phase (0 means none).
    pub spiral_arms: u32,
    /// The phase from which the boss charges at the player.
    pub charge_from_phase: Option<u32>,
    /// What the boss calls to its side.
    pub summon: EnemyKind,
    /// Multiplies the floor's boss health.
    pub health_factor: f32,
    /// The end boss gets its own sprite.
    pub is_end_boss: bool,
}

pub fn boss_pattern(kind: BossKind) -> BossPattern {
    match kind {
        BossKind::OgreWarlord => BossPattern {
            name: "Ogre Warlord",
            kind: EnemyKind::Ogre,
            radius: 46.0,
            visual: 3.2,
            tint: [1.0, 0.62, 0.72],
            ring: 10,
            aimed: 0,
            spiral_arms: 0,
            charge_from_phase: Some(1),
            summon: EnemyKind::Goblin,
            health_factor: 1.0,
            is_end_boss: false,
        },
        BossKind::BatMatron => BossPattern {
            name: "Bat Matron",
            kind: EnemyKind::Bat,
            radius: 34.0,
            visual: 4.5,
            tint: [0.85, 0.7, 1.0],
            ring: 6,
            aimed: 0,
            spiral_arms: 0,
            charge_from_phase: Some(0),
            summon: EnemyKind::Bat,
            health_factor: 0.8,
            is_end_boss: false,
        },
        BossKind::BoneColossus => BossPattern {
            name: "Bone Colossus",
            kind: EnemyKind::Skeleton,
            radius: 46.0,
            visual: 4.2,
            tint: [0.75, 1.0, 0.95],
            ring: 8,
            aimed: 3,
            spiral_arms: 0,
            charge_from_phase: None,
            summon: EnemyKind::Skeleton,
            health_factor: 1.1,
            is_end_boss: false,
        },
        BossKind::Archwarlock => BossPattern {
            name: "Archwarlock",
            kind: EnemyKind::Warlock,
            radius: 44.0,
            visual: 4.2,
            tint: [0.9, 0.8, 1.0],
            ring: 10,
            aimed: 5,
            spiral_arms: 2,
            charge_from_phase: None,
            summon: EnemyKind::Archer,
            health_factor: 1.0,
            is_end_boss: false,
        },
        BossKind::HollowKing => BossPattern {
            name: "The Hollow King",
            kind: EnemyKind::Warlock,
            radius: 48.0,
            visual: 4.6,
            tint: [1.0, 1.0, 1.0],
            ring: 12,
            aimed: 5,
            spiral_arms: 4,
            charge_from_phase: Some(2),
            summon: EnemyKind::Skeleton,
            health_factor: 1.9,
            is_end_boss: true,
        },
    }
}

/// The bosses a floor can end with. Floors before the last one offer a choice, so two
/// runs rarely meet the same sequence; the last floor always ends with the end boss.
pub fn floor_boss_pool(floor: u32) -> &'static [BossKind] {
    match floor {
        0 | 1 => &[BossKind::OgreWarlord, BossKind::BatMatron],
        2 => &[BossKind::BoneColossus, BossKind::Archwarlock],
        _ => &[BossKind::HollowKing],
    }
}

pub fn pick_boss(rng: &mut Rng, floor: u32) -> BossKind {
    let pool = floor_boss_pool(floor);
    pool[rng.below(pool.len())]
}

/// Health of a floor's boss.
pub fn boss_health(floor: u32, kind: BossKind) -> f32 {
    let base = 700.0 * (1.0 + 0.9 * floor.saturating_sub(1) as f32);
    base * boss_pattern(kind).health_factor
}

/// Boss phase from the remaining health: `0` fresh, `1` below two thirds, `2` below one third.
pub fn boss_phase(health_fraction: f32) -> u32 {
    if health_fraction < 1.0 / 3.0 {
        2
    } else if health_fraction < 2.0 / 3.0 {
        1
    } else {
        0
    }
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
    fn only_combat_rooms_have_a_quota() {
        for kind in [RoomKind::Arena, RoomKind::Elite] {
            assert!(kind.is_combat());
            assert!(round_quota(kind, 1, 1) > 0);
            assert!(round_batch_size(kind, 1, 1) > 0);
            assert!(round_alive_cap(kind, 1) > 0);
        }
        for kind in [RoomKind::Treasure, RoomKind::Shop, RoomKind::Rest] {
            assert!(!kind.is_combat());
            assert_eq!(round_quota(kind, 1, 1), 0);
        }
    }

    #[test]
    fn rounds_get_longer_and_denser_deeper_in() {
        assert!(round_quota(RoomKind::Arena, 2, 3) > round_quota(RoomKind::Arena, 1, 1));
        assert!(
            round_spawn_interval(RoomKind::Arena, 3, 4)
                < round_spawn_interval(RoomKind::Arena, 1, 1)
        );
        assert!(round_spawn_interval(RoomKind::Arena, 9, 9) >= 0.8);
        assert!(round_batch_size(RoomKind::Arena, 3, 4) > round_batch_size(RoomKind::Arena, 1, 1));
    }

    #[test]
    fn the_first_room_is_a_gentle_warmup() {
        let rate = |floor, layer| {
            round_batch_size(RoomKind::Arena, floor, layer) as f32
                / round_spawn_interval(RoomKind::Arena, floor, layer)
        };
        assert!(rate(1, 0) < 0.8, "fewer than one enemy per second");
        assert!(rate(1, 0) < rate(1, 1), "the second room is denser again");
    }

    #[test]
    fn difficulty_grows_with_layer_and_floor() {
        assert!(difficulty_time(1, 3) > difficulty_time(1, 0));
        assert!(difficulty_time(2, 0) > difficulty_time(1, 4));
        assert!(enemy_health_multiplier(3, 2) > enemy_health_multiplier(1, 0));
    }

    #[test]
    fn the_last_floor_always_ends_with_the_end_boss() {
        assert_eq!(floor_boss_pool(FLOORS), &[BossKind::HollowKing]);
        assert!(boss_pattern(BossKind::HollowKing).is_end_boss);
        for floor in 1..FLOORS {
            assert!(
                floor_boss_pool(floor).len() >= 2,
                "floor {floor} offers a choice"
            );
            for &kind in floor_boss_pool(floor) {
                assert!(!boss_pattern(kind).is_end_boss);
            }
        }
    }

    #[test]
    fn picked_bosses_belong_to_the_floor_and_vary() {
        let mut rng = Rng::new(5);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            let boss = pick_boss(&mut rng, 1);
            assert!(floor_boss_pool(1).contains(&boss));
            seen.insert(boss);
        }
        assert_eq!(seen.len(), floor_boss_pool(1).len());
    }

    #[test]
    fn bosses_are_distinct_and_the_end_boss_is_the_toughest() {
        let all = [
            BossKind::OgreWarlord,
            BossKind::BatMatron,
            BossKind::BoneColossus,
            BossKind::Archwarlock,
            BossKind::HollowKing,
        ];
        for (i, &a) in all.iter().enumerate() {
            for &b in &all[i + 1..] {
                assert_ne!(boss_pattern(a).name, boss_pattern(b).name);
            }
        }
        let king = boss_health(FLOORS, BossKind::HollowKing);
        for &kind in &all[..4] {
            assert!(king > boss_health(FLOORS, kind));
        }
        assert!(boss_health(2, BossKind::OgreWarlord) > boss_health(1, BossKind::OgreWarlord));
    }

    #[test]
    fn every_floor_has_its_own_biome() {
        let biomes: Vec<Biome> = (1..=FLOORS).map(biome_for_floor).collect();
        for (i, a) in biomes.iter().enumerate() {
            for b in &biomes[i + 1..] {
                assert_ne!(a, b);
            }
        }
        assert_eq!(biome_for_floor(1), Biome::Crypt);
        assert_eq!(biome_for_floor(2), Biome::Forest);
        assert_eq!(Biome::default(), Biome::Forest);
        assert_eq!(Biome::ALL.len(), 3);
    }

    #[test]
    fn boss_phases_follow_the_remaining_health() {
        assert_eq!(boss_phase(1.0), 0);
        assert_eq!(boss_phase(0.7), 0);
        assert_eq!(boss_phase(0.5), 1);
        assert_eq!(boss_phase(0.2), 2);
        assert_eq!(boss_phase(0.0), 2);
    }
}
