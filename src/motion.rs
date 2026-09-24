//! Interruptible native equivalents of the capsule's CSS motion curves.
use std::time::{Duration, Instant};

pub fn should_collapse(
    inside: bool,
    dragging: bool,
    dialog: bool,
    left_at: Option<Instant>,
    now: Instant,
) -> bool {
    !inside
        && !dragging
        && !dialog
        && left_at
            .is_some_and(|left| now.saturating_duration_since(left) >= Duration::from_millis(100))
}

pub struct Motion {
    from: f32,
    to: f32,
    began: Instant,
    duration: Duration,
    forward_duration: Duration,
    reverse_duration: Duration,
}

impl Motion {
    pub fn with_durations(
        value: f32,
        forward_milliseconds: u64,
        reverse_milliseconds: u64,
    ) -> Self {
        Self {
            from: value,
            to: value,
            began: Instant::now(),
            duration: Duration::from_millis(forward_milliseconds),
            forward_duration: Duration::from_millis(forward_milliseconds),
            reverse_duration: Duration::from_millis(reverse_milliseconds),
        }
    }

    pub fn value(&self, now: Instant) -> f32 {
        // A zero duration is the reduced-motion case: land on the target now.
        if self.duration.is_zero() {
            return self.to;
        }
        let t = (now.saturating_duration_since(self.began).as_secs_f32()
            / self.duration.as_secs_f32())
        .clamp(0., 1.);
        self.from + (self.to - self.from) * ease(t)
    }

    /// Whether the value is still travelling. Used to decide if another frame
    /// is worth painting.
    pub fn animating(&self, now: Instant) -> bool {
        self.from != self.to
            && !self.duration.is_zero()
            && now.saturating_duration_since(self.began) < self.duration
    }

    /// Aim at a new value, keeping the current one so the motion can be
    /// reversed mid-flight. Returns whether the target actually changed.
    pub fn target(&mut self, target: f32, now: Instant) -> bool {
        if self.to == target {
            return false;
        }
        let current = self.value(now);
        self.from = current;
        self.to = target;
        self.began = now;
        self.duration = if target >= current {
            self.forward_duration
        } else {
            self.reverse_duration
        };
        true
    }
}

/// cubic-bezier(0.2, 0, 0, 1), with x inverted before evaluating y.
pub fn ease(t: f32) -> f32 {
    if t <= 0. {
        return 0.;
    }
    if t >= 1. {
        return 1.;
    }
    let (mut low, mut high) = (0., 1.);
    for _ in 0..16 {
        let u = (low + high) * 0.5;
        let x = 0.6 * (1. - u) * (1. - u) * u + u * u * u;
        if x < t {
            low = u;
        } else {
            high = u;
        }
    }
    let u = (low + high) * 0.5;
    3. * (1. - u) * u * u + u * u * u
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reversing_hover_preserves_current_geometry() {
        let start = Instant::now();
        let mut motion = Motion::with_durations(0., 260, 150);
        motion.target(1., start);
        let middle = start + Duration::from_millis(90);
        let before = motion.value(middle);
        assert!(before > 0. && before < 1.);
        motion.target(0., middle);
        assert_eq!(motion.value(middle), before);
        assert_eq!(motion.value(middle + Duration::from_millis(280)), 0.);
    }

    /// Reduced motion uses zero-length durations, which must land on the target
    /// immediately instead of dividing by zero.
    #[test]
    fn zero_duration_snaps_and_reports_no_motion() {
        let start = Instant::now();
        let mut motion = Motion::with_durations(0., 0, 0);
        assert!(motion.target(1., start));
        assert_eq!(motion.value(start), 1.);
        assert!(!motion.animating(start));
        assert!(motion.target(0., start));
        assert_eq!(motion.value(start), 0.);
    }

    #[test]
    fn retargeting_reports_only_real_changes() {
        let start = Instant::now();
        let mut motion = Motion::with_durations(0., 260, 150);
        assert!(motion.target(1., start));
        assert!(!motion.target(1., start));
        assert!(motion.animating(start));
    }
}
