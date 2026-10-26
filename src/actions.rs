//! UI-independent music jobs. Progress crosses into the native UI through messages.
use crate::formatting::format_bytes;
use crate::lyrics::LyricLine;
use crate::track::*;
use crate::{bilibili, douyin, kugou, local_music, qishui, qqmusic, shitease, storage, youtube};
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
    if text.trim().is_empty() {
        return Err("Type a song name first.".into());
    }
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

pub async fn import_video(text: String, youtube_cookie: String) -> Result<(Track, String), String> {
    let (source, url) = crate::media_import::detect(&text)?;
    let url = crate::media_import::resolve_share(source, url).await?;
    let preview = match source {
        VideoImportSource::Bilibili => {
            VideoImportPreview::Bilibili(bilibili::preview_from_url(url).await?)
        }
        VideoImportSource::Youtube => {
            VideoImportPreview::Youtube(youtube::preview_from_url_with_cookie(url, &youtube_cookie).await?)
        }
        VideoImportSource::Douyin => VideoImportPreview::Douyin(douyin::preview(&url).await?),
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

pub async fn load_track_audio(
    track: &Track,
    mut status: Progress,
    youtube_cookie: &str,
) -> Result<crate::audio::LoadedAudio, String> {
    use crate::{
        audio::LoadedAudio,
        streaming::{RemoteMedia, media_client},
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
        SOURCE_DOUYIN => {
            let preview =
                douyin::preview(&format!("https://www.douyin.com/video/{}", track.id)).await?;
            douyin::remote(&preview)?
        }
        SOURCE_BILIBILI => bilibili::resolve_audio(&track.id).await?,
        SOURCE_YOUTUBE => youtube::resolve_audio_with_cookie(&track.id, youtube_cookie).await?,
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

use crate::media_import::Provider as VideoImportSource;

enum VideoImportPreview {
    Bilibili(bilibili::ImportPreview),
    Youtube(youtube::ImportPreview),
    Douyin(douyin::Preview),
}

impl VideoImportPreview {
    fn track(&self) -> Track {
        match self {
            Self::Bilibili(preview) => preview.track.to_owned(),
            Self::Youtube(preview) => preview.track.to_owned(),
            Self::Douyin(preview) => Track {
                source: SOURCE_DOUYIN.into(),
                id: preview.id.clone(),
                media_id: String::new(),
                stream_url: String::new(),
                name: preview.title.clone(),
                artist: preview.author.clone(),
                album: "Douyin video".into(),
                cover: preview.cover.clone(),
                duration: preview.duration,
            },
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
        VideoImportPreview::Douyin(preview) => {
            format!(
                "{duration}, {} video download; plays as one audio track",
                preview_size(preview.size, false)
            )
        }
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
    if track.source == SOURCE_QISHUI {
        return qishui::lyric(&track.id)
            .await
            .map(|text| crate::lyrics::parse_lrc(&text))
            .unwrap_or_default();
    }
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
    if track.source == SOURCE_YOUTUBE || track.source == SOURCE_DOUYIN {
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
