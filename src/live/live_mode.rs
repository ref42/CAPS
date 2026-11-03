//! Bounded live chat and song-request jobs; dropping a session cancels both workers.
use crate::{
    app::actions,
    live::bilibili_live::{self, LiveEvent},
    music::track::Track,
};
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};

#[derive(Clone)]
enum ConnectionState {
    Status(String),
    Connected(u64),
}

pub enum Event {
    Status(String),
    Connected(u64),
    Chat {
        user: String,
        text: String,
        emotes: Vec<bilibili_live::danmu::Emote>,
        kind: bilibili_live::activity::Kind,
        author: bilibili_live::activity::Author,
    },
    DeleteSuperChats(Vec<String>),
    Requested {
        epoch: u64,
        query: String,
        result: Result<Track, String>,
    },
}

pub struct Session {
    target: (u64, Option<String>),
    connected_room: Arc<AtomicU64>,
    state: watch::Receiver<Option<ConnectionState>>,
    _state_sender: watch::Sender<Option<ConnectionState>>,
    rx: mpsc::Receiver<Event>,
    requests: mpsc::Sender<(u64, String)>,
    epoch: Arc<AtomicU64>,
    connection: JoinHandle<()>,
    search: JoinHandle<()>,
    gate: RequestGate,
}

impl Session {
    pub fn start(room: u64, proxy: Option<String>) -> Self {
        let target = (room, proxy.clone());
        let connected_room = Arc::new(AtomicU64::new(0));
        let ready = connected_room.clone();
        let (state_tx, state) = watch::channel(None);
        let state_sender = state_tx.clone();
        let (tx, rx) = mpsc::channel(128);
        let (requests, mut pending) = mpsc::channel::<(u64, String)>(8);
        let epoch = Arc::new(AtomicU64::new(0));
        let search_epoch = epoch.clone();
        let search_tx = tx.clone();
        let search = tokio::spawn(async move {
            while let Some((epoch, query)) = pending.recv().await {
                if epoch != search_epoch.load(Ordering::Relaxed) {
                    continue;
                }
                let result = match tokio::time::timeout(
                    Duration::from_secs(20),
                    actions::search(query.clone()),
                )
                .await
                {
                    Ok(Ok(tracks)) => {
                        choose_track(&query, tracks).ok_or_else(|| "No matching song found.".into())
                    }
                    Ok(Err(err)) => Err(err),
                    Err(_) => Err("Song search timed out.".into()),
                };
                if epoch == search_epoch.load(Ordering::Relaxed)
                    && search_tx
                        .send(Event::Requested {
                            epoch,
                            query,
                            result,
                        })
                        .await
                        .is_err()
                {
                    break;
                }
            }
        });
        let connection = tokio::spawn(async move {
            let mut resolved = room;
            let result = bilibili_live::listen(room, proxy, |event| {
                let event = match event {
                    LiveEvent::Status(text) => {
                        state_tx.send_replace(Some(ConnectionState::Status(text)));
                        None
                    }
                    LiveEvent::Connected(room) => {
                        resolved = room;
                        state_tx.send_replace(Some(ConnectionState::Connected(room)));
                        ready.store(room, Ordering::Release);
                        None
                    }
                    LiveEvent::Heartbeat => None,
                    LiveEvent::Command(command) => {
                        for activity in bilibili_live::activity::events(&command, resolved) {
                            let event = match activity {
                                bilibili_live::activity::Event::Message(message) => Event::Chat {
                                    user: message.user, text: message.text, emotes: message.emotes, kind: message.kind, author: message.author,
                                },
                                bilibili_live::activity::Event::DeleteSuperChats(ids) => Event::DeleteSuperChats(ids),
                            };
                            let _ = tx.try_send(event);
                        }
                        None
                    }
                };
                // Never let a busy room block heartbeats or allocate an unbounded backlog.
                if let Some(event) = event {
                    let _ = tx.try_send(event);
                }
            })
            .await;
            if let Err(err) = result {
                state_tx.send_replace(Some(ConnectionState::Status(err.to_string())));
            }
        });
        Self {
            target,
            connected_room,
            state,
            _state_sender: state_sender,
            rx,
            requests,
            epoch,
            connection,
            search,
            gate: RequestGate::default(),
        }
    }

    pub fn poll(&mut self) -> Option<Event> {
        if let Some(event) = self.poll_state() {
            return Some(event);
        }
        while let Ok(event) = self.rx.try_recv() {
            if let Event::Requested { epoch, .. } = &event
                && *epoch != self.epoch.load(Ordering::Relaxed)
            {
                continue;
            }
            return Some(event);
        }
        None
    }

    fn poll_state(&mut self) -> Option<Event> {
        if !self.state.has_changed().unwrap_or(false) {
            return None;
        }
        self.state
            .borrow_and_update()
            .clone()
            .map(|state| match state {
                ConnectionState::Status(text) => Event::Status(text),
                ConnectionState::Connected(room) => Event::Connected(room),
            })
    }

    pub fn cancel_requests(&mut self) {
        self.epoch.fetch_add(1, Ordering::Relaxed);
        self.gate = RequestGate::default();
    }

    pub fn request(&mut self, query: String) -> Result<(), &'static str> {
        let now = Instant::now();
        if !self.gate.allows(&query, now) {
            return Err("Request skipped: repeated or too soon.");
        }
        self.requests
            .try_send((self.epoch.load(Ordering::Relaxed), query.clone()))
            .map_err(|_| "Song request queue is full; try again shortly.")?;
        self.gate.record(query, now);
        Ok(())
    }
}

/// Keep the active room receiving until its replacement has authenticated.
/// The pending room's chat queue is untouched until promotion, including its first messages.
#[derive(Default)]
pub struct Rooms {
    active: Option<Session>,
    pending: Option<Session>,
}

impl Rooms {
    pub fn has_session(&self) -> bool {
        self.active.is_some() || self.pending.is_some()
    }

    pub fn switching(&self) -> bool {
        self.pending.is_some()
    }

    pub fn connect(&mut self, room: u64, proxy: Option<String>) -> bool {
        let target = (room, proxy.clone());
        if self
            .pending
            .as_ref()
            .is_some_and(|session| session.target == target)
        {
            return false;
        }
        if self
            .active
            .as_ref()
            .is_some_and(|session| session.target == target)
        {
            let cancelled = self.pending.take().is_some();
            return cancelled;
        }
        self.cancel_requests();
        self.pending = Some(Session::start(room, proxy));
        true
    }

    pub fn poll(&mut self) -> Option<Event> {
        if let Some(pending) = self.pending.as_mut() {
            let room = pending.connected_room.load(Ordering::Acquire);
            if room != 0 {
                self.active = self.pending.take();
                return Some(Event::Connected(room));
            }
            // Control messages have their own channel, so a full chat buffer cannot
            // hide successful authentication. Never drain the pending chat queue here.
            if let Some(event) = pending.poll_state() {
                if let Event::Connected(room) = event {
                    self.active = self.pending.take();
                    return Some(Event::Connected(room));
                }
                return Some(event);
            }
        }
        while let Some(event) = self.active.as_mut().and_then(Session::poll) {
            if self.switching() && !matches!(event, Event::Chat { .. } | Event::DeleteSuperChats(_)) {
                continue;
            }
            return Some(event);
        }
        None
    }

    pub fn cancel_requests(&mut self) {
        for session in [&mut self.active, &mut self.pending].into_iter().flatten() {
            session.cancel_requests();
        }
    }

    pub fn request(&mut self, query: String) -> Result<(), &'static str> {
        if self.switching() {
            return Err("Room switch in progress.");
        }
        self.active.as_mut().ok_or("Not connected.")?.request(query)
    }

    pub fn active_room(&self) -> Option<u64> {
        self.active
            .as_ref()
            .map(|session| session.connected_room.load(Ordering::Acquire))
            .filter(|room| *room != 0)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.connection.abort();
        self.search.abort();
    }
}

pub fn song_request(text: &str) -> Option<String> {
    let rest = text.trim().strip_prefix("点歌")?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let query = rest.split_whitespace().collect::<Vec<_>>().join(" ");
    if query.is_empty() || query.chars().count() > 160 {
        return None;
    }
    Some(query)
}

fn normalized(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Honor platform priority, then prefer a title/artist match within that group.
fn choose_track(query: &str, tracks: Vec<Track>) -> Option<Track> {
    let parsed = crate::music::search_query::SearchQuery::parse(query).ok()?;
    let query = normalized(&parsed.text);
    let mut best = None;
    let mut score = (false, 0);
    for track in tracks {
        let title = normalized(&track.name);
        let artist = normalized(&track.artist);
        let rank = if !title.is_empty() && query == title {
            100
        } else if !title.is_empty()
            && !artist.is_empty()
            && (query == format!("{title}{artist}") || query == format!("{artist}{title}"))
        {
            110
        } else if !title.is_empty()
            && query.contains(&title)
            && !artist.is_empty()
            && query.contains(&artist)
        {
            90
        } else {
            1
        };
        let rank = (parsed.prefers(&track), rank);
        if best.is_none() || rank > score {
            score = rank;
            best = Some(track);
        }
    }
    best
}

#[derive(Default)]
struct RequestGate {
    recent: VecDeque<(String, Instant)>,
}
impl RequestGate {
    fn allows(&self, query: &str, now: Instant) -> bool {
        !self
            .recent
            .back()
            .is_some_and(|(_, time)| now.duration_since(*time) < Duration::from_secs(3))
            && !self.recent.iter().any(|(old, time)| {
                normalized(old) == normalized(query)
                    && now.duration_since(*time) < Duration::from_secs(60)
            })
    }
    fn record(&mut self, query: String, now: Instant) {
        while self
            .recent
            .front()
            .is_some_and(|(_, time)| now.duration_since(*time) >= Duration::from_secs(60))
        {
            self.recent.pop_front();
        }
        self.recent.push_back((query, now));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(room: u64) -> (Session, mpsc::Sender<Event>) {
        let (tx, rx) = mpsc::channel(128);
        let (requests, _) = mpsc::channel(1);
        let (state_tx, state) = watch::channel(None);
        (
            Session {
                target: (room, None),
                connected_room: Arc::new(AtomicU64::new(0)),
                state,
                _state_sender: state_tx,
                rx,
                requests,
                epoch: Arc::new(AtomicU64::new(0)),
                connection: tokio::spawn(std::future::pending()),
                search: tokio::spawn(std::future::pending()),
                gate: RequestGate::default(),
            },
            tx,
        )
    }

    fn chat(text: &str) -> Event {
        Event::Chat {
            user: "viewer".into(),
            text: text.into(),
            emotes: Vec::new(),
            kind: bilibili_live::activity::Kind::Danmu,
            author: bilibili_live::activity::Author::default(),
        }
    }

    #[tokio::test]
    async fn handoff_keeps_old_room_until_ready_and_preserves_first_new_messages() {
        let (old, old_tx) = session(1);
        old.connected_room.store(1, Ordering::Release);
        let (new, new_tx) = session(2);
        let ready = new.connected_room.clone();
        let old_worker = old.connection.abort_handle();
        let mut rooms = Rooms {
            active: Some(old),
            pending: Some(new),
        };
        old_tx.try_send(chat("old while connecting")).unwrap();
        new_tx.try_send(chat("first new message")).unwrap();
        new_tx.try_send(chat("second new message")).unwrap();
        assert!(
            matches!(rooms.poll(), Some(Event::Chat { text, .. }) if text == "old while connecting")
        );
        // The pending room's messages have not been consumed.
        assert!(rooms.poll().is_none());
        assert_eq!(rooms.active_room(), Some(1));
        old_tx.try_send(chat("stale old message")).unwrap();
        ready.store(2, Ordering::Release);
        assert!(matches!(rooms.poll(), Some(Event::Connected(2))));
        assert_eq!(rooms.active_room(), Some(2));
        assert!(
            matches!(rooms.poll(), Some(Event::Chat { text, .. }) if text == "first new message")
        );
        assert!(
            matches!(rooms.poll(), Some(Event::Chat { text, .. }) if text == "second new message")
        );
        assert!(rooms.poll().is_none());
        tokio::task::yield_now().await;
        assert!(old_worker.is_finished());
    }

    #[tokio::test]
    async fn full_chat_buffer_cannot_drop_room_authentication() {
        let (new, tx) = session(2);
        for index in 0..128 {
            tx.try_send(chat(&index.to_string())).unwrap();
        }
        new._state_sender
            .send_replace(Some(ConnectionState::Connected(2)));
        new.connected_room.store(2, Ordering::Release);
        let mut rooms = Rooms {
            active: None,
            pending: Some(new),
        };
        assert!(matches!(rooms.poll(), Some(Event::Connected(2))));
        let mut count = 0;
        while let Some(event) = rooms.poll() {
            if matches!(event, Event::Chat { .. }) {
                count += 1;
            }
        }
        assert_eq!(count, 128);
    }

    #[tokio::test]
    async fn repeated_apply_does_not_restart_and_switching_back_keeps_old_connection() {
        let (old, _) = session(1);
        old.connected_room.store(1, Ordering::Release);
        let old_worker = old.connection.abort_handle();
        let (new, _) = session(2);
        let pending_worker = new.connection.abort_handle();
        let mut rooms = Rooms {
            active: Some(old),
            pending: Some(new),
        };
        assert!(!rooms.connect(2, None));
        assert!(!pending_worker.is_finished());
        assert!(rooms.connect(1, None));
        assert!(!rooms.switching());
        assert_eq!(rooms.active_room(), Some(1));
        assert!(!rooms.connect(1, None));
        tokio::task::yield_now().await;
        assert!(pending_worker.is_finished());
        assert!(!old_worker.is_finished());
        drop(rooms);
        tokio::task::yield_now().await;
        assert!(old_worker.is_finished());
    }

    #[tokio::test]
    async fn switching_cancels_old_song_results_and_disconnect_cancels_both_rooms() {
        let (old, tx) = session(1);
        let old_worker = old.connection.abort_handle();
        let (new, _) = session(2);
        let pending_worker = new.connection.abort_handle();
        let mut rooms = Rooms {
            active: Some(old),
            pending: Some(new),
        };
        tx.try_send(Event::Requested {
            epoch: 0,
            query: "old".into(),
            result: Err("old result".into()),
        })
        .unwrap();
        rooms.cancel_requests();
        assert!(rooms.request("new".into()).is_err());
        assert!(rooms.poll().is_none());
        drop(rooms);
        tokio::task::yield_now().await;
        assert!(old_worker.is_finished());
        assert!(pending_worker.is_finished());
    }

    #[test]
    fn parses_title_and_optional_artist_without_guessing_spaces() {
        assert_eq!(song_request("点歌 晴天"), Some("晴天".into()));
        assert_eq!(
            song_request(" 点歌  晴天  周杰伦 "),
            Some("晴天 周杰伦".into())
        );
        assert_eq!(
            song_request("点歌 Never Gonna Give You Up Rick Astley"),
            Some("Never Gonna Give You Up Rick Astley".into())
        );
        for text in ["点歌", "我想点歌 晴天", "点歌台", "点歌   "] {
            assert!(song_request(text).is_none());
        }
        assert!(song_request(&format!("点歌 {}", "a".repeat(161))).is_none());
    }
    #[test]
    fn limits_bursts_and_repeated_queries() {
        let now = Instant::now();
        let mut gate = RequestGate::default();
        gate.record("晴天 周杰伦".into(), now);
        assert!(!gate.allows("另一首", now + Duration::from_secs(2)));
        assert!(gate.allows("另一首", now + Duration::from_secs(3)));
        assert!(!gate.allows("晴天  周杰伦", now + Duration::from_secs(59)));
        assert!(gate.allows("晴天 周杰伦", now + Duration::from_secs(60)));
    }



    #[test]
    fn artist_match_beats_a_title_only_match() {
        let track = |artist: &str| Track {
            source: "netease".into(),
            id: artist.into(),
            media_id: String::new(),
            stream_url: String::new(),
            name: "晴天".into(),
            artist: artist.into(),
            album: String::new(),
            cover: String::new(),
            duration: None,
        };
        assert_eq!(
            choose_track("晴天 周杰伦", vec![track("翻唱"), track("周杰伦")])
                .unwrap()
                .artist,
            "周杰伦"
        );
        assert!(choose_track("晴天", vec![]).is_none());
    }
    #[test]
    fn song_requests_honor_platform_then_artist_and_fall_back_to_other_sources() {
        let track = |source: &str, name: &str, artist: &str| Track {
            source: source.into(),
            id: format!("{source}:{name}:{artist}"),
            media_id: String::new(),
            stream_url: String::new(),
            name: name.into(),
            artist: artist.into(),
            album: String::new(),
            cover: String::new(),
            duration: None,
        };
        for platform in ["qq", "QQ音乐", "platform:qq", "@qq"] {
            let query = song_request(&format!("点歌 晴天 周杰伦 {platform}")).unwrap();
            let selected = choose_track(&query, vec![
                track("netease", "晴天", "周杰伦"),
                track("qqmusic", "晴天", "翻唱"),
                track("qqmusic", "晴天", "周杰伦"),
            ]).unwrap();
            assert_eq!(selected.source, "qqmusic");
            assert_eq!(selected.artist, "周杰伦");
        }
        let selected = choose_track("晴天 周杰伦 qq", vec![
            track("netease", "晴天", "周杰伦"),
            track("qqmusic", "晴天", "翻唱"),
        ]).unwrap();
        assert_eq!(selected.source, "qqmusic");
        let selected = choose_track("晴天 qq", vec![
            track("qqmusic", "晴天 qq", "翻唱"),
            track("qqmusic", "晴天", "周杰伦"),
        ]).unwrap();
        assert_eq!(selected.name, "晴天");
        let selected = choose_track("晴天 周杰伦 qq", vec![
            track("kugou", "晴天", "翻唱"),
            track("netease", "晴天", "周杰伦"),
        ]).unwrap();
        assert_eq!(selected.artist, "周杰伦");
        let selected = choose_track("晴天 neteast", vec![
            track("qqmusic", "晴天", "周杰伦"),
            track("shitease", "晴天", "周杰伦"),
        ]).unwrap();
        assert_eq!(selected.source, "shitease");
    }


    #[tokio::test]
    async fn disabling_requests_discards_already_completed_results() {
        let (mut session, tx) = session(1);
        tx.send(Event::Requested {
            epoch: 0,
            query: "old".into(),
            result: Err("old result".into()),
        })
        .await
        .unwrap();
        session.cancel_requests();
        assert!(session.poll().is_none());
    }
}
