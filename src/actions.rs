//! UI-independent music jobs. Progress crosses into the native UI through messages.
use crate::formatting::format_bytes;
use crate::lyrics::LyricLine;
use crate::track::*;
use crate::{bilibili, kugou, local_music, qqmusic, shitease, storage, youtube};
use std::{collections::HashSet, sync::Arc};
use tokio::io::AsyncWriteExt;

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
    if text.trim().is_empty() {
        return Err("Type a song name first.".into());
    }
    let (a, b, c) = tokio::join!(
        shitease::search_shitease_songs(text.to_owned(), Some(150), None),
        qqmusic::search(text.to_owned(), 150),
        kugou::search(text, 150)
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
    if tracks.is_empty() {
        Err("Online music search is unavailable.".into())
    } else {
        Ok(tracks)
    }
}

pub async fn random(count: u32) -> Result<Vec<Track>, String> {
    let count = count.clamp(1, 999);
    let (a, b, c) = tokio::join!(
        shitease::random_shitease_queue(Some(count), None),
        qqmusic::search_random(count),
        kugou::search_random(count)
    );
    let providers: Vec<Vec<Track>> = vec![
        a.unwrap_or_default().into_iter().map(Track::from).collect(),
        b.unwrap_or_default().into_iter().map(Track::from).collect(),
        c.unwrap_or_default().into_iter().map(Track::from).collect(),
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

pub async fn import_video(
    source: VideoImportSource,
    url: String,
) -> Result<(Track, String), String> {
    if !source.is_supported_url(&url) {
        return Err(format!("Paste a supported {} URL first.", source.label()));
    }
    let preview = match source {
        VideoImportSource::Bilibili => {
            VideoImportPreview::Bilibili(bilibili::preview_from_url(url).await?)
        }
        VideoImportSource::Youtube => {
            VideoImportPreview::Youtube(youtube::preview_from_url(url).await?)
        }
    };
    Ok((preview.track(), video_preview_detail(&preview)))
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

pub async fn load_track_path(track: &Track, mut status: Progress) -> Result<String, String> {
    if track.source == SOURCE_LOCAL {
        if !std::path::Path::new(&track.id).is_file() {
            return Err("Local audio file is not available.".to_string());
        }
        return Ok(track.id.to_owned());
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
        let title = track.name.to_owned();
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
        let title = track.name.to_owned();
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
    if track.source == SOURCE_QQMUSIC {
        let path = storage::song_cache_file(&track.source, &track.id)
            .ok_or_else(|| "Song cache path is not available.".to_string())?;
        if path.exists() && path.metadata().map(|meta| meta.len()).unwrap_or(0) > 0 {
            return Ok(path.to_string_lossy().to_string());
        }
        let url = qqmusic::stream_url_with_media(&track.id, &track.media_id).await?;
        let parent = path
            .parent()
            .ok_or_else(|| "Song cache path is not valid.".to_string())?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|err| format!("Song cache unavailable: {err}"))?;
        let http = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/124.0 Safari/537.36")
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    reqwest::header::REFERER,
                    reqwest::header::HeaderValue::from_static("https://y.qq.com/"),
                );
                headers
            })
            .build()
            .map_err(|err| format!("QQ Music client unavailable: {err}"))?;
        let response = http
            .get(&url)
            .send()
            .await
            .map_err(|err| format!("QQ Music stream request failed: {err}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "QQ Music stream request failed with HTTP {}.",
                response.status()
            ));
        }
        return download_response_to_cache(response, &path, &track.name, &mut status, "Buffering")
            .await;
    }
    if track.source == SOURCE_KUGOU {
        let path = storage::song_cache_file(&track.source, &track.id)
            .ok_or_else(|| "Song cache path is not available.".to_string())?;
        if path.exists() && path.metadata().map(|meta| meta.len()).unwrap_or(0) > 0 {
            return Ok(path.to_string_lossy().to_string());
        }
        let (quality_hash, album_id, album_audio_id) = kugou_metadata(track);
        let url = kugou::stream_url(&track.id, quality_hash, album_id, album_audio_id).await?;
        let parent = path
            .parent()
            .ok_or_else(|| "Song cache path is not valid.".to_string())?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|err| format!("Song cache unavailable: {err}"))?;
        let response = reqwest::Client::builder()
            .user_agent("CAPS/1.0")
            .build()
            .map_err(|err| format!("KuGou download client unavailable: {err}"))?
            .get(url)
            .header(
                reqwest::header::REFERER,
                reqwest::header::HeaderValue::from_static("https://www.kugou.com/"),
            )
            .send()
            .await
            .map_err(|err| format!("KuGou stream request failed: {err}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "KuGou stream request failed with HTTP {}.",
                response.status()
            ));
        }
        return download_response_to_cache(response, &path, &track.name, &mut status, "Buffering")
            .await;
    }
    let info =
        shitease::get_shitease_song_url(track.id.to_owned(), Some("exhigh".to_string()), None)
            .await?;
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

fn kugou_metadata(track: &Track) -> (&str, &str, &str) {
    let mut parts = track.media_id.split('|');
    (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
    )
}

async fn download_response_to_cache(
    mut response: reqwest::Response,
    path: &std::path::Path,
    title: &str,
    status: &mut Progress,
    label: &str,
) -> Result<String, String> {
    let total = response.content_length();
    let temp_path = path.with_extension("download");
    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .map_err(|err| format!("Song cache write failed: {err}"))?;
    let mut downloaded = 0_u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|err| format!("Stream read failed: {err}"))?
    {
        downloaded += chunk.len() as u64;
        file.write_all(&chunk)
            .await
            .map_err(|err| format!("Song cache write failed: {err}"))?;
        if let Some(total) = total {
            let percent = downloaded.saturating_mul(100) / total.max(1);
            status.set(format!("{label} {title}: {percent}%"));
        } else {
            status.set(format!("{label} {title}: {}", format_bytes(downloaded)));
        }
    }
    file.flush()
        .await
        .map_err(|err| format!("Song cache write failed: {err}"))?;
    drop(file);
    tokio::fs::rename(&temp_path, path)
        .await
        .map_err(|err| format!("Song cache finalize failed: {err}"))?;
    Ok(path.to_string_lossy().to_string())
}

#[allow(dead_code)]
pub async fn prefetch_track(track: Track, path: std::path::PathBuf) -> Result<(), String> {
    let url = if track.source == SOURCE_QQMUSIC {
        qqmusic::stream_url_with_media(&track.id, &track.media_id).await?
    } else if track.source == SOURCE_KUGOU {
        let (quality_hash, album_id, album_audio_id) = kugou_metadata(&track);
        kugou::stream_url(&track.id, quality_hash, album_id, album_audio_id).await?
    } else if track.source == SOURCE_NETEASE {
        shitease::get_shitease_song_url(track.id.to_owned(), Some("exhigh".to_string()), None)
            .await?
            .url
            .unwrap_or_default()
    } else {
        track.stream_url.to_owned()
    };
    if url.is_empty() {
        return Ok(());
    }
    let response = reqwest::Client::builder()
        .user_agent("CAPS/1.0")
        .build()
        .map_err(|err| format!("Audio download client unavailable: {err}"))?
        .get(url)
        .send()
        .await
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?;
    let mut ignored_status = Progress::new(|_| {});
    let _ = download_response_to_cache(
        response,
        &path,
        &track.name,
        &mut ignored_status,
        "Prefetching",
    )
    .await?;
    Ok(())
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
            Self::Bilibili(preview) => preview.track.to_owned(),
            Self::Youtube(preview) => preview.track.to_owned(),
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

pub async fn load_track_lyrics(track: &Track) -> Vec<LyricLine> {
    if track.source == SOURCE_LOCAL {
        let id = track.id.to_owned();
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
    if track.source == SOURCE_QQMUSIC {
        return qqmusic::lyric(&track.id)
            .await
            .map(|text| crate::lyrics::parse_lrc(&text))
            .unwrap_or_default();
    }
    if track.source == SOURCE_KUGOU {
        if let Ok(text) = kugou::lyric(&track.id, &track.name, &track.artist, track.duration).await
        {
            return crate::lyrics::parse_lrc(&text);
        }
        return crate::lyric_finder::find(&track.name, &track.artist, track.duration)
            .await
            .map(|text| crate::lyrics::parse_lrc(&text))
            .unwrap_or_default();
    }
    if track.source == SOURCE_NETEASE || track.source == SOURCE_SHITEASE {
        return shitease::get_shitease_lyric(track.id.to_owned(), None)
            .await
            .map(|response| crate::lyrics::parse_lrc(response.lyric.as_deref().unwrap_or_default()))
            .unwrap_or_default();
    }
    Vec::new()
}
