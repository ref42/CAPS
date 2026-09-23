#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusicMode {
    Normal,
    Quiet,
    Silent,
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
    use super::PlayOrder;

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
