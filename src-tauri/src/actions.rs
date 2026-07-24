use crate::audio::{AudioCommand, AudioPlayer};
use crate::local_music;
use crate::lyrics::LyricLine;
use crate::netease;
use crate::storage;
use crate::track::{SOURCE_LOCAL, Track};
use dioxus::prelude::*;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

const LOCAL_IMPORT_BATCH_SIZE: usize = 80;

pub fn spawn_play(
    track: Track,
    player: Arc<AudioPlayer>,
    mut current_index: Signal<Option<usize>>,
    mut current_track: Signal<Option<Track>>,
    mut status: Signal<String>,
    mut lyrics: Signal<Vec<LyricLine>>,
) {
    spawn(async move {
        status.set(format!("Loading {}...", track.name));
        lyrics.set(Vec::new());
        let path = match load_track_path(&track).await {
            Ok(path) => path,
            Err(err) => {
                player.send(AudioCommand::Stop);
                current_index.set(None);
                current_track.set(None);
                status.set(err);
                return;
            }
        };
        current_track.set(Some(track.clone()));
        player.send(AudioCommand::LoadFile {
            path,
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

pub enum RandomQueueMode {
    Append,
    Replace,
}

pub fn spawn_random_queue(
    count: u32,
    mode: RandomQueueMode,
    mut queue: Signal<Vec<Track>>,
    mut current_index: Signal<Option<usize>>,
    mut current_track: Signal<Option<Track>>,
    player: Arc<AudioPlayer>,
    mut status: Signal<String>,
    mut lyrics: Signal<Vec<LyricLine>>,
) {
    spawn(async move {
        status.set(format!("Loading random {count}..."));
        match netease::random_netease_queue(Some(count), None).await {
            Ok(items) => {
                let tracks = items.into_iter().map(Track::from).collect::<Vec<_>>();
                let loaded = tracks.len();
                match mode {
                    RandomQueueMode::Append => {
                        let total = {
                            let mut next = queue.write();
                            next.extend(tracks);
                            next.len()
                        };
                        status.set(format!("Added {loaded} random tracks. Queue has {total}."));
                    }
                    RandomQueueMode::Replace => {
                        queue.set(tracks);
                        current_index.set(None);
                        current_track.set(None);
                        lyrics.set(Vec::new());
                        player.send(AudioCommand::Stop);
                        status.set(format!("Replaced queue with {loaded} random tracks."));
                    }
                }
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
        let (sender, mut receiver) =
            tokio::sync::mpsc::unbounded_channel::<Result<(Vec<Track>, usize), String>>();

        tokio::task::spawn_blocking(move || {
            let result =
                local_music::load_all_batched(&folder, LOCAL_IMPORT_BATCH_SIZE, |batch, total| {
                    sender.send(Ok((batch, total))).is_ok()
                });
            if let Err(err) = result {
                let _ = sender.send(Err(err));
            }
        });

        let mut loaded = 0;
        while let Some(event) = receiver.recv().await {
            match event {
                Ok((tracks, total)) => {
                    loaded = total;
                    let added = tracks.len();
                    {
                        let mut next = queue.write();
                        next.extend(tracks);
                    }
                    let total_queue = queue.read().len();
                    status.set(format!(
                        "Loaded {loaded} local tracks. Added {added}, queue has {total_queue}."
                    ));
                }
                Err(err) => {
                    status.set(err);
                    return;
                }
            }
        }

        if loaded == 0 {
            status.set("No supported audio files found.".to_string());
        } else {
            let total = queue.read().len();
            status.set(format!("Queued {loaded} local tracks. Queue has {total}."));
        }
    });
}

async fn load_track_path(track: &Track) -> Result<String, String> {
    if track.source == SOURCE_LOCAL {
        if !std::path::Path::new(&track.id).is_file() {
            return Err("Local audio file is not available.".to_string());
        }
        return Ok(track.id.clone());
    }
    let info =
        netease::get_netease_song_url(track.id.clone(), Some("exhigh".to_string()), None).await?;
    let url = info
        .url
        .filter(|url| !url.is_empty())
        .ok_or_else(|| "No playable stream for this track.".to_string())?;
    let path = storage::song_cache_file(&track.source, &track.id)
        .ok_or_else(|| "Song cache path is not available.".to_string())?;
    if path.exists() && path.metadata().map(|meta| meta.len()).unwrap_or(0) > 0 {
        return Ok(path.to_string_lossy().to_string());
    }
    let Some(parent) = path.parent() else {
        return Err("Song cache path is not valid.".to_string());
    };
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|err| format!("Song cache unavailable: {err}"))?;
    let mut response = reqwest::get(&url)
        .await
        .map_err(|err| format!("Stream request failed: {err}"))?;
    let temp_path = path.with_extension("download");
    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .map_err(|err| format!("Song cache write failed: {err}"))?;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|err| format!("Stream read failed: {err}"))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|err| format!("Song cache write failed: {err}"))?;
    }
    file.flush()
        .await
        .map_err(|err| format!("Song cache write failed: {err}"))?;
    drop(file);
    tokio::fs::rename(&temp_path, &path)
        .await
        .map_err(|err| format!("Song cache finalize failed: {err}"))?;
    Ok(path.to_string_lossy().to_string())
}

async fn load_track_lyrics(track: &Track) -> Vec<LyricLine> {
    if track.source == SOURCE_LOCAL {
        let id = track.id.clone();
        let text = tokio::task::spawn_blocking(move || local_music::read_lyrics(&id))
            .await
            .unwrap_or_default();
        return crate::lyrics::parse_lrc(&text);
    }
    netease::get_netease_lyric(track.id.clone(), None)
        .await
        .map(|response| crate::lyrics::parse_lrc(response.lyric.as_deref().unwrap_or_default()))
        .unwrap_or_default()
}
