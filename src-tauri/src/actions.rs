use crate::audio::{AudioCommand, AudioPlayer};
use crate::local_music;
use crate::lyrics::LyricLine;
use crate::netease;
use crate::track::{SOURCE_LOCAL, Track};
use dioxus::prelude::*;
use std::sync::Arc;

pub fn spawn_play(
    track: Track,
    player: Arc<AudioPlayer>,
    mut current_index: Signal<Option<usize>>,
    mut status: Signal<String>,
    mut lyrics: Signal<Vec<LyricLine>>,
) {
    spawn(async move {
        status.set(format!("Loading {}...", track.name));
        lyrics.set(Vec::new());
        let bytes = match load_track_bytes(&track).await {
            Ok(bytes) => bytes,
            Err(err) => {
                player.send(AudioCommand::Stop);
                current_index.set(None);
                status.set(err);
                return;
            }
        };
        player.send(AudioCommand::LoadBytes {
            bytes,
            title: track.name.clone(),
            detail: track.artist.clone(),
        });
        status.set(format!("Playing {}.", track.name));
        lyrics.set(load_track_lyrics(&track).await);
    });
}

pub fn spawn_search(text: String, mut results: Signal<Vec<Track>>, mut status: Signal<String>) {
    if text.is_empty() {
        status.set("Type a song name first.".to_string());
        return;
    }
    spawn(async move {
        status.set("Searching NetEase...".to_string());
        match netease::search_netease_songs(text, Some(18), None).await {
            Ok(items) => {
                let tracks = items.into_iter().map(Track::from).collect::<Vec<_>>();
                let count = tracks.len();
                results.set(tracks);
                status.set(format!("Found {count} tracks."));
            }
            Err(err) => status.set(err),
        }
    });
}

pub fn spawn_random_queue(
    count: u32,
    mut queue: Signal<Vec<Track>>,
    mut current_index: Signal<Option<usize>>,
    mut status: Signal<String>,
) {
    spawn(async move {
        status.set(format!("Loading random {count}..."));
        match netease::random_netease_queue(Some(count), None).await {
            Ok(items) => {
                let tracks = items.into_iter().map(Track::from).collect::<Vec<_>>();
                let loaded = tracks.len();
                queue.set(tracks);
                current_index.set(None);
                status.set(format!("Queued {loaded} random tracks."));
            }
            Err(err) => status.set(err),
        }
    });
}

pub fn spawn_load_local_queue(
    folder: String,
    mut queue: Signal<Vec<Track>>,
    mut status: Signal<String>,
) {
    if folder.trim().is_empty() {
        status.set("Set a local music folder first.".to_string());
        return;
    }
    spawn(async move {
        status.set("Loading local songs...".to_string());
        match local_music::load_all(&folder) {
            Ok(tracks) => {
                let loaded = tracks.len();
                if loaded == 0 {
                    status.set("No supported audio files found.".to_string());
                    return;
                }
                let mut next = queue.read().clone();
                next.extend(tracks);
                queue.set(next);
                status.set(format!("Queued {loaded} local tracks."));
            }
            Err(err) => status.set(err),
        }
    });
}

async fn load_track_bytes(track: &Track) -> Result<Vec<u8>, String> {
    if track.source == SOURCE_LOCAL {
        return local_music::read_audio(&track.id);
    }
    let info =
        netease::get_netease_song_url(track.id.clone(), Some("exhigh".to_string()), None).await?;
    let url = info
        .url
        .filter(|url| !url.is_empty())
        .ok_or_else(|| "No playable stream for this track.".to_string())?;
    let response = reqwest::get(&url)
        .await
        .map_err(|err| format!("Stream request failed: {err}"))?;
    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|err| format!("Stream read failed: {err}"))
}

async fn load_track_lyrics(track: &Track) -> Vec<LyricLine> {
    if track.source == SOURCE_LOCAL {
        return crate::lyrics::parse_lrc(&local_music::read_lyrics(&track.id));
    }
    netease::get_netease_lyric(track.id.clone(), None)
        .await
        .map(|response| crate::lyrics::parse_lrc(response.lyric.as_deref().unwrap_or_default()))
        .unwrap_or_default()
}
