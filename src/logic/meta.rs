//! Meta-progression: what carries over between runs.
//!
//! Every run earns *soul shards* that are spent at the shrine on small permanent bonuses.
//! This module holds the rules and the serializable save data, without any engine code.

use serde::{Deserialize, Serialize};

use super::stats::PlayerStats;

/// Highest rank of every shrine upgrade.
pub const MAX_RANK: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShrineUpgrade {
    /// More maximum health.
    Vitality,
    /// Faster movement.
    Swiftness,
    /// Start every dungeon run with some gold.
    Fortune,
}

impl ShrineUpgrade {
    pub const ALL: [ShrineUpgrade; 3] = [
        ShrineUpgrade::Vitality,
        ShrineUpgrade::Swiftness,
        ShrineUpgrade::Fortune,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ShrineUpgrade::Vitality => "Vitality",
            ShrineUpgrade::Swiftness => "Swiftness",
            ShrineUpgrade::Fortune => "Fortune",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ShrineUpgrade::Vitality => "+10 maximum health per rank",
            ShrineUpgrade::Swiftness => "+3% movement speed per rank",
            ShrineUpgrade::Fortune => "+15 starting gold per rank (dungeon)",
        }
    }
}

/// How a finished run went; input for [`MetaProgress::record_run`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunSummary {
    /// `true` for dungeon runs, `false` for endless survival.
    pub dungeon: bool,
    pub floor: u32,
    pub kills: u32,
    pub level: u32,
    pub seconds: f32,
    pub victory: bool,
}

/// Everything saved between runs. `#[serde(default)]` keeps old save files loadable when
/// new fields are added.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MetaProgress {
    pub shards: u32,
    pub vitality: u32,
    pub swiftness: u32,
    pub fortune: u32,
    pub runs: u32,
    pub victories: u32,
    pub best_floor: u32,
    pub best_kills: u32,
    pub best_level: u32,
    /// Longest survival in endless mode, in seconds.
    pub best_time: f32,
}

impl MetaProgress {
    pub fn rank(&self, upgrade: ShrineUpgrade) -> u32 {
        match upgrade {
            ShrineUpgrade::Vitality => self.vitality,
            ShrineUpgrade::Swiftness => self.swiftness,
            ShrineUpgrade::Fortune => self.fortune,
        }
    }

    /// Shards needed for the next rank, `None` when the upgrade is maxed out.
    pub fn cost(&self, upgrade: ShrineUpgrade) -> Option<u32> {
        let rank = self.rank(upgrade);
        (rank < MAX_RANK).then(|| 25 * (rank + 1))
    }

    pub fn can_buy(&self, upgrade: ShrineUpgrade) -> bool {
        self.cost(upgrade).is_some_and(|cost| self.shards >= cost)
    }

    /// Buys the next rank. Returns whether it succeeded.
    pub fn buy(&mut self, upgrade: ShrineUpgrade) -> bool {
        let Some(cost) = self.cost(upgrade) else {
            return false;
        };
        if self.shards < cost {
            return false;
        }
        self.shards -= cost;
        match upgrade {
            ShrineUpgrade::Vitality => self.vitality += 1,
            ShrineUpgrade::Swiftness => self.swiftness += 1,
            ShrineUpgrade::Fortune => self.fortune += 1,
        }
        true
    }

    /// Applies the permanent bonuses to the stats of a fresh run.
    pub fn apply_to(&self, stats: &mut PlayerStats) {
        stats.max_health += 10.0 * self.vitality as f32;
        stats.move_speed *= 1.0 + 0.03 * self.swiftness as f32;
    }

    pub fn starting_gold(&self) -> u32 {
        15 * self.fortune
    }

    /// Updates the records and returns the shards the run earned.
    pub fn record_run(&mut self, run: &RunSummary) -> u32 {
        let shards = if run.dungeon {
            run.floor * 10 + run.kills / 10 + run.level + if run.victory { 50 } else { 0 }
        } else {
            (run.seconds / 30.0) as u32 + run.kills / 10 + run.level
        };
        self.shards += shards;
        self.runs += 1;
        if run.victory {
            self.victories += 1;
        }
        if run.dungeon {
            self.best_floor = self.best_floor.max(run.floor);
        } else {
            self.best_time = self.best_time.max(run.seconds);
        }
        self.best_kills = self.best_kills.max(run.kills);
        self.best_level = self.best_level.max(run.level);
        shards
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dungeon_run(floor: u32, victory: bool) -> RunSummary {
        RunSummary {
            dungeon: true,
            floor,
            kills: 95,
            level: 12,
            seconds: 600.0,
            victory,
        }
    }

    #[test]
    fn nothing_is_affordable_without_shards() {
        let meta = MetaProgress::default();
        for upgrade in ShrineUpgrade::ALL {
            assert!(!meta.can_buy(upgrade));
        }
    }

    #[test]
    fn buying_spends_shards_and_raises_the_rank() {
        let mut meta = MetaProgress {
            shards: 100,
            ..Default::default()
        };
        assert!(meta.buy(ShrineUpgrade::Vitality));
        assert_eq!(meta.vitality, 1);
        assert_eq!(meta.shards, 75);
        // The second rank costs more.
        assert_eq!(meta.cost(ShrineUpgrade::Vitality), Some(50));
        assert!(meta.buy(ShrineUpgrade::Vitality));
        assert_eq!(meta.shards, 25);
        assert!(!meta.buy(ShrineUpgrade::Vitality), "cannot afford rank 3");
    }

    #[test]
    fn upgrades_stop_at_the_maximum_rank() {
        let mut meta = MetaProgress {
            shards: 10_000,
            ..Default::default()
        };
        for _ in 0..20 {
            meta.buy(ShrineUpgrade::Fortune);
        }
        assert_eq!(meta.fortune, MAX_RANK);
        assert_eq!(meta.cost(ShrineUpgrade::Fortune), None);
        assert!(!meta.can_buy(ShrineUpgrade::Fortune));
    }

    #[test]
    fn bonuses_apply_to_the_starting_stats() {
        let meta = MetaProgress {
            vitality: 2,
            swiftness: 3,
            fortune: 4,
            ..Default::default()
        };
        let base = PlayerStats::default();
        let mut boosted = base.clone();
        meta.apply_to(&mut boosted);
        assert_eq!(boosted.max_health, base.max_health + 20.0);
        assert!(boosted.move_speed > base.move_speed);
        assert_eq!(meta.starting_gold(), 60);
    }

    #[test]
    fn runs_update_records_and_award_shards() {
        let mut meta = MetaProgress::default();
        let first = meta.record_run(&dungeon_run(2, false));
        assert_eq!(first, 2 * 10 + 9 + 12);
        assert_eq!(meta.shards, first);
        assert_eq!(meta.best_floor, 2);
        assert_eq!(meta.runs, 1);

        // A worse run never lowers a record.
        meta.record_run(&dungeon_run(1, false));
        assert_eq!(meta.best_floor, 2);

        let win = meta.record_run(&dungeon_run(3, true));
        assert!(win >= 50);
        assert_eq!(meta.victories, 1);
    }

    #[test]
    fn endless_runs_track_survival_time() {
        let mut meta = MetaProgress::default();
        meta.record_run(&RunSummary {
            dungeon: false,
            floor: 0,
            kills: 40,
            level: 8,
            seconds: 300.0,
            victory: false,
        });
        assert_eq!(meta.best_time, 300.0);
        assert_eq!(meta.best_floor, 0);
    }

    #[test]
    fn save_data_round_trips_and_tolerates_missing_fields() {
        let meta = MetaProgress {
            shards: 42,
            vitality: 3,
            best_floor: 2,
            ..Default::default()
        };
        let text = ron::to_string(&meta).expect("serializes");
        let back: MetaProgress = ron::from_str(&text).expect("parses");
        assert_eq!(back, meta);

        let old: MetaProgress = ron::from_str("(shards: 7)").expect("old save loads");
        assert_eq!(old.shards, 7);
        assert_eq!(old.vitality, 0);
    }
}
