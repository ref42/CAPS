#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusicMode {
    Normal,
    Quiet,
    Silent,
}

impl MusicMode {
    /// The mode a track starts in when one is picked for us rather than by hand.
    ///
    /// Advancing to the next song is not a reason to undo a display choice the
    /// user made: Quiet has to survive from one track to the next, or it is a
    /// per-song toggle rather than a mode. Silent — stopped, with no track
    /// loaded — is the one state that starts over, because playing a track is
    /// exactly what the user just asked for.
    pub fn after_track_change(self) -> Self {
        match self {
            Self::Silent => Self::Normal,
            quiet => quiet,
        }
    }
}

/// What plays once the current track ends, and what the next button means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayOrder {
    /// Play the queue in order and stop at the end.
    Sequential,
    /// Play the queue in order and wrap around at the end.
    RepeatAll,
    /// Play the current track again.
    RepeatOne,
    /// Take the next track from a random place in the queue.
    Shuffle,
}

impl PlayOrder {
    /// The value kept in the settings file. A key rather than a number, so a
    /// state file written by another version still reads as something sensible.
    pub fn key(self) -> &'static str {
        match self {
            Self::Sequential => "sequential",
            Self::RepeatAll => "all",
            Self::RepeatOne => "one",
            Self::Shuffle => "shuffle",
        }
    }

    /// Anything unrecognised falls back to repeat-all, which is what the player
    /// did before the order was selectable.
    pub fn from_key(key: &str) -> Self {
        match key {
            "sequential" => Self::Sequential,
            "one" => Self::RepeatOne,
            "shuffle" => Self::Shuffle,
            _ => Self::RepeatAll,
        }
    }

    /// The order a click on the mode button moves to.
    pub fn next(self) -> Self {
        match self {
            Self::Sequential => Self::RepeatAll,
            Self::RepeatAll => Self::RepeatOne,
            Self::RepeatOne => Self::Shuffle,
            Self::Shuffle => Self::Sequential,
        }
    }

    /// Name of the order, used as the mode button's own text.
    pub fn label(self) -> (&'static str, &'static str) {
        match self {
            Self::Sequential => ("In order", "顺序播放"),
            Self::RepeatAll => ("Repeat all", "列表循环"),
            Self::RepeatOne => ("Repeat one", "单曲循环"),
            Self::Shuffle => ("Shuffle", "随机播放"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MusicMode, PlayOrder};

    #[test]
    fn quiet_survives_a_track_change() {
        assert_eq!(MusicMode::Quiet.after_track_change(), MusicMode::Quiet);
        assert_eq!(MusicMode::Normal.after_track_change(), MusicMode::Normal);
        // Silent means stopped: the next track starts over as Normal.
        assert_eq!(MusicMode::Silent.after_track_change(), MusicMode::Normal);
    }

    #[test]
    fn every_order_round_trips_through_its_key() {
        for order in [
            PlayOrder::Sequential,
            PlayOrder::RepeatAll,
            PlayOrder::RepeatOne,
            PlayOrder::Shuffle,
        ] {
            assert_eq!(PlayOrder::from_key(order.key()), order);
        }
    }

    #[test]
    fn an_unknown_key_keeps_the_old_behaviour() {
        assert_eq!(PlayOrder::from_key(""), PlayOrder::RepeatAll);
        assert_eq!(PlayOrder::from_key("nonsense"), PlayOrder::RepeatAll);
    }

    #[test]
    fn the_cycle_visits_all_four_orders() {
        let mut order = PlayOrder::Sequential;
        let mut seen = vec![order];
        for _ in 0..3 {
            order = order.next();
            seen.push(order);
        }
        assert_eq!(order.next(), PlayOrder::Sequential);
        for expected in [
            PlayOrder::Sequential,
            PlayOrder::RepeatAll,
            PlayOrder::RepeatOne,
            PlayOrder::Shuffle,
        ] {
            assert!(seen.contains(&expected));
        }
    }
}
