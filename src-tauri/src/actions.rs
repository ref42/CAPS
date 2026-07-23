use crate::audio::{AudioCommand, AudioPlayer};
use crate::lyrics::{LyricLine, parse_lrc};
use crate::netease;
use crate::track::Track;
use dioxus::prelude::*;
use std::sync::Arc;

pub fn spawn_play(
    track: Track,
    player: Arc<AudioPlayer>,
    mut status: Signal<String>,
    mut lyrics: Signal<Vec<LyricLine>>,
) {
    spawn(async move {
        status.set(format!("Loading {}...", track.name));
        lyrics.set(Vec::new());
        let url =
            match netease::get_netease_song_url(track.id.clone(), Some("exhigh".to_string()), None)
                .await
            {
                Ok(info) => info.url.unwrap_or_default(),
                Err(err) => {
                    status.set(err);
                    return;
                }
            };
        if url.is_empty() {
            status.set("No playable stream for this track.".to_string());
            return;
        }
        match reqwest::get(&url).await {
            Ok(response) => match response.bytes().await {
                Ok(bytes) => {
                    player.send(AudioCommand::LoadBytes {
                        bytes: bytes.to_vec(),
                        title: track.name.clone(),
                        detail: track.artist.clone(),
                    });
                    status.set(format!("Playing {}.", track.name));
                    if let Ok(response) = netease::get_netease_lyric(track.id.clone(), None).await {
                        lyrics.set(parse_lrc(response.lyric.as_deref().unwrap_or_default()));
                    }
                }
                Err(err) => status.set(format!("Stream read failed: {err}")),
            },
            Err(err) => status.set(format!("Stream request failed: {err}")),
        }
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

pub fn spawn_random_queue(count: u32, mut queue: Signal<Vec<Track>>, mut status: Signal<String>) {
    spawn(async move {
        status.set(format!("Loading random {count}..."));
        match netease::random_netease_queue(Some(count), None).await {
            Ok(items) => {
                let tracks = items.into_iter().map(Track::from).collect::<Vec<_>>();
                let loaded = tracks.len();
                queue.set(tracks);
                status.set(format!("Queued {loaded} random tracks."));
            }
            Err(err) => status.set(err),
        }
    });
}
