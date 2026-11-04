//! Evidence of recent activity, not an online roster: web chat has no reliable
//! per-viewer departure notification. Silence must never mean "left the room".
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Presence {
    seen: HashMap<u64, Instant>,
}

impl Presence {
    pub fn observe(&mut self, uid: u64, now: Instant) {
        if uid == 0 {
            return;
        }
        if self.seen.len() >= 1000
            && !self.seen.contains_key(&uid)
            && let Some(oldest) = self
                .seen
                .iter()
                .min_by_key(|(_, at)| **at)
                .map(|(uid, _)| *uid)
        {
            self.seen.remove(&oldest);
        }
        self.seen.insert(uid, now);
    }

    pub fn age(&self, uid: u64, now: Instant) -> Option<Duration> {
        self.seen
            .get(&uid)
            .map(|at| now.saturating_duration_since(*at))
    }

    pub fn clear(&mut self) {
        self.seen.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_stable_ids_refreshes_activity_and_resets_on_disconnect() {
        let mut presence = Presence::default();
        let now = Instant::now();
        presence.observe(0, now);
        assert_eq!(presence.age(0, now), None);
        presence.observe(42, now);
        assert_eq!(
            presence.age(42, now + Duration::from_secs(120)),
            Some(Duration::from_secs(120))
        );
        presence.observe(42, now + Duration::from_secs(120));
        assert_eq!(
            presence.age(42, now + Duration::from_secs(120)),
            Some(Duration::ZERO)
        );
        presence.clear();
        assert_eq!(presence.age(42, now), None);
    }

    #[test]
    fn bounds_memory_in_large_rooms() {
        let mut presence = Presence::default();
        let now = Instant::now();
        for uid in 1..=1100 {
            presence.observe(uid, now + Duration::from_millis(uid));
        }
        assert_eq!(presence.seen.len(), 1000);
        assert_eq!(presence.age(1, now), None);
        assert!(presence.age(1100, now).is_some());
    }
}
