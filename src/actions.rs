use crate::audio::{AudioCommand, AudioPlayer};
use crate::bilibili;
use crate::formatting::format_bytes;
use crate::local_music;
use crate::lyrics::LyricLine;
use crate::mode::MusicMode;
use crate::shitease;
use crate::storage;
use crate::track::{SOURCE_BILIBILI, SOURCE_LOCAL, SOURCE_YOUTUBE, Track};
use crate::youtube;
use dioxus::prelude::*;
use std::collections::HashSet;
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
        let path = match load_track_path(&track, status).await {
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
            duration: track.duration.map(|duration| duration as f64),
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
        status.set("Searching music...".to_string());
        match shitease::search_shitease_songs(text, Some(18), None).await {
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

pub fn spawn_import_video_url(
    source: VideoImportSource,
    url: String,
    mut queue: Signal<Vec<Track>>,
    mut status: Signal<String>,
) {
    if !source.is_supported_url(&url) {
        status.set(format!("Paste a supported {} URL first.", source.label()));
        return;
    }
    spawn(async move {
        status.set(format!("Importing {} audio...", source.label()));
        let imported = match source {
            VideoImportSource::Bilibili => bilibili::preview_from_url(url)
                .await
                .map(VideoImportPreview::Bilibili),
            VideoImportSource::Youtube => youtube::preview_from_url(url)
                .await
                .map(VideoImportPreview::Youtube),
        };
        match imported {
            Ok(preview) => {
                let detail = video_preview_detail(&preview);
                let track = preview.track();
                let name = track.name.clone();
                let (added, total) = {
                    let mut next = queue.write();
                    let added = append_unique_tracks(&mut next, [track]);
                    (added, next.len())
                };
                if added == 0 {
                    status.set(format!("Already queued. {detail}"));
                } else {
                    status.set(format!("Imported {name}. {detail}. Queue has {total}."));
                }
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
    mut music_mode: Signal<MusicMode>,
    player: Arc<AudioPlayer>,
    mut status: Signal<String>,
    mut lyrics: Signal<Vec<LyricLine>>,
) {
    spawn(async move {
        status.set(format!("Loading random {count}..."));
        match shitease::random_shitease_queue(Some(count), None).await {
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
                        music_mode.set(MusicMode::Silent);
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
        let mut added_total = 0;
        while let Some(event) = receiver.recv().await {
            match event {
                Ok((tracks, total)) => {
                    loaded = total;
                    let added = {
                        let mut next = queue.write();
                        append_unique_tracks(&mut next, tracks)
                    };
                    added_total += added;
                    let total_queue = queue.read().len();
                    status.set(format!(
                        "Scanned {loaded} local tracks. Added {added} new, queue has {total_queue}."
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
            status.set(format!(
                "Scanned {loaded} local tracks. Added {added_total} new, queue has {total}."
            ));
        }
    });
}

pub fn append_unique_tracks(
    queue: &mut Vec<Track>,
    tracks: impl IntoIterator<Item = Track>,
) -> usize {
    let mut seen = queue
        .iter()
        .map(|track| (track.source.clone(), track.id.clone()))
        .collect::<HashSet<_>>();
    let mut added = 0;
    for track in tracks {
        let key = (track.source.clone(), track.id.clone());
        if seen.insert(key) {
            queue.push(track);
            added += 1;
        }
    }
    added
}

async fn load_track_path(track: &Track, mut status: Signal<String>) -> Result<String, String> {
    if track.source == SOURCE_LOCAL {
        if !std::path::Path::new(&track.id).is_file() {
            return Err("Local audio file is not available.".to_string());
        }
        return Ok(track.id.clone());
    }
    if track.source == SOURCE_BILIBILI {
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
        let title = track.name.clone();
        bilibili::download_audio_to_path_with_progress(&track.id, &path, |progress| {
            status.set(video_download_status(
                &title,
                progress.downloaded,
                progress.total,
            ));
        })
        .await?;
        return Ok(path.to_string_lossy().to_string());
    }
    if track.source == SOURCE_YOUTUBE {
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
        let title = track.name.clone();
        youtube::download_audio_to_path_with_progress(&track.id, &path, |progress| {
            status.set(video_download_status(
                &title,
                progress.downloaded,
                progress.total,
            ));
        })
        .await?;
        return Ok(path.to_string_lossy().to_string());
    }
    let info =
        shitease::get_shitease_song_url(track.id.clone(), Some("exhigh".to_string()), None).await?;
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

#[derive(Clone, Copy)]
pub enum VideoImportSource {
    Bilibili,
    Youtube,
}

impl VideoImportSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bilibili => "Bilibili",
            Self::Youtube => "YouTube",
        }
    }

    fn is_supported_url(self, url: &str) -> bool {
        match self {
            Self::Bilibili => bilibili::is_supported_url(url),
            Self::Youtube => youtube::is_supported_url(url),
        }
    }
}

enum VideoImportPreview {
    Bilibili(bilibili::ImportPreview),
    Youtube(youtube::ImportPreview),
}

impl VideoImportPreview {
    fn track(&self) -> Track {
        match self {
            Self::Bilibili(preview) => preview.track.clone(),
            Self::Youtube(preview) => preview.track.clone(),
        }
    }
}

fn video_preview_detail(preview: &VideoImportPreview) -> String {
    let track = preview.track();
    let duration = track
        .duration
        .map(format_duration)
        .unwrap_or_else(|| "duration unknown".to_string());
    match preview {
        VideoImportPreview::Bilibili(preview) => {
            let size = preview_size(preview.size_bytes, preview.estimated_size);
            let bitrate = preview_bitrate(preview.bandwidth);
            format!(
                "{duration}, {size} audio, {bitrate}, {}, {} routes",
                preview.codec, preview.route_count
            )
        }
        VideoImportPreview::Youtube(preview) => {
            let size = preview_size(preview.size_bytes, preview.estimated_size);
            let bitrate = preview_bitrate(preview.bandwidth);
            format!(
                "{duration}, {size}, {bitrate}, {}, {}",
                preview.stream_kind, preview.codec
            )
        }
    }
}

fn preview_size(size_bytes: Option<u64>, estimated_size: bool) -> String {
    size_bytes
        .map(|bytes| {
            if estimated_size {
                format!("~{}", format_bytes(bytes))
            } else {
                format_bytes(bytes)
            }
        })
        .unwrap_or_else(|| "size unknown".to_string())
}

fn preview_bitrate(bandwidth: u64) -> String {
    if bandwidth > 0 {
        format!("{} kbps", bandwidth / 1000)
    } else {
        "bitrate unknown".to_string()
    }
}

fn video_download_status(title: &str, downloaded_bytes: u64, total_bytes: Option<u64>) -> String {
    let downloaded = format_bytes(downloaded_bytes);
    if let Some(total) = total_bytes.filter(|total| *total > 0) {
        let percent = (downloaded_bytes as f64 / total as f64 * 100.0).clamp(0.0, 100.0);
        format!(
            "Downloading {title}: {downloaded} / {} ({percent:.0}%).",
            format_bytes(total)
        )
    } else {
        format!("Downloading {title}: {downloaded}.")
    }
}

fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

async fn load_track_lyrics(track: &Track) -> Vec<LyricLine> {
    if track.source == SOURCE_LOCAL {
        let id = track.id.clone();
        let text = tokio::task::spawn_blocking(move || local_music::read_lyrics(&id))
            .await
            .unwrap_or_default();
        return crate::lyrics::parse_lrc(&text);
    }
    if track.source == SOURCE_BILIBILI {
        return Vec::new();
    }
    if track.source == SOURCE_YOUTUBE {
        return Vec::new();
    }
    shitease::get_shitease_lyric(track.id.clone(), None)
        .await
        .map(|response| crate::lyrics::parse_lrc(response.lyric.as_deref().unwrap_or_default()))
        .unwrap_or_default()
}
