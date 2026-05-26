//! Critically-damped spring integration.
//!
//! Every transition in Eden that moves, scales, or fades is driven by a spring
//! rather than a fixed-duration easing curve (see the animation principles in
//! the build plan). A spring has no fixed duration: it is defined by a target
//! and a physical response, so interrupting it mid-flight stays continuous
//! instead of snapping or restarting.

/// Tuning for a [`Spring`].
///
/// The default is the house spring: `stiffness = 170`, `damping = 26`, which is
/// approximately critical damping for unit mass (`2 * sqrt(170) ~= 26.08`) and
/// so settles quickly without overshoot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringConfig {
    /// Restoring force per unit of displacement from the target.
    pub stiffness: f64,
    /// Velocity-proportional damping force.
    pub damping: f64,
    /// The spring is considered at rest once it is within this distance of the
    /// target (in the spring's own units).
    pub rest_distance: f64,
    /// ...and once its speed drops below this threshold.
    pub rest_velocity: f64,
}

impl SpringConfig {
    /// The default house spring (`stiffness = 170`, `damping = 26`).
    ///
    /// Rest thresholds are tuned for pixel-space values (positions and sizes),
    /// which is the common case: it stops once it is within ~0.4px and moving
    /// slower than ~2px/s, so the imperceptible sub-pixel tail of a soft spring
    /// doesn't keep the frame loop awake. Use [`SpringConfig::UNIT`] for
    /// normalized `0..=1` values instead.
    pub const DEFAULT: Self = Self {
        stiffness: 170.0,
        damping: 26.0,
        rest_distance: 0.4,
        rest_velocity: 2.0,
    };

    /// A spring for normalized `0..=1` values (opacity, crossfade mix), with
    /// correspondingly tight rest thresholds. Slightly stiffer than the house
    /// spring so a fade reads as deliberate but stays under ~300ms.
    pub const UNIT: Self = Self {
        stiffness: 210.0,
        damping: 30.0,
        // ~1/256: below a single 8-bit colour step, so resting here is visually
        // exact for an opacity/crossfade value.
        rest_distance: 0.004,
        rest_velocity: 0.03,
    };

    /// A snappier spring for small, frequent UI transitions (hover, focus).
    pub const SNAPPY: Self = Self {
        stiffness: 320.0,
        damping: 34.0,
        ..Self::DEFAULT
    };

    /// A faster spring used when the OS "reduce motion" preference is set.
    ///
    /// Motion is reduced, not eliminated: this still crosses its range in well
    /// under 80ms, preserving the "where did this come from" cue while removing
    /// the lingering travel.
    pub const REDUCED: Self = Self {
        stiffness: 1100.0,
        damping: 66.0,
        ..Self::DEFAULT
    };

    /// Builds a critically-damped config for the given stiffness (unit mass).
    #[must_use]
    pub fn critically_damped(stiffness: f64) -> Self {
        Self {
            stiffness,
            damping: 2.0 * stiffness.sqrt(),
            ..Self::DEFAULT
        }
    }
}

impl Default for SpringConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A single scalar value pulled toward a target by a spring force.
///
/// Step it once per frame with the elapsed time. [`Spring::step`] returns
/// whether the spring is still in motion, which the caller uses to decide
/// whether to schedule another frame.
#[derive(Clone, Copy, Debug)]
pub struct Spring {
    value: f64,
    velocity: f64,
    target: f64,
    config: SpringConfig,
}

impl Spring {
    /// Creates a spring at rest at `value`, using [`SpringConfig::DEFAULT`].
    #[must_use]
    pub fn new(value: f64) -> Self {
        Self::with_config(value, SpringConfig::DEFAULT)
    }

    /// Creates a spring at rest at `value` with the given configuration.
    #[must_use]
    pub fn with_config(value: f64, config: SpringConfig) -> Self {
        Self {
            value,
            velocity: 0.0,
            target: value,
            config,
        }
    }

    /// The current value.
    #[must_use]
    pub fn value(&self) -> f64 {
        self.value
    }

    /// The target the spring is travelling toward.
    #[must_use]
    pub fn target(&self) -> f64 {
        self.target
    }

    /// The current velocity.
    #[must_use]
    pub fn velocity(&self) -> f64 {
        self.velocity
    }

    /// Replaces the configuration, keeping the current value, velocity, target.
    pub fn set_config(&mut self, config: SpringConfig) {
        self.config = config;
    }

    /// Sets a new target. The spring keeps its current value and velocity, so a
    /// retarget mid-flight is smooth rather than a restart.
    pub fn set_target(&mut self, target: f64) {
        self.target = target;
    }

    /// Snaps value and target to `value`, clearing velocity. Use this for an
    /// instantaneous jump (initial placement, or honoring reduced motion).
    pub fn jump_to(&mut self, value: f64) {
        self.value = value;
        self.target = value;
        self.velocity = 0.0;
    }

    /// Whether the spring has effectively reached its target and stopped.
    #[must_use]
    pub fn is_at_rest(&self) -> bool {
        (self.value - self.target).abs() <= self.config.rest_distance
            && self.velocity.abs() <= self.config.rest_velocity
    }

    /// Advances the simulation by `dt` seconds. Returns `true` while the spring
    /// is still moving and `false` once it has settled (and snapped exactly to
    /// the target).
    ///
    /// Integration is semi-implicit Euler, sub-stepped at a fixed 240Hz so a
    /// long stalled frame can never make a stiff spring diverge.
    pub fn step(&mut self, dt: f64) -> bool {
        if self.is_at_rest() {
            self.value = self.target;
            self.velocity = 0.0;
            return false;
        }

        // design: cap dt to ~one frame at 15fps. Beyond that the app was stalled
        // and snapping the simulation forward is better than animating the gap.
        let dt = dt.clamp(0.0, 1.0 / 15.0);
        const SUB_DT: f64 = 1.0 / 240.0;
        let steps = (dt / SUB_DT).ceil().max(1.0);
        let h = dt / steps;

        let mut remaining = steps as u32;
        while remaining > 0 {
            let restoring = -self.config.stiffness * (self.value - self.target);
            let damping = -self.config.damping * self.velocity;
            // Unit mass, so acceleration equals net force.
            self.velocity += (restoring + damping) * h;
            self.value += self.velocity * h;
            remaining -= 1;
        }

        if self.is_at_rest() {
            self.value = self.target;
            self.velocity = 0.0;
            false
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle(spring: &mut Spring) -> u32 {
        let mut frames = 0;
        // 60fps frames, generous cap.
        while spring.step(1.0 / 60.0) {
            frames += 1;
            assert!(frames < 600, "spring failed to settle within 10s");
        }
        frames
    }

    #[test]
    fn at_rest_when_value_equals_target() {
        let mut s = Spring::new(5.0);
        assert!(!s.step(1.0 / 60.0));
        assert!(s.is_at_rest());
    }

    #[test]
    fn converges_to_target() {
        let mut s = Spring::new(0.0);
        s.set_target(100.0);
        settle(&mut s);
        assert!((s.value() - 100.0).abs() < 0.5);
        assert_eq!(s.value(), s.target());
    }

    #[test]
    fn critically_damped_does_not_overshoot() {
        let mut s = Spring::new(0.0);
        s.set_target(1.0);
        let mut max_seen = 0.0_f64;
        while s.step(1.0 / 120.0) {
            max_seen = max_seen.max(s.value());
        }
        // A critically (or over-) damped spring approaches monotonically; allow
        // only a hair of numerical slop.
        assert!(max_seen <= 1.0 + 1e-3, "overshot to {max_seen}");
    }

    #[test]
    fn perceptually_fast_then_terminates() {
        let mut s = Spring::new(0.0);
        s.set_target(240.0);
        // After ~0.3s a critically-damped house spring has covered the large
        // majority of the distance — the motion reads as a ~300ms move even
        // though the sub-pixel tail takes longer to formally settle.
        for _ in 0..18 {
            s.step(1.0 / 60.0);
        }
        assert!(s.value() > 0.8 * 240.0, "only reached {} after 0.3s", s.value());
        // And it does terminate (snapping exactly to target at rest).
        settle(&mut s);
        assert_eq!(s.value(), 240.0);
    }

    #[test]
    fn unit_config_is_perceptually_fast_and_terminates() {
        let mut s = Spring::with_config(0.0, SpringConfig::UNIT);
        s.set_target(1.0);
        // ~0.25s in, a crossfade should be nearly complete.
        for _ in 0..15 {
            s.step(1.0 / 60.0);
        }
        assert!(s.value() > 0.85, "unit fade only at {} after 0.25s", s.value());
        let mut frames = 15;
        while s.step(1.0 / 60.0) {
            frames += 1;
            assert!(frames < 120, "unit spring failed to settle");
        }
        assert_eq!(s.value(), 1.0);
    }

    #[test]
    fn retarget_is_continuous() {
        let mut s = Spring::new(0.0);
        s.set_target(100.0);
        for _ in 0..10 {
            s.step(1.0 / 60.0);
        }
        let v = s.velocity();
        s.set_target(50.0);
        // Velocity is preserved across a retarget (no restart).
        assert_eq!(s.velocity(), v);
    }

    #[test]
    fn jump_to_clears_motion() {
        let mut s = Spring::new(0.0);
        s.set_target(100.0);
        s.step(1.0 / 60.0);
        s.jump_to(42.0);
        assert_eq!(s.value(), 42.0);
        assert_eq!(s.velocity(), 0.0);
        assert!(s.is_at_rest());
    }
}
