//! Experience, levels and difficulty curves.

/// Experience needed to advance from `level` to `level + 1`.
///
/// Grows quadratically so early level-ups come quickly and later ones take longer:
/// 5, 14, 25, 38, 53, ...
pub fn xp_to_next_level(level: u32) -> u32 {
    let l = level.saturating_sub(1);
    5 + 8 * l + l * l
}

/// The player's experience and level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Experience {
    pub level: u32,
    /// Experience collected towards the next level.
    pub current: u32,
}

impl Default for Experience {
    fn default() -> Self {
        Self {
            level: 1,
            current: 0,
        }
    }
}

impl Experience {
    /// Experience required for the next level-up.
    pub fn needed(&self) -> u32 {
        xp_to_next_level(self.level)
    }

    /// Adds experience and returns how many levels were gained.
    pub fn add(&mut self, amount: u32) -> u32 {
        self.current += amount;
        let mut gained = 0;
        while self.current >= self.needed() {
            self.current -= self.needed();
            self.level += 1;
            gained += 1;
        }
        gained
    }

    /// Progress towards the next level in `[0, 1)`.
    pub fn progress(&self) -> f32 {
        self.current as f32 / self.needed() as f32
    }
}

/// Seconds between two enemy spawn waves at a given point of the run.
pub fn spawn_interval(elapsed_secs: f32) -> f32 {
    (1.4 * 0.5f32.powf(elapsed_secs / 150.0)).max(0.25)
}

/// Number of enemies spawned per wave.
pub fn spawn_batch_size(elapsed_secs: f32) -> u32 {
    1 + (elapsed_secs / 25.0) as u32
}

/// Health multiplier applied to newly spawned enemies.
pub fn enemy_health_multiplier(elapsed_secs: f32) -> f32 {
    1.0 + elapsed_secs / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xp_curve_is_strictly_increasing() {
        for level in 1..100 {
            assert!(xp_to_next_level(level + 1) > xp_to_next_level(level));
        }
        assert_eq!(xp_to_next_level(1), 5);
    }

    #[test]
    fn adding_xp_can_gain_multiple_levels() {
        let mut xp = Experience::default();
        assert_eq!(xp.add(4), 0);
        assert_eq!(xp.level, 1);
        // 1 more reaches level 2 (needs 5), then 14 more reaches level 3.
        assert_eq!(xp.add(1 + 14 + 2), 2);
        assert_eq!(xp.level, 3);
        assert_eq!(xp.current, 2);
        assert!(xp.progress() > 0.0 && xp.progress() < 1.0);
    }

    #[test]
    fn difficulty_ramps_up_over_time() {
        assert!(spawn_interval(300.0) < spawn_interval(0.0));
        assert!(spawn_interval(10_000.0) >= 0.25);
        assert!(spawn_batch_size(120.0) > spawn_batch_size(0.0));
        assert!(enemy_health_multiplier(200.0) > enemy_health_multiplier(0.0));
    }
}
