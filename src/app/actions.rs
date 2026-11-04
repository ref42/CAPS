//! UI-independent music jobs. Progress crosses into the native UI through messages.
use crate::music::lyrics::LyricLine;
use crate::music::track::*;
use crate::{
    app::storage, music::local_music, providers::kugou, providers::qishui, providers::qqmusic,
    providers::shitease,
};
use std::{collections::HashSet, sync::Arc};

#[derive(Clone)]
pub struct Progress(Arc<dyn Fn(String) + Send + Sync>);
impl Progress {
    pub fn new(callback: impl Fn(String) + Send + Sync + 'static) -> Self {
        Self(Arc::new(callback))
    }
    pub fn set(&mut self, text: String) {
        (self.0)(text);
    }
}

pub async fn search(text: String) -> Result<Vec<Track>, String> {
    let query = crate::music::search_query::SearchQuery::parse(&text)?;
    let text = query.text.clone();
    let (a, b, c, d) = tokio::join!(
        shitease::search_shitease_songs(text.to_owned(), Some(150), None),
        qqmusic::search(text.to_owned(), 150),
        kugou::search(text.clone(), 150),
        qishui::search(text, 100)
    );
    let mut tracks = Vec::new();
    if let Ok(items) = a {
        tracks.extend(items.into_iter().map(Track::from));
    }
    if let Ok(items) = b {
        tracks.extend(items.into_iter().map(Track::from));
    }
    if let Ok(items) = c {
        tracks.extend(items.into_iter().map(Track::from));
    }
    if let Ok(items) = d {
        tracks.extend(items);
    }
    if tracks.is_empty() {
        Err("Online music search is unavailable.".into())
    } else {
        query.rank(&mut tracks);
        Ok(tracks)
    }
}

pub async fn random(count: u32) -> Result<Vec<Track>, String> {
    let count = count.clamp(1, 999);
    let (a, b, c, d) = tokio::join!(
        shitease::random_shitease_queue(Some(count), None),
        qqmusic::search_random(count),
        kugou::search_random(count),
        qishui::random(count)
    );
    let providers: Vec<Vec<Track>> = vec![
        a.unwrap_or_default().into_iter().map(Track::from).collect(),
        b.unwrap_or_default().into_iter().map(Track::from).collect(),
        c.unwrap_or_default().into_iter().map(Track::from).collect(),
        d.unwrap_or_default(),
    ];
    let mut tracks = Vec::new();
    for index in 0..count as usize {
        for provider in &providers {
            if tracks.len() == count as usize {
                break;
            }
            if let Some(track) = provider.get(index) {
                tracks.push(track.to_owned());
            }
        }
    }
    if tracks.is_empty() {
        Err("Online random music is unavailable.".into())
    } else {
        Ok(tracks)
    }
}

/// Explicitly adding a song promotes its existing entry, preserving metadata.
/// Returns the previous index so playback and shuffle cursors can follow it.
pub fn move_or_append_track(queue: &mut Vec<Track>, track: Track) -> Option<usize> {
    let previous = queue
        .iter()
        .position(|queued| queued.source == track.source && queued.id == track.id);
    let track = previous.map(|index| queue.remove(index)).unwrap_or(track);
    queue.push(track);
    previous
}

pub fn index_after_move_to_end(index: usize, moved: usize, last: usize) -> usize {
    if index == moved {
        last
    } else if index > moved {
        index - 1
    } else {
        index
    }
}

pub fn append_unique_tracks(
    queue: &mut Vec<Track>,
    tracks: impl IntoIterator<Item = Track>,
) -> usize {
    let mut seen = queue
        .iter()
        .map(|track| (track.source.to_owned(), track.id.to_owned()))
        .collect::<HashSet<_>>();
    let mut added = 0;
    for track in tracks {
        let key = (track.source.to_owned(), track.id.to_owned());
        if seen.insert(key) {
            queue.push(track);
            added += 1;
        }
    }
    added
}

pub async fn load_track_audio(
    track: &Track,
    mut status: Progress,
) -> Result<crate::music::audio::LoadedAudio, String> {
    use crate::{
        music::audio::LoadedAudio,
        providers::streaming::{RemoteMedia, media_client},
    };
    if track.source == SOURCE_LOCAL {
        if !std::path::Path::new(&track.id).is_file() {
            return Err("Local audio file is not available.".into());
        }
        return Ok(LoadedAudio::File(track.id.clone()));
    }
    let path = storage::song_cache_file(&track.source, &track.id)
        .ok_or("Song cache path is not available.")?;
    if path.is_file() && path.metadata().is_ok_and(|meta| meta.len() > 0) {
        return Ok(LoadedAudio::File(path.to_string_lossy().into_owned()));
    }
    tokio::fs::create_dir_all(path.parent().ok_or("Invalid cache path")?)
        .await
        .map_err(|e| e.to_string())?;
    status.set(format!("Resolving audio: {}…", track.name));
    let remote = match track.source.as_str() {
        SOURCE_QISHUI => qishui::resolve_audio(&track.id).await?,
        SOURCE_QQMUSIC => RemoteMedia {
            client: media_client("https://y.qq.com/")?,
            urls: vec![qqmusic::stream_url_with_media(&track.id, &track.media_id).await?],
        },
        SOURCE_KUGOU => {
            let (quality, album, audio) = kugou_metadata(track);
            RemoteMedia {
                client: media_client("https://www.kugou.com/")?,
                urls: vec![kugou::stream_url(&track.id, quality, album, audio).await?],
            }
        }
        SOURCE_NETEASE | SOURCE_SHITEASE => {
            let info =
                shitease::get_shitease_song_url(track.id.clone(), Some("exhigh".into()), None)
                    .await?;
            RemoteMedia {
                client: media_client("https://music.163.com/")?,
                urls: vec![
                    info.url
                        .filter(|u| !u.is_empty())
                        .ok_or("No playable stream for this track.")?,
                ],
            }
        }
        _ => return Err(format!("Unsupported music source: {}", track.source)),
    };
    status.set(format!("Buffering audio: {}…", track.name));
    remote.prepare(&path).await.map(LoadedAudio::Stream)
}

fn kugou_metadata(track: &Track) -> (&str, &str, &str) {
    let mut parts = track.media_id.split('|');
    (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
    )
}

pub async fn load_track_lyrics(track: &Track) -> Vec<LyricLine> {
    if track.source == SOURCE_QISHUI {
        return qishui::lyric(&track.id)
            .await
            .map(|text| crate::music::lyrics::parse_lrc(&text))
            .unwrap_or_default();
    }
    if track.source == SOURCE_LOCAL {
        let id = track.id.to_owned();
        let text = tokio::task::spawn_blocking(move || local_music::read_lyrics(&id))
            .await
            .unwrap_or_default();
        return crate::music::lyrics::parse_lrc(&text);
    }
    if track.source == SOURCE_QQMUSIC {
        return qqmusic::lyric(&track.id)
            .await
            .map(|text| crate::music::lyrics::parse_lrc(&text))
            .unwrap_or_default();
    }
    if track.source == SOURCE_KUGOU {
        if let Ok(text) = kugou::lyric(&track.id, &track.name, &track.artist, track.duration).await
        {
            return crate::music::lyrics::parse_lrc(&text);
        }
        return crate::music::lyric_finder::find(&track.name, &track.artist, track.duration)
            .await
            .map(|text| crate::music::lyrics::parse_lrc(&text))
            .unwrap_or_default();
    }
    if track.source == SOURCE_NETEASE || track.source == SOURCE_SHITEASE {
        return shitease::get_shitease_lyric(track.id.to_owned(), None)
            .await
            .map(|response| {
                crate::music::lyrics::parse_lrc(response.lyric.as_deref().unwrap_or_default())
            })
            .unwrap_or_default();
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(source: &str, id: &str) -> Track {
        serde_json::from_value(serde_json::json!({
            "source": source, "id": id, "name": "Same title", "artist": "Artist",
            "album": "Saved album", "cover": "", "duration": 180
        }))
        .unwrap()
    }

    #[test]
    fn readding_promotes_existing_entry_preserving_metadata_and_other_sources() {
        let mut queue = vec![
            track(SOURCE_QQMUSIC, "a"),
            track(SOURCE_QQMUSIC, "b"),
            track(SOURCE_NETEASE, "a"),
        ];
        let original = queue[0].clone();
        let mut fresh = original.clone();
        fresh.album.clear();
        assert_eq!(move_or_append_track(&mut queue, fresh), Some(0));
        assert_eq!(
            queue,
            vec![
                track(SOURCE_QQMUSIC, "b"),
                track(SOURCE_NETEASE, "a"),
                original.clone()
            ]
        );
        assert_eq!(move_or_append_track(&mut queue, original.clone()), Some(2));
        assert_eq!(queue.len(), 3);
        assert_eq!(
            move_or_append_track(&mut queue, track(SOURCE_KUGOU, "a")),
            None
        );
        assert_eq!(queue.len(), 4);
    }

    #[test]
    fn moving_any_row_preserves_playback_and_history_track_identity() {
        let original = vec![
            track(SOURCE_QQMUSIC, "a"),
            track(SOURCE_QQMUSIC, "b"),
            track(SOURCE_QQMUSIC, "c"),
        ];
        for moved in 0..original.len() {
            let mut queue = original.clone();
            move_or_append_track(&mut queue, original[moved].clone());
            for (index, track) in original.iter().enumerate() {
                assert_eq!(
                    &queue[index_after_move_to_end(index, moved, queue.len() - 1)],
                    track
                );
            }
        }
    }

    #[tokio::test]
    #[ignore = "requires live music provider access"]
    async fn live_search_keeps_other_providers_after_preferred_results() {
        let tracks = super::search("晴天 周杰伦 qq".into()).await.unwrap();
        let counts = tracks
            .iter()
            .fold(std::collections::BTreeMap::new(), |mut counts, track| {
                *counts.entry(track.source.clone()).or_insert(0) += 1;
                counts
            });
        println!("Results per provider: {counts:?}");
        assert!(
            counts.contains_key(crate::music::track::SOURCE_QQMUSIC),
            "QQ provider did not respond"
        );
        assert!(counts.len() > 1, "Other providers did not respond");
        let mut passed_preferred = false;
        for track in &tracks {
            if track.source != crate::music::track::SOURCE_QQMUSIC {
                passed_preferred = true;
            } else {
                assert!(!passed_preferred);
            }
        }
    }
}
