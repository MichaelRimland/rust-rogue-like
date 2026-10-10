//! Timing and geometry rules for the movement skills (dash and blink).

/// A skill cooldown that starts out ready. The duration is passed when the skill fires,
/// so it can change as the skill levels up.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Cooldown {
    remaining: f32,
}

impl Cooldown {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tick(&mut self, dt: f32) {
        self.remaining = (self.remaining - dt).max(0.0);
    }

    pub fn is_ready(&self) -> bool {
        self.remaining <= 0.0
    }

    /// Seconds until the skill can be used again, `0.0` when ready.
    pub fn remaining(&self) -> f32 {
        self.remaining
    }

    /// Starts a cooldown of `duration` seconds if the skill is ready.
    /// Returns whether it fired.
    pub fn trigger(&mut self, duration: f32) -> bool {
        if !self.is_ready() {
            return false;
        }
        self.remaining = duration;
        true
    }
}

/// Moves `target` back along the line from `origin` so that it is at most `max_distance`
/// away from `origin`. Targets inside the range are returned unchanged.
pub fn limit_distance(origin: [f32; 2], target: [f32; 2], max_distance: f32) -> [f32; 2] {
    let dx = target[0] - origin[0];
    let dy = target[1] - origin[1];
    let distance = (dx * dx + dy * dy).sqrt();
    if distance <= max_distance || distance < 1e-6 {
        return target;
    }
    let scale = max_distance / distance;
    [origin[0] + dx * scale, origin[1] + dy * scale]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cooldown_starts_ready_and_blocks_until_elapsed() {
        let mut cooldown = Cooldown::new();
        assert!(cooldown.trigger(2.0));
        assert!(!cooldown.trigger(2.0));
        cooldown.tick(1.5);
        assert!(!cooldown.is_ready());
        assert!((cooldown.remaining() - 0.5).abs() < 1e-6);
        cooldown.tick(0.5);
        assert!(cooldown.trigger(2.0));
    }

    #[test]
    fn cooldown_never_goes_negative() {
        let mut cooldown = Cooldown::new();
        cooldown.trigger(1.0);
        cooldown.tick(10.0);
        assert_eq!(cooldown.remaining(), 0.0);
    }

    #[test]
    fn targets_in_range_are_unchanged() {
        assert_eq!(
            limit_distance([0.0, 0.0], [30.0, 40.0], 100.0),
            [30.0, 40.0]
        );
    }

    #[test]
    fn far_targets_are_pulled_back_onto_the_range_circle() {
        let limited = limit_distance([10.0, 10.0], [310.0, 410.0], 100.0);
        let dx = limited[0] - 10.0;
        let dy = limited[1] - 10.0;
        assert!(((dx * dx + dy * dy).sqrt() - 100.0).abs() < 1e-3);
        // Same direction as the original target (300, 400).
        assert!((dx * 400.0 - dy * 300.0).abs() < 1e-2);
    }

    #[test]
    fn target_on_top_of_origin_does_not_produce_nan() {
        let limited = limit_distance([5.0, 5.0], [5.0, 5.0], 0.0);
        assert!(limited[0].is_finite() && limited[1].is_finite());
    }
}
