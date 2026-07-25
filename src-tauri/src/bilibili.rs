use crate::track::{SOURCE_BILIBILI, Track};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::AsyncWriteExt;

const ORIGIN: &str = "https://www.bilibili.com";
const API_ORIGIN: &str = "https://api.bilibili.com";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";
static DOWNLOAD_ID: AtomicU64 = AtomicU64::new(1);

pub fn is_supported_url(text: &str) -> bool {
    extract_video_ref(text).is_some()
}

#[derive(Clone, Debug)]
pub struct ImportPreview {
    pub track: Track,
    pub size_bytes: Option<u64>,
    pub estimated_size: bool,
    pub bandwidth: u64,
    pub codec: String,
    pub route_count: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

pub async fn preview_from_url(url: String) -> Result<ImportPreview, String> {
    let video_ref = extract_video_ref(&url)
        .ok_or_else(|| "Paste a supported Bilibili video URL first.".to_string())?;
    let resolved = resolve_video_ref(&video_ref).await?;
    let stream = best_audio_stream(&resolved).await?;
    let (size_bytes, estimated_size) = stream
        .size
        .map(|size| (Some(size), false))
        .unwrap_or_else(|| (estimated_stream_size(&resolved, &stream), true));
    let bandwidth = stream.bandwidth;
    let codec = stream.codec.clone();
    let route_count = stream.urls.len();

    Ok(ImportPreview {
        track: Track {
            source: SOURCE_BILIBILI.to_string(),
            id: track_id(&resolved.bvid, resolved.page),
            name: resolved.title,
            artist: resolved.uploader,
            album: "Bilibili video".to_string(),
            cover: normalize_image_url(&resolved.cover),
            duration: (resolved.duration > 0).then_some(resolved.duration),
        },
        size_bytes,
        estimated_size,
        bandwidth,
        codec,
        route_count,
    })
}

pub async fn download_audio_to_path_with_progress<F>(
    track_id: &str,
    path: &Path,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    let video_ref = parse_track_id(track_id)?;
    let resolved = resolve_video_ref(&video_ref).await?;
    let stream = best_audio_stream(&resolved).await?;
    let expected_size = stream
        .size
        .or_else(|| estimated_stream_size(&resolved, &stream));
    let client = client()?;

    let mut last_error = None;
    for url in &stream.urls {
        let temp_path = unique_temp_path(path);
        match download_audio_url(
            &client,
            url,
            &resolved.webpage_url,
            expected_size,
            &temp_path,
            &mut on_progress,
        )
        .await
        {
            Ok(()) => {
                if path.exists() {
                    tokio::fs::remove_file(path)
                        .await
                        .map_err(|err| format!("Audio cache replace failed: {err}"))?;
                }
                tokio::fs::rename(&temp_path, path)
                    .await
                    .map_err(|err| format!("Audio cache finalize failed: {err}"))?;
                return Ok(());
            }
            Err(err) => {
                cleanup_download(&temp_path).await;
                last_error = Some(err);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "No Bilibili audio URL found.".to_string()))
}

fn unique_temp_path(path: &Path) -> PathBuf {
    let id = DOWNLOAD_ID.fetch_add(1, Ordering::Relaxed);
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "audio".into());
    path.with_file_name(format!("{file_name}.{millis:x}.{id:x}.download"))
}

async fn download_audio_url<F>(
    client: &reqwest::Client,
    url: &str,
    referer: &str,
    expected_size: Option<u64>,
    temp_path: &Path,
    on_progress: &mut F,
) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    let _ = tokio::fs::remove_file(temp_path).await;
    let mut response = client
        .get(url)
        .header(reqwest::header::REFERER, referer)
        .send()
        .await
        .map_err(|err| format!("Bilibili audio request failed: {err}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Bilibili audio request failed: HTTP {}",
            response.status()
        ));
    }

    let total = response.content_length().or(expected_size);
    let write_result = async {
        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|err| format!("Audio cache write failed: {err}"))?;
        let mut written = 0_u64;
        on_progress(DownloadProgress {
            downloaded: written,
            total,
        });
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|err| format!("Bilibili audio read failed: {err}"))?
        {
            written += chunk.len() as u64;
            file.write_all(&chunk)
                .await
                .map_err(|err| format!("Audio cache write failed: {err}"))?;
            on_progress(DownloadProgress {
                downloaded: written,
                total,
            });
        }
        file.flush()
            .await
            .map_err(|err| format!("Audio cache write failed: {err}"))?;
        Ok::<u64, String>(written)
    }
    .await;

    let written = match write_result {
        Ok(written) => written,
        Err(err) => {
            cleanup_download(&temp_path).await;
            return Err(err);
        }
    };

    if written == 0 {
        cleanup_download(&temp_path).await;
        return Err("Bilibili returned an empty audio stream.".to_string());
    }

    Ok(())
}

#[derive(Clone, Debug)]
struct VideoRef {
    bvid: String,
    page: u32,
}

#[derive(Clone, Debug)]
struct ResolvedVideo {
    bvid: String,
    cid: u64,
    page: u32,
    title: String,
    uploader: String,
    cover: String,
    duration: u64,
    webpage_url: String,
}

#[derive(Clone, Debug)]
struct AudioStream {
    urls: Vec<String>,
    bandwidth: u64,
    size: Option<u64>,
    codec: String,
}

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    code: i64,
    #[serde(default)]
    message: String,
    data: Option<T>,
}

#[derive(Debug, Deserialize)]
struct ViewData {
    bvid: String,
    #[serde(default)]
    cid: Option<u64>,
    #[serde(default)]
    title: String,
    #[serde(default)]
    pic: String,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    owner: Option<ViewOwner>,
    #[serde(default)]
    pages: Vec<ViewPage>,
}

#[derive(Debug, Deserialize)]
struct ViewOwner {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct ViewPage {
    #[serde(default)]
    cid: u64,
    #[serde(default)]
    page: u32,
    #[serde(default)]
    part: String,
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(UA)
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::REFERER,
                reqwest::header::HeaderValue::from_static(ORIGIN),
            );
            headers.insert(
                reqwest::header::ORIGIN,
                reqwest::header::HeaderValue::from_static(ORIGIN),
            );
            headers
        })
        .build()
        .map_err(|err| err.to_string())
}

async fn resolve_video_ref(video_ref: &VideoRef) -> Result<ResolvedVideo, String> {
    let url = format!(
        "{API_ORIGIN}/x/web-interface/view?bvid={}",
        urlencoding::encode(&video_ref.bvid)
    );
    let response: ApiResponse<ViewData> = client()?
        .get(url)
        .send()
        .await
        .map_err(|err| format!("Bilibili metadata unavailable: {err}"))?
        .json()
        .await
        .map_err(|err| format!("Invalid Bilibili metadata: {err}"))?;

    if response.code != 0 {
        return Err(api_error("Bilibili metadata unavailable", &response));
    }

    let data = response
        .data
        .ok_or_else(|| "Bilibili metadata is missing.".to_string())?;
    let page = video_ref.page.max(1);
    let selected_page = data
        .pages
        .iter()
        .find(|item| item.page == page)
        .or_else(|| data.pages.first());
    let cid = selected_page
        .map(|item| item.cid)
        .filter(|cid| *cid > 0)
        .or(data.cid)
        .ok_or_else(|| "Bilibili video cid is missing.".to_string())?;
    let page_title = selected_page
        .filter(|item| data.pages.len() > 1 && !item.part.trim().is_empty())
        .map(|item| format!("{} p{:02} {}", data.title, item.page, item.part))
        .unwrap_or_else(|| data.title.clone());

    Ok(ResolvedVideo {
        bvid: data.bvid,
        cid,
        page,
        title: clean_or(&page_title, "Bilibili audio"),
        uploader: data
            .owner
            .map(|owner| clean_or(&owner.name, "Bilibili"))
            .unwrap_or_else(|| "Bilibili".to_string()),
        cover: data.pic,
        duration: data.duration,
        webpage_url: format!("{ORIGIN}/video/{}?p={page}", video_ref.bvid),
    })
}

async fn best_audio_stream(video: &ResolvedVideo) -> Result<AudioStream, String> {
    let url = format!(
        "{API_ORIGIN}/x/player/playurl?bvid={}&cid={}&fnval=4048&try_look=1",
        urlencoding::encode(&video.bvid),
        video.cid
    );
    let response: Value = client()?
        .get(url)
        .header(reqwest::header::REFERER, video.webpage_url.as_str())
        .send()
        .await
        .map_err(|err| format!("Bilibili play info unavailable: {err}"))?
        .json()
        .await
        .map_err(|err| format!("Invalid Bilibili play info: {err}"))?;

    let code = response.get("code").and_then(Value::as_i64).unwrap_or(-1);
    if code != 0 {
        let message = response
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("request failed");
        return Err(format!(
            "Bilibili play info unavailable: {message} ({code})"
        ));
    }

    response
        .get("data")
        .and_then(|data| data.get("dash"))
        .and_then(|dash| dash.get("audio"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|audio| {
            let urls = audio_urls(audio);
            if urls.is_empty() {
                return None;
            }
            Some(AudioStream {
                urls,
                bandwidth: audio.get("bandwidth").and_then(Value::as_u64).unwrap_or(0),
                size: audio.get("size").and_then(Value::as_u64),
                codec: first_string(audio, &["codecs", "codec"])
                    .unwrap_or_else(|| "audio".to_string()),
            })
        })
        .max_by_key(|stream| stream.bandwidth)
        .ok_or_else(|| "No audio-only stream found for this Bilibili video.".to_string())
}

fn estimated_stream_size(video: &ResolvedVideo, stream: &AudioStream) -> Option<u64> {
    if video.duration == 0 || stream.bandwidth == 0 {
        return None;
    }
    Some(video.duration.saturating_mul(stream.bandwidth) / 8)
}

fn audio_urls(audio: &Value) -> Vec<String> {
    let mut urls = Vec::new();
    if let Some(url) = first_string(audio, &["baseUrl", "base_url", "url"]) {
        urls.push(url);
    }
    if let Some(items) = audio
        .get("backupUrl")
        .or_else(|| audio.get("backup_url"))
        .and_then(Value::as_array)
    {
        for url in items.iter().filter_map(Value::as_str) {
            if !url.is_empty() && !urls.iter().any(|item| item == url) {
                urls.push(url.to_string());
            }
        }
    }
    urls
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn extract_video_ref(text: &str) -> Option<VideoRef> {
    let raw = text.trim();
    if raw.is_empty() {
        return None;
    }
    let bvid = extract_bvid(raw)?;
    Some(VideoRef {
        bvid,
        page: query_param(raw, "p")
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|page| *page > 0)
            .unwrap_or(1),
    })
}

fn extract_bvid(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find("bv")?;
    let candidate = text[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    if candidate.len() < 12 {
        return None;
    }
    Some(format!("BV{}", &candidate[2..12]))
}

fn query_param<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let query = text.split_once('?')?.1;
    let query = query.split_once('#').map_or(query, |(query, _)| query);
    for part in query.split('&') {
        if let Some((item_key, value)) = part.split_once('=') {
            if item_key.eq_ignore_ascii_case(key) {
                return Some(value);
            }
        }
    }
    None
}

fn track_id(bvid: &str, page: u32) -> String {
    if page <= 1 {
        bvid.to_string()
    } else {
        format!("{bvid}:p{page}")
    }
}

fn parse_track_id(id: &str) -> Result<VideoRef, String> {
    let (bvid, page) = id
        .split_once(":p")
        .map(|(bvid, page)| (bvid, page.parse::<u32>().unwrap_or(1)))
        .unwrap_or((id, 1));
    let video_ref =
        extract_video_ref(bvid).ok_or_else(|| "Invalid Bilibili track id.".to_string())?;
    Ok(VideoRef {
        bvid: video_ref.bvid,
        page: page.max(1),
    })
}

fn normalize_image_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("http://") {
        format!("https://{rest}")
    } else {
        url.to_string()
    }
}

fn clean_or(value: &str, fallback: &str) -> String {
    let text = value.trim();
    if text.is_empty() {
        fallback.to_string()
    } else {
        text.to_string()
    }
}

fn api_error<T>(prefix: &str, response: &ApiResponse<T>) -> String {
    let message = clean_or(&response.message, "request failed");
    format!("{prefix}: {} ({})", message, response.code)
}

async fn cleanup_download(path: &Path) {
    let _ = tokio::fs::remove_file(path).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bilibili_url() {
        let video = extract_video_ref("https://www.bilibili.com/video/BV1xUUiBKE1n/?p=2")
            .expect("video ref");
        assert_eq!(video.bvid, "BV1xUUiBKE1n");
        assert_eq!(video.page, 2);
    }

    #[test]
    fn parses_track_id() {
        let video = parse_track_id("BV1xUUiBKE1n:p3").expect("track id");
        assert_eq!(video.bvid, "BV1xUUiBKE1n");
        assert_eq!(video.page, 3);
    }

    #[tokio::test]
    #[ignore]
    async fn live_resolves_audio_stream() {
        let video =
            extract_video_ref("https://www.bilibili.com/video/BV1xUUiBKE1n/").expect("video ref");
        let resolved = resolve_video_ref(&video).await.expect("metadata");
        let stream = best_audio_stream(&resolved).await.expect("audio stream");
        let client = client().expect("client");
        let mut last_error = None;
        for url in stream.urls {
            let result = client
                .get(url)
                .header(reqwest::header::REFERER, resolved.webpage_url.as_str())
                .header(reqwest::header::RANGE, "bytes=0-63")
                .send()
                .await;
            let Ok(mut response) = result else {
                last_error = Some("request failed".to_string());
                continue;
            };
            if !response.status().is_success() {
                last_error = Some(format!("HTTP {}", response.status()));
                continue;
            }
            let chunk = response
                .chunk()
                .await
                .expect("audio chunk")
                .expect("audio bytes");
            assert!(!chunk.is_empty());
            return;
        }
        panic!(
            "no reachable audio URL: {}",
            last_error.unwrap_or_else(|| "none".to_string())
        );
    }
}
