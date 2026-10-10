//! Player stats, weapons and the upgrades offered on level-up.

use super::rng::Rng;

/// Arcane Missile: the starting weapon. Fires bolts towards the mouse cursor.
#[derive(Debug, Clone, PartialEq)]
pub struct MissileStats {
    pub damage: f32,
    pub cooldown: f32,
    pub speed: f32,
    /// Bolts per volley.
    pub count: u32,
    /// How many additional enemies a bolt passes through.
    pub pierce: u32,
    /// Fork support: 0 means the bolts do not fork.
    pub fork_level: u32,
    /// Chain support: how often a bolt jumps to a new enemy after a hit.
    pub chain_level: u32,
    /// Spell Echo support: how often each volley is repeated.
    pub echo_level: u32,
    /// Impact Burst support: 0 means hits do not explode.
    pub burst_level: u32,
    /// Searing Touch support: 0 means hits do not set enemies on fire.
    pub burn_level: u32,
    /// Prism Bolt evolution (Fork + Chain at max level).
    pub prism: bool,
    /// Arcane Tempest evolution (Spell Echo at max level and a big volley).
    pub tempest: bool,
}

impl MissileStats {
    pub const MAX_FORK_LEVEL: u32 = 3;
    /// Damage of a forked bolt relative to the bolt it split from.
    pub const FORK_DAMAGE_FACTOR: f32 = 0.6;

    pub const MAX_CHAIN_LEVEL: u32 = 3;
    pub const MAX_ECHO_LEVEL: u32 = 3;
    pub const MAX_BURST_LEVEL: u32 = 3;
    pub const MAX_BURN_LEVEL: u32 = 3;
    /// Damage of echoed volleys relative to the original volley.
    pub const ECHO_DAMAGE_FACTOR: f32 = 0.75;
    /// Damage of the explosion relative to the bolt that caused it.
    pub const BURST_DAMAGE_FACTOR: f32 = 0.5;

    /// How often a bolt may jump to a new enemy after hitting one.
    pub fn chain_count(&self) -> u32 {
        self.chain_level
    }

    /// How many extra times each volley is repeated.
    pub fn echo_count(&self) -> u32 {
        self.echo_level + u32::from(self.tempest)
    }

    /// Damage of echoed volleys relative to the original volley.
    pub fn echo_damage_factor(&self) -> f32 {
        if self.tempest {
            1.0
        } else {
            Self::ECHO_DAMAGE_FACTOR
        }
    }

    /// Damage of a forked bolt relative to the bolt it split from.
    pub fn fork_damage_factor(&self) -> f32 {
        if self.prism {
            0.9
        } else {
            Self::FORK_DAMAGE_FACTOR
        }
    }

    /// How many times a forked bolt may jump to a new enemy.
    pub fn fork_chains(&self) -> u32 {
        u32::from(self.prism)
    }

    /// Explosion radius on hit, `0.0` without Impact Burst.
    pub fn burst_radius(&self) -> f32 {
        if self.burst_level == 0 {
            0.0
        } else {
            30.0 + 20.0 * self.burst_level as f32
        }
    }

    /// Burn damage per second, `0.0` without Searing Touch.
    pub fn burn_dps(&self) -> f32 {
        self.damage * 0.25 * self.burn_level as f32
    }

    /// Number of bolts a missile splits into on its first hit, `0` without Fork.
    pub fn fork_count(&self) -> u32 {
        if self.fork_level == 0 {
            0
        } else {
            self.fork_level + 1
        }
    }
}

/// Dash: a short invulnerable burst. Level 0 means it is not unlocked yet.
#[derive(Debug, Clone, PartialEq)]
pub struct DashStats {
    pub level: u32,
}

impl DashStats {
    pub const MAX_LEVEL: u32 = 5;
    /// Dash speed in world units per second.
    pub const SPEED: f32 = 900.0;

    pub fn distance(&self) -> f32 {
        120.0 + 25.0 * self.level as f32
    }

    /// How long the burst lasts, derived from distance and speed.
    pub fn duration(&self) -> f32 {
        self.distance() / Self::SPEED
    }

    pub fn cooldown(&self) -> f32 {
        (2.5 - 0.3 * self.level as f32).max(1.0)
    }
}

/// Blink: teleports towards the cursor. Level 0 means it is not unlocked yet.
#[derive(Debug, Clone, PartialEq)]
pub struct BlinkStats {
    pub level: u32,
}

impl BlinkStats {
    pub const MAX_LEVEL: u32 = 5;

    pub fn range(&self) -> f32 {
        180.0 + 40.0 * self.level as f32
    }

    pub fn cooldown(&self) -> f32 {
        (6.8 - 0.8 * self.level as f32).max(2.8)
    }
}

/// Holy Aura: damages every enemy inside a circle around the player.
#[derive(Debug, Clone, PartialEq)]
pub struct AuraStats {
    /// 0 means the weapon is not unlocked yet.
    pub level: u32,
    /// Set by the Sanctum evolution (Holy Aura + Orbiting Runes at max level).
    pub evolved: bool,
}

impl AuraStats {
    pub const MAX_LEVEL: u32 = 5;
    /// Seconds between two damage ticks.
    pub const TICK: f32 = 0.5;

    pub fn radius(&self) -> f32 {
        let base = 60.0 + 15.0 * self.level as f32;
        if self.evolved { base * 1.4 } else { base }
    }

    pub fn damage_per_tick(&self) -> f32 {
        let base = 3.0 + 2.5 * self.level as f32;
        if self.evolved { base * 2.0 } else { base }
    }
}

/// Orbiting Runes: stones that circle the player and hit what they touch.
#[derive(Debug, Clone, PartialEq)]
pub struct RuneStats {
    /// 0 means the weapon is not unlocked yet.
    pub level: u32,
    /// Set by the Sanctum evolution (Holy Aura + Orbiting Runes at max level).
    pub evolved: bool,
}

impl RuneStats {
    pub const MAX_LEVEL: u32 = 5;
    /// Seconds between two damage ticks.
    pub const TICK: f32 = 0.25;
    pub const HIT_RADIUS: f32 = 12.0;

    pub fn count(&self) -> u32 {
        match (self.level, self.evolved) {
            (0, _) => 0,
            (level, false) => level + 1,
            (level, true) => level + 3,
        }
    }

    pub fn orbit_radius(&self) -> f32 {
        75.0 + 5.0 * self.level as f32
    }

    /// Angular speed in radians per second.
    pub fn angular_speed(&self) -> f32 {
        2.5 + 0.3 * self.level as f32
    }

    pub fn damage_per_tick(&self) -> f32 {
        let base = 5.0 + 3.0 * self.level as f32;
        if self.evolved { base * 1.5 } else { base }
    }
}

/// Everything upgrades can modify.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerStats {
    pub max_health: f32,
    pub move_speed: f32,
    pub pickup_radius: f32,
    pub regen_per_sec: f32,
    pub missile: MissileStats,
    pub aura: AuraStats,
    pub runes: RuneStats,
    pub dash: DashStats,
    pub blink: BlinkStats,
}

impl Default for PlayerStats {
    fn default() -> Self {
        Self {
            max_health: 100.0,
            move_speed: 160.0,
            pickup_radius: 70.0,
            regen_per_sec: 0.0,
            missile: MissileStats {
                damage: 10.0,
                cooldown: 0.9,
                speed: 420.0,
                count: 1,
                pierce: 0,
                fork_level: 0,
                chain_level: 0,
                echo_level: 0,
                burst_level: 0,
                burn_level: 0,
                prism: false,
                tempest: false,
            },
            aura: AuraStats {
                level: 0,
                evolved: false,
            },
            runes: RuneStats {
                level: 0,
                evolved: false,
            },
            dash: DashStats { level: 0 },
            blink: BlinkStats { level: 0 },
        }
    }
}

/// An upgrade the player can choose on level-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Upgrade {
    MissileDamage,
    MissileRate,
    MissileCount,
    MissilePierce,
    MissileFork,
    MissileChain,
    MissileSpeed,
    MissileEcho,
    MissileBurst,
    MissileBurn,
    HolyAura,
    OrbitingRunes,
    MaxHealth,
    MoveSpeed,
    PickupRadius,
    Regeneration,
    Dash,
    Blink,
    /// Evolutions: only offered once their ingredients are maxed out.
    EvolveSanctum,
    EvolvePrism,
    EvolveTempest,
}

const MIN_MISSILE_COOLDOWN: f32 = 0.25;
const MAX_MISSILE_COUNT: u32 = 6;
const MAX_MISSILE_PIERCE: u32 = 4;
const MAX_MOVE_SPEED: f32 = 260.0;
const MAX_MISSILE_SPEED: f32 = 900.0;
/// Volley size needed for the Arcane Tempest evolution.
const EVOLVE_TEMPEST_VOLLEY: u32 = 4;
const MAX_PICKUP_RADIUS: f32 = 250.0;

impl Upgrade {
    pub const ALL: [Upgrade; 21] = [
        Upgrade::MissileDamage,
        Upgrade::MissileRate,
        Upgrade::MissileCount,
        Upgrade::MissilePierce,
        Upgrade::MissileFork,
        Upgrade::MissileChain,
        Upgrade::MissileSpeed,
        Upgrade::MissileEcho,
        Upgrade::MissileBurst,
        Upgrade::MissileBurn,
        Upgrade::HolyAura,
        Upgrade::OrbitingRunes,
        Upgrade::MaxHealth,
        Upgrade::MoveSpeed,
        Upgrade::PickupRadius,
        Upgrade::Regeneration,
        Upgrade::Dash,
        Upgrade::Blink,
        Upgrade::EvolveSanctum,
        Upgrade::EvolvePrism,
        Upgrade::EvolveTempest,
    ];

    /// Health restored immediately when `MaxHealth` is picked.
    pub const MAX_HEALTH_HEAL: f32 = 25.0;

    /// Short name shown on the level-up card.
    pub fn title(self, stats: &PlayerStats) -> String {
        match self {
            Upgrade::MissileDamage => "Arcane Missile: Power".into(),
            Upgrade::MissileRate => "Arcane Missile: Haste".into(),
            Upgrade::MissileCount => "Arcane Missile: Volley".into(),
            Upgrade::MissilePierce => "Arcane Missile: Piercing".into(),
            Upgrade::MissileFork if stats.missile.fork_level == 0 => "NEW: Fork".into(),
            Upgrade::MissileFork => format!("Fork Lv {}", stats.missile.fork_level + 1),
            Upgrade::MissileChain if stats.missile.chain_level == 0 => "NEW: Chain".into(),
            Upgrade::MissileChain => format!("Chain Lv {}", stats.missile.chain_level + 1),
            Upgrade::MissileSpeed => "Arcane Missile: Swiftness".into(),
            Upgrade::MissileEcho if stats.missile.echo_level == 0 => "NEW: Spell Echo".into(),
            Upgrade::MissileEcho => format!("Spell Echo Lv {}", stats.missile.echo_level + 1),
            Upgrade::MissileBurst if stats.missile.burst_level == 0 => "NEW: Impact Burst".into(),
            Upgrade::MissileBurst => format!("Impact Burst Lv {}", stats.missile.burst_level + 1),
            Upgrade::MissileBurn if stats.missile.burn_level == 0 => "NEW: Searing Touch".into(),
            Upgrade::MissileBurn => format!("Searing Touch Lv {}", stats.missile.burn_level + 1),
            Upgrade::HolyAura if stats.aura.level == 0 => "NEW: Holy Aura".into(),
            Upgrade::HolyAura => format!("Holy Aura Lv {}", stats.aura.level + 1),
            Upgrade::OrbitingRunes if stats.runes.level == 0 => "NEW: Orbiting Runes".into(),
            Upgrade::OrbitingRunes => format!("Orbiting Runes Lv {}", stats.runes.level + 1),
            Upgrade::MaxHealth => "Troll Blood".into(),
            Upgrade::MoveSpeed => "Elven Boots".into(),
            Upgrade::PickupRadius => "Lodestone Amulet".into(),
            Upgrade::Regeneration => "Healing Herbs".into(),
            Upgrade::Dash if stats.dash.level == 0 => "NEW: Dash [Space]".into(),
            Upgrade::Dash => format!("Dash Lv {}", stats.dash.level + 1),
            Upgrade::Blink if stats.blink.level == 0 => "NEW: Blink [Right Mouse]".into(),
            Upgrade::Blink => format!("Blink Lv {}", stats.blink.level + 1),
            Upgrade::EvolveSanctum => "EVOLVE: Sanctum".into(),
            Upgrade::EvolvePrism => "EVOLVE: Prism Bolt".into(),
            Upgrade::EvolveTempest => "EVOLVE: Arcane Tempest".into(),
        }
    }

    /// One-line explanation of what the upgrade does.
    pub fn description(self, stats: &PlayerStats) -> String {
        match self {
            Upgrade::MissileDamage => "+25% missile damage".into(),
            Upgrade::MissileRate => "Missiles fire 15% faster".into(),
            Upgrade::MissileCount => "+1 missile per volley".into(),
            Upgrade::MissilePierce => "Missiles pass through +1 enemy".into(),
            Upgrade::MissileFork if stats.missile.fork_level == 0 => {
                "Missiles split in two after their first hit".into()
            }
            Upgrade::MissileFork => "Missiles split into one more bolt".into(),
            Upgrade::MissileChain if stats.missile.chain_level == 0 => {
                "Missiles jump to a nearby enemy after a hit".into()
            }
            Upgrade::MissileChain => "One more jump per missile".into(),
            Upgrade::MissileSpeed => "+20% missile speed".into(),
            Upgrade::MissileEcho if stats.missile.echo_level == 0 => {
                "Every volley repeats once at 75% damage".into()
            }
            Upgrade::MissileEcho => "One more repeat per volley".into(),
            Upgrade::MissileBurst if stats.missile.burst_level == 0 => {
                "Hits explode and damage nearby enemies".into()
            }
            Upgrade::MissileBurst => "Larger explosion".into(),
            Upgrade::MissileBurn if stats.missile.burn_level == 0 => {
                "Hit enemies burn for 3 seconds".into()
            }
            Upgrade::MissileBurn => "Burning deals more damage".into(),
            Upgrade::HolyAura if stats.aura.level == 0 => {
                "A sacred circle burns nearby enemies".into()
            }
            Upgrade::HolyAura => "Larger radius, more damage".into(),
            Upgrade::OrbitingRunes if stats.runes.level == 0 => {
                "Runestones circle you and smash enemies".into()
            }
            Upgrade::OrbitingRunes => "+1 rune, faster and stronger".into(),
            Upgrade::MaxHealth => format!("+20 max health, heal {}", Self::MAX_HEALTH_HEAL),
            Upgrade::MoveSpeed => "+10% movement speed".into(),
            Upgrade::PickupRadius => "+30% experience pickup range".into(),
            Upgrade::Regeneration => "Regenerate +0.5 health per second".into(),
            Upgrade::Dash if stats.dash.level == 0 => {
                "Burst forward and ignore damage while dashing".into()
            }
            Upgrade::Dash => "Dash farther, shorter cooldown".into(),
            Upgrade::Blink if stats.blink.level == 0 => "Teleport towards the cursor".into(),
            Upgrade::Blink => "Blink farther, shorter cooldown".into(),
            Upgrade::EvolveSanctum => {
                "Holy Aura and Runes fuse: bigger, twice the damage, 2 more runes".into()
            }
            Upgrade::EvolvePrism => "Forked bolts hit for 90% damage and jump once more".into(),
            Upgrade::EvolveTempest => "One more echo, and echoes deal full damage".into(),
        }
    }

    /// Whether the upgrade can still be offered (i.e. it is not maxed out).
    pub fn is_available(self, stats: &PlayerStats) -> bool {
        match self {
            Upgrade::MissileRate => stats.missile.cooldown > MIN_MISSILE_COOLDOWN + f32::EPSILON,
            Upgrade::MissileCount => stats.missile.count < MAX_MISSILE_COUNT,
            Upgrade::MissilePierce => stats.missile.pierce < MAX_MISSILE_PIERCE,
            Upgrade::MissileFork => stats.missile.fork_level < MissileStats::MAX_FORK_LEVEL,
            Upgrade::MissileChain => stats.missile.chain_level < MissileStats::MAX_CHAIN_LEVEL,
            Upgrade::MissileSpeed => stats.missile.speed < MAX_MISSILE_SPEED,
            Upgrade::MissileEcho => stats.missile.echo_level < MissileStats::MAX_ECHO_LEVEL,
            Upgrade::MissileBurst => stats.missile.burst_level < MissileStats::MAX_BURST_LEVEL,
            Upgrade::MissileBurn => stats.missile.burn_level < MissileStats::MAX_BURN_LEVEL,
            Upgrade::Dash => stats.dash.level < DashStats::MAX_LEVEL,
            Upgrade::Blink => stats.blink.level < BlinkStats::MAX_LEVEL,
            Upgrade::EvolveSanctum => {
                stats.aura.level >= AuraStats::MAX_LEVEL
                    && stats.runes.level >= RuneStats::MAX_LEVEL
                    && !stats.aura.evolved
            }
            Upgrade::EvolvePrism => {
                stats.missile.fork_level >= MissileStats::MAX_FORK_LEVEL
                    && stats.missile.chain_level >= MissileStats::MAX_CHAIN_LEVEL
                    && !stats.missile.prism
            }
            Upgrade::EvolveTempest => {
                stats.missile.echo_level >= MissileStats::MAX_ECHO_LEVEL
                    && stats.missile.count >= EVOLVE_TEMPEST_VOLLEY
                    && !stats.missile.tempest
            }
            Upgrade::HolyAura => stats.aura.level < AuraStats::MAX_LEVEL,
            Upgrade::OrbitingRunes => stats.runes.level < RuneStats::MAX_LEVEL,
            Upgrade::MoveSpeed => stats.move_speed < MAX_MOVE_SPEED,
            Upgrade::PickupRadius => stats.pickup_radius < MAX_PICKUP_RADIUS,
            Upgrade::MissileDamage | Upgrade::MaxHealth | Upgrade::Regeneration => true,
        }
    }

    /// Applies the upgrade to the stats.
    pub fn apply(self, stats: &mut PlayerStats) {
        match self {
            Upgrade::MissileDamage => stats.missile.damage *= 1.25,
            Upgrade::MissileRate => {
                stats.missile.cooldown = (stats.missile.cooldown * 0.85).max(MIN_MISSILE_COOLDOWN)
            }
            Upgrade::MissileCount => {
                stats.missile.count = (stats.missile.count + 1).min(MAX_MISSILE_COUNT)
            }
            Upgrade::MissilePierce => {
                stats.missile.pierce = (stats.missile.pierce + 1).min(MAX_MISSILE_PIERCE)
            }
            Upgrade::MissileFork => {
                stats.missile.fork_level =
                    (stats.missile.fork_level + 1).min(MissileStats::MAX_FORK_LEVEL)
            }
            Upgrade::Dash => stats.dash.level = (stats.dash.level + 1).min(DashStats::MAX_LEVEL),
            Upgrade::Blink => {
                stats.blink.level = (stats.blink.level + 1).min(BlinkStats::MAX_LEVEL)
            }
            Upgrade::HolyAura => {
                stats.aura.level = (stats.aura.level + 1).min(AuraStats::MAX_LEVEL)
            }
            Upgrade::OrbitingRunes => {
                stats.runes.level = (stats.runes.level + 1).min(RuneStats::MAX_LEVEL)
            }
            Upgrade::MissileChain => {
                stats.missile.chain_level =
                    (stats.missile.chain_level + 1).min(MissileStats::MAX_CHAIN_LEVEL)
            }
            Upgrade::MissileSpeed => {
                stats.missile.speed = (stats.missile.speed * 1.2).min(MAX_MISSILE_SPEED)
            }
            Upgrade::MissileEcho => {
                stats.missile.echo_level =
                    (stats.missile.echo_level + 1).min(MissileStats::MAX_ECHO_LEVEL)
            }
            Upgrade::MissileBurst => {
                stats.missile.burst_level =
                    (stats.missile.burst_level + 1).min(MissileStats::MAX_BURST_LEVEL)
            }
            Upgrade::MissileBurn => {
                stats.missile.burn_level =
                    (stats.missile.burn_level + 1).min(MissileStats::MAX_BURN_LEVEL)
            }
            Upgrade::EvolveSanctum => {
                stats.aura.evolved = true;
                stats.runes.evolved = true;
            }
            Upgrade::EvolvePrism => stats.missile.prism = true,
            Upgrade::EvolveTempest => stats.missile.tempest = true,
            Upgrade::MaxHealth => stats.max_health += 20.0,
            Upgrade::MoveSpeed => stats.move_speed = (stats.move_speed * 1.1).min(MAX_MOVE_SPEED),
            Upgrade::PickupRadius => {
                stats.pickup_radius = (stats.pickup_radius * 1.3).min(MAX_PICKUP_RADIUS)
            }
            Upgrade::Regeneration => stats.regen_per_sec += 0.5,
        }
    }
}

/// Rolls up to `n` distinct, currently available upgrades.
///
/// New weapons are weighted a little higher so builds get going early.
pub fn roll_upgrades(rng: &mut Rng, stats: &PlayerStats, n: usize) -> Vec<Upgrade> {
    let mut pool: Vec<(Upgrade, f32)> = Upgrade::ALL
        .iter()
        .copied()
        .filter(|u| u.is_available(stats))
        .map(|u| {
            let weight = match u {
                Upgrade::HolyAura if stats.aura.level == 0 => 2.0,
                Upgrade::OrbitingRunes if stats.runes.level == 0 => 2.0,
                // Evolutions are the reward for a focused build, so they come up often.
                Upgrade::EvolveSanctum | Upgrade::EvolvePrism | Upgrade::EvolveTempest => 4.0,
                _ => 1.0,
            };
            (u, weight)
        })
        .collect();

    let mut picked = Vec::with_capacity(n);
    while picked.len() < n && !pool.is_empty() {
        let weights: Vec<f32> = pool.iter().map(|(_, w)| *w).collect();
        let Some(i) = rng.weighted_index(&weights) else {
            break;
        };
        picked.push(pool.swap_remove(i).0);
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolled_upgrades_are_distinct_and_available() {
        let mut rng = Rng::new(11);
        let stats = PlayerStats::default();
        for _ in 0..500 {
            let choices = roll_upgrades(&mut rng, &stats, 3);
            assert_eq!(choices.len(), 3);
            for (i, a) in choices.iter().enumerate() {
                assert!(a.is_available(&stats));
                assert!(!choices[i + 1..].contains(a));
            }
        }
    }

    #[test]
    fn maxed_upgrades_are_no_longer_offered() {
        let mut stats = PlayerStats::default();
        for _ in 0..20 {
            Upgrade::MissileCount.apply(&mut stats);
            Upgrade::HolyAura.apply(&mut stats);
        }
        assert_eq!(stats.missile.count, MAX_MISSILE_COUNT);
        assert_eq!(stats.aura.level, AuraStats::MAX_LEVEL);
        assert!(!Upgrade::MissileCount.is_available(&stats));
        assert!(!Upgrade::HolyAura.is_available(&stats));

        let mut rng = Rng::new(1);
        for _ in 0..200 {
            let choices = roll_upgrades(&mut rng, &stats, 3);
            assert!(!choices.contains(&Upgrade::MissileCount));
            assert!(!choices.contains(&Upgrade::HolyAura));
        }
    }

    #[test]
    fn cooldown_never_drops_below_minimum() {
        let mut stats = PlayerStats::default();
        let mut guard = 0;
        while Upgrade::MissileRate.is_available(&stats) {
            Upgrade::MissileRate.apply(&mut stats);
            guard += 1;
            assert!(guard < 100, "MissileRate never maxes out");
        }
        assert!(stats.missile.cooldown >= MIN_MISSILE_COOLDOWN);
    }

    #[test]
    fn movement_skills_and_fork_start_locked_and_level_up() {
        let mut stats = PlayerStats::default();
        assert_eq!(stats.dash.level, 0);
        assert_eq!(stats.blink.level, 0);
        assert_eq!(stats.missile.fork_count(), 0);

        Upgrade::Dash.apply(&mut stats);
        Upgrade::Blink.apply(&mut stats);
        Upgrade::MissileFork.apply(&mut stats);
        assert_eq!(stats.dash.level, 1);
        assert_eq!(stats.blink.level, 1);
        assert_eq!(stats.missile.fork_count(), 2);

        Upgrade::MissileFork.apply(&mut stats);
        assert_eq!(stats.missile.fork_count(), 3);
    }

    #[test]
    fn higher_skill_levels_are_stronger_but_cooldowns_have_a_floor() {
        let mut low = PlayerStats::default();
        let mut high = PlayerStats::default();
        Upgrade::Dash.apply(&mut low);
        Upgrade::Blink.apply(&mut low);
        for _ in 0..DashStats::MAX_LEVEL {
            Upgrade::Dash.apply(&mut high);
            Upgrade::Blink.apply(&mut high);
        }
        assert!(high.dash.distance() > low.dash.distance());
        assert!(high.blink.range() > low.blink.range());
        assert!(high.dash.cooldown() < low.dash.cooldown());
        assert!(high.blink.cooldown() < low.blink.cooldown());
        assert!(high.dash.cooldown() >= 1.0);
        assert!(high.blink.cooldown() >= 2.8);
        assert!(!Upgrade::Dash.is_available(&high));
        assert!(!Upgrade::Blink.is_available(&high));
    }

    #[test]
    fn spell_modifiers_start_locked_and_can_be_levelled() {
        let mut stats = PlayerStats::default();
        let m = &stats.missile;
        assert_eq!(m.chain_count(), 0);
        assert_eq!(m.echo_count(), 0);
        assert_eq!(m.burst_radius(), 0.0);
        assert_eq!(m.burn_dps(), 0.0);

        for upgrade in [
            Upgrade::MissileChain,
            Upgrade::MissileEcho,
            Upgrade::MissileBurst,
            Upgrade::MissileBurn,
        ] {
            upgrade.apply(&mut stats);
        }
        let m = &stats.missile;
        assert_eq!(m.chain_count(), 1);
        assert_eq!(m.echo_count(), 1);
        assert!(m.burst_radius() > 0.0);
        assert!(m.burn_dps() > 0.0);

        let first_burst = m.burst_radius();
        Upgrade::MissileBurst.apply(&mut stats);
        assert!(stats.missile.burst_radius() > first_burst);
    }

    #[test]
    fn spell_modifiers_stop_being_offered_at_their_cap() {
        let mut stats = PlayerStats::default();
        for _ in 0..20 {
            Upgrade::MissileChain.apply(&mut stats);
            Upgrade::MissileEcho.apply(&mut stats);
            Upgrade::MissileBurst.apply(&mut stats);
            Upgrade::MissileBurn.apply(&mut stats);
            Upgrade::MissileFork.apply(&mut stats);
        }
        for upgrade in [
            Upgrade::MissileChain,
            Upgrade::MissileEcho,
            Upgrade::MissileBurst,
            Upgrade::MissileBurn,
            Upgrade::MissileFork,
        ] {
            assert!(!upgrade.is_available(&stats), "{upgrade:?} should be maxed");
        }
    }

    #[test]
    fn missile_speed_is_capped() {
        let mut stats = PlayerStats::default();
        let mut guard = 0;
        while Upgrade::MissileSpeed.is_available(&stats) {
            Upgrade::MissileSpeed.apply(&mut stats);
            guard += 1;
            assert!(guard < 100, "MissileSpeed never maxes out");
        }
        assert!(stats.missile.speed <= MAX_MISSILE_SPEED);
    }

    #[test]
    fn evolutions_need_their_ingredients_and_apply_once() {
        let mut stats = PlayerStats::default();
        for evolution in [
            Upgrade::EvolveSanctum,
            Upgrade::EvolvePrism,
            Upgrade::EvolveTempest,
        ] {
            assert!(!evolution.is_available(&stats), "{evolution:?} too early");
        }

        for _ in 0..10 {
            Upgrade::HolyAura.apply(&mut stats);
            Upgrade::OrbitingRunes.apply(&mut stats);
            Upgrade::MissileFork.apply(&mut stats);
            Upgrade::MissileChain.apply(&mut stats);
            Upgrade::MissileEcho.apply(&mut stats);
            Upgrade::MissileCount.apply(&mut stats);
        }
        let (aura_radius, rune_count) = (stats.aura.radius(), stats.runes.count());
        let echo = stats.missile.echo_count();
        for evolution in [
            Upgrade::EvolveSanctum,
            Upgrade::EvolvePrism,
            Upgrade::EvolveTempest,
        ] {
            assert!(evolution.is_available(&stats), "{evolution:?} not offered");
            evolution.apply(&mut stats);
            assert!(
                !evolution.is_available(&stats),
                "{evolution:?} offered twice"
            );
        }
        assert!(stats.aura.radius() > aura_radius);
        assert_eq!(stats.runes.count(), rune_count + 2);
        assert_eq!(stats.missile.echo_count(), echo + 1);
        assert!(stats.missile.fork_damage_factor() > MissileStats::FORK_DAMAGE_FACTOR);
        assert_eq!(stats.missile.fork_chains(), 1);
        assert_eq!(stats.missile.echo_damage_factor(), 1.0);
    }

    #[test]
    fn unlocking_runes_gives_two_runes() {
        let mut stats = PlayerStats::default();
        assert_eq!(stats.runes.count(), 0);
        Upgrade::OrbitingRunes.apply(&mut stats);
        assert_eq!(stats.runes.count(), 2);
        assert!(Upgrade::OrbitingRunes.title(&stats).contains("Lv 2"));
    }
}
