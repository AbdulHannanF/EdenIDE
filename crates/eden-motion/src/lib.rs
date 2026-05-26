//! `eden-motion` — spring-physics animations and choreography.
//!
//! Eden's motion system has one rule: springs, never linear easings. A spring
//! is defined by where it is going and how it responds, not by a duration, so
//! interruptions stay continuous. See [`Spring`] for the integrator and
//! [`MotionPrefs`] for the reduce-motion accessibility path.
//!
//! The "animation driver" is deliberately not a global object. A spring is
//! stepped once per frame by whoever owns it, and reports (via [`Spring::step`])
//! whether it is still moving; the frame loop schedules another frame while any
//! spring is in motion and otherwise idles. This keeps motion pull-based and
//! costs nothing at rest.

mod spring;

pub use spring::{Spring, SpringConfig};

/// Accessibility preferences that shape how motion is presented.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MotionPrefs {
    /// When set, transitions are shortened (not removed) — see
    /// [`SpringConfig::REDUCED`].
    pub reduce_motion: bool,
}

impl MotionPrefs {
    /// Reads preferences from the environment.
    ///
    /// `EDEN_REDUCE_MOTION=1` forces reduced motion. OS-level detection (e.g.
    /// the Windows "show animations" setting) is wired up in a later phase;
    /// this env override is the portable mechanism in the meantime.
    #[must_use]
    pub fn from_env() -> Self {
        let reduce_motion = std::env::var("EDEN_REDUCE_MOTION")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        Self { reduce_motion }
    }

    /// Returns the config to use for a transition, downgrading to the fast
    /// [`SpringConfig::REDUCED`] response when reduce-motion is active.
    #[must_use]
    pub fn resolve(self, normal: SpringConfig) -> SpringConfig {
        if self.reduce_motion {
            SpringConfig::REDUCED
        } else {
            normal
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_swaps_config_under_reduce_motion() {
        let normal = SpringConfig::DEFAULT;
        assert_eq!(MotionPrefs { reduce_motion: false }.resolve(normal), normal);
        assert_eq!(
            MotionPrefs { reduce_motion: true }.resolve(normal),
            SpringConfig::REDUCED
        );
    }
}
