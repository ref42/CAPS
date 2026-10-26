//! Session-only playback priority for accepted live-chat requests.
use crate::track::Track;
use std::collections::VecDeque;

#[derive(Clone, PartialEq, Eq)]
struct Key(String, String);

impl Key {
    fn of(track: &Track) -> Self {
        Self(track.source.clone(), track.id.clone())
    }
    fn index(&self, queue: &[Track]) -> Option<usize> {
        queue
            .iter()
            .position(|track| self.0 == track.source && self.1 == track.id)
    }
}

#[derive(Default)]
pub struct Requests {
    current: Option<Key>,
    pending: VecDeque<Key>,
    resume: Option<Key>,
}

impl Requests {
    /// Start immediately over ordinary playback; protect an active chat song.
    pub fn accept(
        &mut self,
        queue: &mut Vec<Track>,
        track: Track,
        playing: Option<&Track>,
    ) -> (usize, bool, bool) {
        let key = Key::of(&track);
        let (index, added) = match key.index(queue) {
            Some(index) => (index, false),
            None => {
                queue.push(track);
                (queue.len() - 1, true)
            }
        };
        if self.current.as_ref() == Some(&key)
            || (self.current.is_some() && self.pending.contains(&key))
        {
            return (index, added, false);
        }
        if self.current.is_some() {
            self.pending.push_back(key);
            (index, added, false)
        } else {
            self.resume = playing.map(Key::of).filter(|previous| *previous != key);
            if !self.pending.contains(&key) {
                self.pending.push_back(key);
            }
            let next = self
                .advance(queue)
                .expect("the accepted track is in the queue");
            (next, added, true)
        }
    }

    /// Resolve by identity so queue edits cannot redirect a request to another song.
    /// Requests override shuffle/repeat-one; afterwards restart the interrupted song.
    pub fn advance(&mut self, queue: &[Track]) -> Option<usize> {
        self.current = None;
        while let Some(key) = self.pending.pop_front() {
            if let Some(index) = key.index(queue) {
                self.current = Some(key);
                return Some(index);
            }
        }
        self.resume.take().and_then(|key| key.index(queue))
    }

    pub fn manual_play(&mut self, track: &Track) {
        let key = Key::of(track);
        let requested = self.current.as_ref() == Some(&key) || self.pending.contains(&key);
        self.pending.retain(|pending| *pending != key);
        self.current = requested.then_some(key);
        if !requested {
            self.resume = None;
        }
    }

    pub fn remove(&mut self, track: &Track) {
        let key = Key::of(track);
        self.pending.retain(|pending| *pending != key);
        if self.resume.as_ref() == Some(&key) {
            self.resume = None;
        }
    }

    /// A room change or request toggle drops waiting priority, not current audio.
    pub fn cancel_waiting(&mut self) {
        self.pending.clear();
        self.resume = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn track(id: &str) -> Track {
        Track {
            source: "qqmusic".into(),
            id: id.into(),
            name: id.into(),
            artist: String::new(),
            media_id: String::new(),
            stream_url: String::new(),
            album: String::new(),
            cover: String::new(),
            duration: None,
        }
    }

    #[test]
    fn interrupts_regular_song_then_serves_requests_fifo_and_returns_to_regular_song() {
        let mut requests = Requests::default();
        let regular = track("regular");
        let mut queue = vec![regular.clone(), track("other")];
        assert_eq!(
            requests.accept(&mut queue, track("first"), Some(&regular)),
            (2, true, true)
        );
        // The current marker is set before loading, protecting loading/paused requests too.
        assert_eq!(
            requests.accept(&mut queue, track("second"), Some(&track("first"))),
            (3, true, false)
        );
        assert_eq!(
            requests.accept(&mut queue, track("third"), Some(&track("first"))),
            (4, true, false)
        );
        assert_eq!(requests.advance(&queue), Some(3));
        assert_eq!(requests.advance(&queue), Some(4));
        assert_eq!(requests.advance(&queue), Some(0));
        assert_eq!(requests.advance(&queue), None);
    }

    #[test]
    fn existing_queue_entries_get_priority_without_duplicates_or_restarting_active_request() {
        let mut requests = Requests::default();
        let mut queue = vec![track("regular"), track("first"), track("second")];
        assert_eq!(
            requests.accept(&mut queue, track("first"), Some(&track("regular"))),
            (1, false, true)
        );
        assert_eq!(
            requests.accept(&mut queue, track("first"), Some(&track("first"))),
            (1, false, false)
        );
        assert_eq!(
            requests.accept(&mut queue, track("second"), Some(&track("first"))),
            (2, false, false)
        );
        assert_eq!(
            requests.accept(&mut queue, track("second"), Some(&track("first"))),
            (2, false, false)
        );
        assert_eq!(queue.len(), 3);
        assert_eq!(requests.advance(&queue), Some(2));
        assert_eq!(requests.advance(&queue), Some(0));
    }

    #[test]
    fn removals_and_reindexing_do_not_play_the_wrong_track() {
        let mut requests = Requests::default();
        let mut queue = vec![track("regular")];
        requests.accept(&mut queue, track("first"), Some(&track("regular")));
        requests.accept(&mut queue, track("removed"), Some(&track("first")));
        requests.accept(&mut queue, track("last"), Some(&track("first")));
        requests.remove(&queue[2]);
        queue.remove(2);
        requests.remove(&queue[0]);
        queue.remove(0);
        assert_eq!(requests.advance(&queue), Some(1));
        assert_eq!(requests.advance(&queue), None);
    }

    #[test]
    fn manual_selection_consumes_request_and_room_change_cancels_waiting_priority() {
        let mut requests = Requests::default();
        let mut queue = vec![];
        assert_eq!(
            requests.accept(&mut queue, track("first"), None),
            (0, true, true)
        );
        requests.accept(&mut queue, track("second"), Some(&track("first")));
        requests.manual_play(&track("second"));
        assert!(
            !requests
                .accept(&mut queue, track("third"), Some(&track("second")))
                .2
        );
        assert_eq!(requests.advance(&queue), Some(2));
        assert_eq!(requests.advance(&queue), None);
        requests.accept(&mut queue, track("fourth"), None);
        requests.accept(&mut queue, track("fifth"), Some(&track("fourth")));
        requests.cancel_waiting();
        assert_eq!(requests.advance(&queue), None);
    }

    #[test]
    fn manual_regular_playback_does_not_let_new_requests_overtake_waiting_requests() {
        let mut requests = Requests::default();
        let mut queue = vec![track("regular")];
        requests.accept(&mut queue, track("first"), None);
        requests.accept(&mut queue, track("second"), Some(&track("first")));
        requests.manual_play(&track("regular"));
        assert_eq!(
            requests.accept(&mut queue, track("third"), Some(&track("regular"))),
            (2, true, true)
        );
        assert_eq!(requests.advance(&queue), Some(3));
        assert_eq!(requests.advance(&queue), Some(0));
    }
}
