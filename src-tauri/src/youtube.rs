use crate::track::{SOURCE_YOUTUBE, Track};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::AsyncWriteExt;

const ORIGIN: &str = "https://www.youtube.com";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";
static DOWNLOAD_ID: AtomicU64 = AtomicU64::new(1);

pub fn is_supported_url(text: &str) -> bool {
    extract_video_id(text).is_some()
}

#[derive(Clone, Debug)]
pub struct ImportPreview {
    pub track: Track,
    pub size_bytes: Option<u64>,
    pub estimated_size: bool,
    pub bandwidth: u64,
    pub codec: String,
    pub stream_kind: String,
}

#[derive(Clone, Copy, Debug)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

pub async fn preview_from_url(url: String) -> Result<ImportPreview, String> {
    let video_id =
        extract_video_id(&url).ok_or_else(|| "Paste a supported YouTube URL first.".to_string())?;
    let resolved = resolve_video(&video_id).await?;
    let stream = best_stream(&resolved)?;
    let (size_bytes, estimated_size) = stream
        .size
        .map(|size| (Some(size), false))
        .unwrap_or_else(|| (estimated_stream_size(&resolved, &stream), true));

    Ok(ImportPreview {
        track: Track {
            source: SOURCE_YOUTUBE.to_string(),
            id: resolved.video_id,
            name: resolved.title,
            artist: resolved.uploader,
            album: "YouTube video".to_string(),
            cover: resolved.cover,
            duration: (resolved.duration > 0).then_some(resolved.duration),
        },
        size_bytes,
        estimated_size,
        bandwidth: stream.bandwidth,
        codec: stream.codec,
        stream_kind: stream.kind.label().to_string(),
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
    let video_id =
        extract_video_id(track_id).ok_or_else(|| "Invalid YouTube track id.".to_string())?;
    let resolved = resolve_video(&video_id).await?;
    let streams = playable_streams(&resolved);
    if streams.is_empty() {
        return Err(no_stream_error());
    }
    let client = client()?;

    let mut last_error = None;
    for stream in streams {
        let expected_size = stream
            .size
            .or_else(|| estimated_stream_size(&resolved, &stream));
        for _ in 0..2 {
            let temp_path = unique_temp_path(path);
            match download_stream_url(
                &client,
                &stream.url,
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
    }

    Err(last_error.unwrap_or_else(no_stream_error))
}

#[derive(Clone, Debug)]
struct ResolvedVideo {
    video_id: String,
    title: String,
    uploader: String,
    cover: String,
    duration: u64,
    webpage_url: String,
    player_responses: Vec<Value>,
}

#[derive(Clone, Debug)]
struct MediaStream {
    url: String,
    bandwidth: u64,
    size: Option<u64>,
    codec: String,
    duration: Option<u64>,
    kind: StreamKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StreamKind {
    AudioOnly,
    CompatibleMp4,
}

impl StreamKind {
    fn label(self) -> &'static str {
        match self {
            Self::AudioOnly => "audio-only",
            Self::CompatibleMp4 => "compatible mp4",
        }
    }
}

#[derive(Clone, Copy)]
struct InnertubeClient {
    name: &'static str,
    version: &'static str,
    header_name: &'static str,
    user_agent: &'static str,
    extra: fn() -> Value,
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

async fn resolve_video(video_id: &str) -> Result<ResolvedVideo, String> {
    let webpage_url = format!("{ORIGIN}/watch?v={video_id}&bpctr=9999999999&has_verified=1");
    let client = client()?;
    let webpage = client
        .get(&webpage_url)
        .send()
        .await
        .map_err(|err| format!("YouTube page unavailable: {err}"))?
        .text()
        .await
        .map_err(|err| format!("YouTube page read failed: {err}"))?;

    let mut player_responses = Vec::new();
    if let Some(response) = extract_json_after_marker(&webpage, "ytInitialPlayerResponse") {
        if let Ok(value) = serde_json::from_str::<Value>(&response) {
            player_responses.push(value);
        }
    }

    if let Some(api_key) = extract_innertube_api_key(&webpage) {
        for spec in innertube_clients() {
            if let Ok(response) = call_innertube_player(&client, &api_key, video_id, spec).await {
                player_responses.push(response);
            }
            if player_responses
                .iter()
                .any(|response| !collect_streams(response).is_empty())
            {
                break;
            }
        }
    }

    let details = player_responses
        .iter()
        .find_map(|response| response.get("videoDetails"))
        .ok_or_else(|| "YouTube metadata is missing.".to_string())?;
    let title = clean_or(
        details.get("title").and_then(Value::as_str),
        "YouTube audio",
    );
    let uploader = clean_or(details.get("author").and_then(Value::as_str), "YouTube");
    let duration = details
        .get("lengthSeconds")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let cover = details
        .get("thumbnail")
        .and_then(|thumbnail| thumbnail.get("thumbnails"))
        .and_then(Value::as_array)
        .and_then(|items| items.iter().rev().find_map(|item| item.get("url")))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    Ok(ResolvedVideo {
        video_id: video_id.to_string(),
        title,
        uploader,
        cover,
        duration,
        webpage_url: format!("{ORIGIN}/watch?v={video_id}"),
        player_responses,
    })
}

async fn call_innertube_player(
    client: &reqwest::Client,
    api_key: &str,
    video_id: &str,
    spec: InnertubeClient,
) -> Result<Value, String> {
    let mut client_context = json!({
        "clientName": spec.name,
        "clientVersion": spec.version,
        "hl": "en",
        "timeZone": "UTC",
        "utcOffsetMinutes": 0,
    });
    merge_object(&mut client_context, (spec.extra)());
    let body = json!({
        "context": {
            "client": client_context,
        },
        "videoId": video_id,
        "contentCheckOk": true,
        "racyCheckOk": true,
        "playbackContext": {
            "contentPlaybackContext": {
                "html5Preference": "HTML5_PREF_WANTS",
            },
        },
    });
    let url = format!(
        "{ORIGIN}/youtubei/v1/player?key={}&prettyPrint=false",
        urlencoding::encode(api_key)
    );
    client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("x-youtube-client-name", spec.header_name)
        .header("x-youtube-client-version", spec.version)
        .header(reqwest::header::USER_AGENT, spec.user_agent)
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("YouTube player API unavailable: {err}"))?
        .json()
        .await
        .map_err(|err| format!("Invalid YouTube player API response: {err}"))
}

fn innertube_clients() -> [InnertubeClient; 5] {
    [
        InnertubeClient {
            name: "ANDROID",
            version: "21.26.364",
            header_name: "3",
            user_agent: "com.google.android.youtube/21.26.364 (Linux; U; Android 11) gzip",
            extra: || {
                json!({
                    "androidSdkVersion": 30,
                    "userAgent": "com.google.android.youtube/21.26.364 (Linux; U; Android 11) gzip",
                    "osName": "Android",
                    "osVersion": "11",
                })
            },
        },
        InnertubeClient {
            name: "ANDROID_VR",
            version: "1.65.10",
            header_name: "28",
            user_agent: "com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
            extra: || {
                json!({
                    "deviceMake": "Oculus",
                    "deviceModel": "Quest 3",
                    "androidSdkVersion": 32,
                    "userAgent": "com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
                    "osName": "Android",
                    "osVersion": "12L",
                })
            },
        },
        InnertubeClient {
            name: "VISIONOS",
            version: "1.02",
            header_name: "101",
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15",
            extra: || {
                json!({
                    "deviceMake": "Apple",
                    "deviceModel": "RealityDevice17,1",
                    "userAgent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15",
                    "osName": "visionOS",
                    "osVersion": "26.5.23O471",
                })
            },
        },
        InnertubeClient {
            name: "IOS",
            version: "21.26.4",
            header_name: "5",
            user_agent: "com.google.ios.youtube/21.26.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X;)",
            extra: || {
                json!({
                    "deviceMake": "Apple",
                    "deviceModel": "iPhone16,2",
                    "userAgent": "com.google.ios.youtube/21.26.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X;)",
                    "osName": "iPhone",
                    "osVersion": "18.3.2.22D82",
                })
            },
        },
        InnertubeClient {
            name: "WEB",
            version: "2.20260708.00.00",
            header_name: "1",
            user_agent: UA,
            extra: || json!({}),
        },
    ]
}

fn merge_object(target: &mut Value, extra: Value) {
    let Some(target) = target.as_object_mut() else {
        return;
    };
    let Some(extra) = extra.as_object() else {
        return;
    };
    for (key, value) in extra {
        target.insert(key.clone(), value.clone());
    }
}

fn best_stream(video: &ResolvedVideo) -> Result<MediaStream, String> {
    playable_streams(video)
        .into_iter()
        .next()
        .ok_or_else(no_stream_error)
}

fn playable_streams(video: &ResolvedVideo) -> Vec<MediaStream> {
    let mut streams = video
        .player_responses
        .iter()
        .flat_map(collect_streams)
        .collect::<Vec<_>>();
    streams.sort_by_key(|stream| {
        let kind_score = match stream.kind {
            StreamKind::AudioOnly => 2_u64,
            StreamKind::CompatibleMp4 => 1,
        };
        std::cmp::Reverse(kind_score * 10_000_000 + stream.bandwidth)
    });
    streams
}

fn no_stream_error() -> String {
    "No direct YouTube audio stream found. This video may require YouTube JS signature, n-challenge, or PO-token support.".to_string()
}

fn collect_streams(response: &Value) -> Vec<MediaStream> {
    let mut streams = Vec::new();
    let Some(streaming_data) = response.get("streamingData") else {
        return streams;
    };
    for key in ["adaptiveFormats", "formats"] {
        let Some(items) = streaming_data.get(key).and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            if item
                .get("drmFamilies")
                .and_then(Value::as_array)
                .is_some_and(|families| !families.is_empty())
            {
                continue;
            }
            if item.get("targetDurationSec").is_some()
                || item
                    .get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| kind == "FORMAT_STREAM_TYPE_OTF")
            {
                continue;
            }
            let Some((url, ciphered)) = stream_url(item) else {
                continue;
            };
            if ciphered || has_query_param(&url, "n") {
                continue;
            }
            let mime = item.get("mimeType").and_then(Value::as_str).unwrap_or("");
            let kind = stream_kind(item, mime);
            let Some(kind) = kind else {
                continue;
            };
            streams.push(MediaStream {
                url,
                bandwidth: number_or_string(item, &["averageBitrate", "bitrate"]).unwrap_or(0),
                size: number_or_string(item, &["contentLength"]),
                codec: codec_label(mime),
                duration: number_or_string(item, &["approxDurationMs"])
                    .map(|value| (value + 999) / 1000),
                kind,
            });
        }
    }
    streams
}

fn stream_url(item: &Value) -> Option<(String, bool)> {
    if let Some(url) = item.get("url").and_then(Value::as_str) {
        return Some((url.to_string(), false));
    }
    let cipher = item
        .get("signatureCipher")
        .or_else(|| item.get("cipher"))
        .and_then(Value::as_str)?;
    let parts = parse_query_like(cipher);
    let url = parts.iter().find(|(key, _)| key == "url")?.1.clone();
    let encrypted = parts
        .iter()
        .any(|(key, value)| key == "s" && !value.is_empty());
    Some((url, encrypted))
}

fn stream_kind(item: &Value, mime: &str) -> Option<StreamKind> {
    if mime.starts_with("audio/mp4") {
        return Some(StreamKind::AudioOnly);
    }
    if !mime.starts_with("video/mp4") {
        return None;
    }
    if item
        .get("audioChannels")
        .and_then(Value::as_u64)
        .is_some_and(|channels| channels > 0)
    {
        return Some(StreamKind::CompatibleMp4);
    }
    let lower = mime.to_ascii_lowercase();
    (lower.contains("mp4a") || lower.contains("opus")).then_some(StreamKind::CompatibleMp4)
}

fn estimated_stream_size(video: &ResolvedVideo, stream: &MediaStream) -> Option<u64> {
    let duration = stream.duration.unwrap_or(video.duration);
    if duration == 0 || stream.bandwidth == 0 {
        return None;
    }
    Some(duration.saturating_mul(stream.bandwidth) / 8)
}

async fn download_stream_url<F>(
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
        .map_err(|err| format!("YouTube stream request failed: {err}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "YouTube stream request failed: HTTP {}",
            response.status()
        ));
    }

    let total = response.content_length().or(expected_size);
    let mut file = tokio::fs::File::create(temp_path)
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
        .map_err(|err| format!("YouTube stream read failed: {err}"))?
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

    if written == 0 {
        return Err("YouTube returned an empty stream.".to_string());
    }
    Ok(())
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

fn extract_video_id(text: &str) -> Option<String> {
    let raw = text.trim();
    if raw.len() == 11 && raw.chars().all(is_video_id_char) {
        return Some(raw.to_string());
    }
    for key in ["v", "video_id"] {
        if let Some(value) = query_param(raw, key).filter(|value| value.len() >= 11) {
            let id = value.chars().take(11).collect::<String>();
            if id.chars().all(is_video_id_char) {
                return Some(id);
            }
        }
    }
    for marker in ["/shorts/", "/embed/", "/live/", "/v/", "youtu.be/"] {
        if let Some((_, rest)) = raw.split_once(marker) {
            let id = rest.chars().take(11).collect::<String>();
            if id.len() == 11 && id.chars().all(is_video_id_char) {
                return Some(id);
            }
        }
    }
    None
}

fn is_video_id_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
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

fn extract_innertube_api_key(webpage: &str) -> Option<String> {
    extract_json_after_marker(webpage, "ytcfg.set")
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| {
            value
                .get("INNERTUBE_API_KEY")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| extract_quoted_value(webpage, "INNERTUBE_API_KEY"))
}

fn extract_quoted_value(text: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\":\"");
    let start = text.find(&marker)? + marker.len();
    let rest = &text[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_json_after_marker(text: &str, marker: &str) -> Option<String> {
    let start = text.find(marker)?;
    let open = text[start..].find('{')? + start;
    extract_balanced_json(text, open)
}

fn extract_balanced_json(text: &str, open: usize) -> Option<String> {
    let bytes = text.as_bytes();
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut escape = false;
    for index in open..bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(text[open..=index].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_query_like(text: &str) -> Vec<(String, String)> {
    text.split('&')
        .filter_map(|part| {
            let (key, value) = part.split_once('=')?;
            let key = urlencoding::decode(key).ok()?.into_owned();
            let value = urlencoding::decode(value).ok()?.into_owned();
            Some((key, value))
        })
        .collect()
}

fn has_query_param(url: &str, key: &str) -> bool {
    let Some(query) = url.split_once('?').map(|(_, query)| query) else {
        return false;
    };
    parse_query_like(query)
        .iter()
        .any(|(item_key, value)| item_key == key && !value.is_empty())
}

fn number_or_string(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        let item = value.get(*key)?;
        item.as_u64()
            .or_else(|| item.as_str().and_then(|text| text.parse::<u64>().ok()))
    })
}

fn codec_label(mime: &str) -> String {
    let Some((_, rest)) = mime.split_once("codecs=\"") else {
        return mime
            .split_once(';')
            .map_or(mime, |(mime, _)| mime)
            .to_string();
    };
    rest.split_once('"')
        .map(|(codec, _)| codec.to_string())
        .unwrap_or_else(|| "audio".to_string())
}

fn clean_or(value: Option<&str>, fallback: &str) -> String {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

async fn cleanup_download(path: &Path) {
    let _ = tokio::fs::remove_file(path).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_youtube_urls() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=iKbylgzysHw").as_deref(),
            Some("iKbylgzysHw")
        );
        assert_eq!(
            extract_video_id("https://youtu.be/iKbylgzysHw?t=12").as_deref(),
            Some("iKbylgzysHw")
        );
        assert_eq!(
            extract_video_id("iKbylgzysHw").as_deref(),
            Some("iKbylgzysHw")
        );
    }

    #[test]
    fn extracts_balanced_json_with_strings() {
        let text = r#"x ytInitialPlayerResponse = {"a":"};","b":{"c":1}}; y"#;
        let value = extract_json_after_marker(text, "ytInitialPlayerResponse").expect("json");
        assert_eq!(value, r#"{"a":"};","b":{"c":1}}"#);
    }

    #[tokio::test]
    #[ignore]
    async fn live_resolves_youtube_stream() {
        let video = resolve_video("iKbylgzysHw").await.expect("video");
        let stream = best_stream(&video).expect("stream");
        assert!(stream.bandwidth > 0);

        let chunk = client()
            .expect("client")
            .get(&stream.url)
            .header(reqwest::header::REFERER, video.webpage_url.as_str())
            .header(reqwest::header::RANGE, "bytes=0-63")
            .send()
            .await
            .expect("stream response")
            .chunk()
            .await
            .expect("stream read")
            .expect("stream bytes");
        assert!(!chunk.is_empty());
    }
}
