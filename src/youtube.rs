use crate::track::{SOURCE_YOUTUBE, Track};
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const ORIGIN: &str = "https://www.youtube.com";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

#[derive(Clone, Debug)]
pub struct ImportPreview {
    pub track: Track,
    pub size_bytes: Option<u64>,
    pub estimated_size: bool,
    pub bandwidth: u64,
    pub codec: String,
    pub stream_kind: String,
}

pub async fn preview_from_url_with_cookie(
    url: String,
    cookie: &str,
) -> Result<ImportPreview, String> {
    let video_id =
        extract_video_id(&url).ok_or_else(|| "Paste a supported YouTube URL first.".to_string())?;
    let resolved = resolve_video(&video_id, cookie).await?;
    let stream = best_stream(&resolved)?;
    remember_audio(
        &video_id,
        cookie,
        playable_streams(&resolved)
            .into_iter()
            .map(|stream| stream.url)
            .collect(),
    )?;
    let (size_bytes, estimated_size) = stream
        .size
        .map(|size| (Some(size), false))
        .unwrap_or_else(|| (estimated_stream_size(&resolved, &stream), true));

    Ok(ImportPreview {
        track: Track {
            source: SOURCE_YOUTUBE.to_string(),
            id: resolved.video_id,
            media_id: String::new(),
            stream_url: String::new(),
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

pub async fn resolve_audio_with_cookie(
    track_id: &str,
    cookie: &str,
) -> Result<crate::streaming::RemoteMedia, String> {
    let id = extract_video_id(track_id).ok_or("Invalid YouTube track ID.")?;
    if let Some(urls) = take_cached_audio(&id, cookie)? {
        return Ok(crate::streaming::RemoteMedia {
            client: crate::streaming::media_client(&format!("{ORIGIN}/watch?v={id}"))?,
            urls,
        });
    }
    let resolved = resolve_video(&id, cookie).await?;
    let streams = playable_streams(&resolved);
    if streams.is_empty() {
        return Err(youtube_status_error(&resolved.player_responses));
    }
    Ok(crate::streaming::RemoteMedia {
        client: crate::streaming::media_client(&resolved.webpage_url)?,
        urls: streams.into_iter().map(|stream| stream.url).collect(),
    })
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

// Import already resolved a playable stream. Reuse it once for immediate
// playback instead of triggering another independent YouTube challenge.
// Signed URLs never enter Track/state/export. Limit both age and entry count.
#[derive(Default)]
struct PreviewAudioCache(Vec<PreviewAudio>);

struct PreviewAudio {
    video_id: String,
    session: [u8; 20],
    created: Instant,
    urls: Vec<String>,
}

impl PreviewAudioCache {
    fn take(&mut self, id: &str, session: &[u8; 20], now: Instant) -> Option<Vec<String>> {
        self.0.retain(|entry| {
            now.saturating_duration_since(entry.created) < Duration::from_secs(120)
        });
        let index = self
            .0
            .iter()
            .position(|entry| entry.video_id == id && &entry.session == session)?;
        Some(self.0.remove(index).urls)
    }

    fn insert(&mut self, entry: PreviewAudio) {
        self.0.retain(|old| {
            entry.created.saturating_duration_since(old.created) < Duration::from_secs(120)
                && !(old.video_id == entry.video_id && old.session == entry.session)
        });
        if self.0.len() >= 8 {
            self.0.remove(0);
        }
        self.0.push(entry);
    }
}

fn preview_cache() -> &'static Mutex<PreviewAudioCache> {
    static CACHE: OnceLock<Mutex<PreviewAudioCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(PreviewAudioCache::default()))
}

fn session_key(cookie: &str) -> Result<[u8; 20], String> {
    // This is only a cache partition, never an authentication credential.
    Ok(Sha1::digest(normalize_cookie(cookie)?.as_bytes()).into())
}

fn remember_audio(id: &str, cookie: &str, urls: Vec<String>) -> Result<(), String> {
    let session = session_key(cookie)?;
    if let Ok(mut cache) = preview_cache().lock() {
        cache.insert(PreviewAudio {
            video_id: id.into(),
            session,
            created: Instant::now(),
            urls,
        });
    }
    Ok(())
}

fn take_cached_audio(id: &str, cookie: &str) -> Result<Option<Vec<String>>, String> {
    let session = session_key(cookie)?;
    Ok(preview_cache()
        .lock()
        .ok()
        .and_then(|mut cache| cache.take(id, &session, Instant::now())))
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

pub fn normalize_cookie(input: &str) -> Result<String, String> {
    let error =
        "Invalid YouTube cookie. Paste only the Cookie request header (name=value; name=value).";
    // Validate before trimming so CR/LF cannot be silently accepted.
    if input
        .bytes()
        .any(|byte| byte.is_ascii_control() || !byte.is_ascii())
    {
        return Err(error.into());
    }
    let mut cookie = input.trim();
    if cookie
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("cookie:"))
    {
        cookie = cookie[7..].trim();
    }
    if cookie.is_empty() {
        return Ok(String::new());
    }
    for pair in cookie.split(';') {
        let (name, value) = pair.trim().split_once('=').ok_or(error)?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
            || value
                .bytes()
                .any(|byte| byte <= b' ' || byte == b',' || byte == b'\\')
        {
            return Err(error.into());
        }
    }
    Ok(cookie.into())
}

fn session_headers(cookie: &str) -> Result<reqwest::header::HeaderMap, String> {
    use reqwest::header::{COOKIE, HeaderMap, HeaderValue, ORIGIN as ORIGIN_HEADER, REFERER};
    let cookie = normalize_cookie(cookie)?;
    let mut headers = HeaderMap::new();
    headers.insert(REFERER, HeaderValue::from_static(ORIGIN));
    headers.insert(ORIGIN_HEADER, HeaderValue::from_static(ORIGIN));
    if !cookie.is_empty() {
        let mut value =
            HeaderValue::from_str(&cookie).map_err(|_| "Invalid YouTube cookie header.")?;
        value.set_sensitive(true);
        headers.insert(COOKIE, value);
    }
    Ok(headers)
}

fn session_authorization(cookie: &str, timestamp: u64) -> Option<reqwest::header::HeaderValue> {
    let pairs: Vec<_> = cookie
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .collect();
    let sapisid = ["SAPISID", "__Secure-3PAPISID"].iter().find_map(|name| {
        pairs
            .iter()
            .find(|(key, value)| key == name && !value.is_empty())
            .map(|(_, value)| *value)
    })?;
    let digest = Sha1::digest(format!("{timestamp} {sapisid} {ORIGIN}").as_bytes());
    let digest: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    let mut header =
        reqwest::header::HeaderValue::from_str(&format!("SAPISIDHASH {timestamp}_{digest}"))
            .ok()?;
    header.set_sensitive(true);
    Some(header)
}

fn client(cookie: &str) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(20))
        // This client is private to the fixed YouTube endpoints. A redirected
        // login/consent page must not receive the manually supplied credentials.
        .redirect(reqwest::redirect::Policy::none())
        .default_headers(session_headers(cookie)?)
        .build()
        .map_err(|err| err.to_string())
}

async fn resolve_video(video_id: &str, cookie: &str) -> Result<ResolvedVideo, String> {
    let cookie = normalize_cookie(cookie)?;
    let webpage_url = format!("{ORIGIN}/watch?v={video_id}");
    let session_client = client(&cookie)?;
    let page_response = session_client
        .get(&webpage_url)
        .send()
        .await
        .map_err(|err| format!("YouTube page unavailable: {err}"))?
        .error_for_status()
        .map_err(|err| format!("YouTube page unavailable: {err}"))?;
    if page_response.status().is_redirection() {
        return Err("YouTube redirected to verification or consent. Complete it on YouTube and apply a fresh session in Settings.".into());
    }
    let webpage = page_response
        .text()
        .await
        .map_err(|err| format!("YouTube page read failed: {err}"))?;

    let mut player_responses = Vec::new();
    if let Some(response) = extract_json_after_marker(&webpage, "ytInitialPlayerResponse")
        && let Ok(value) = serde_json::from_str::<Value>(&response)
    {
        player_responses.push(value);
    }

    if let Some(api_key) = extract_innertube_api_key(&webpage) {
        let anonymous_client = client("")?;
        let mut clients = innertube_clients();
        // Browser cookies authenticate the WEB API; mobile clients use their
        // own authentication protocol and remain anonymous fallbacks.
        if !cookie.is_empty() {
            clients.rotate_right(1);
        }
        for spec in clients {
            if player_responses
                .iter()
                .any(|response| !collect_streams(response).is_empty())
            {
                break;
            }
            let web = spec.name == "WEB";
            let request_client = if web {
                &session_client
            } else {
                &anonymous_client
            };
            if let Ok(response) = call_innertube_player(
                request_client,
                &api_key,
                video_id,
                spec,
                if web { &cookie } else { "" },
            )
            .await
            {
                player_responses.push(response);
            }
        }
    }

    let details = player_responses
        .iter()
        .find_map(|response| response.get("videoDetails"))
        .ok_or_else(|| youtube_status_error(&player_responses))?;
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
    cookie: &str,
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
        "context": {"client": client_context},
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
    let mut request = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("x-youtube-client-name", spec.header_name)
        .header("x-youtube-client-version", spec.version)
        .header(reqwest::header::USER_AGENT, spec.user_agent)
        .json(&body);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if let Some(authorization) = session_authorization(cookie, timestamp) {
        request = request
            .header(reqwest::header::AUTHORIZATION, authorization)
            .header("x-goog-authuser", "0")
            .header("x-origin", ORIGIN);
    }
    request
        .send()
        .await
        .map_err(|err| format!("YouTube player API unavailable: {err}"))?
        .error_for_status()
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
        target.insert(key.to_owned(), value.to_owned());
    }
}

fn best_stream(video: &ResolvedVideo) -> Result<MediaStream, String> {
    playable_streams(video)
        .into_iter()
        .next()
        .ok_or_else(|| youtube_status_error(&video.player_responses))
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

fn youtube_status_error(responses: &[Value]) -> String {
    // A usable public session can coexist with rejected fallback clients.
    // Do not claim sign-in is necessary when the remaining problem is decoding
    // YouTube's signed/adaptive-only transport.
    if responses
        .iter()
        .any(|response| response["playabilityStatus"]["status"] == "OK")
    {
        return no_stream_error();
    }
    let status = responses.iter().find_map(|response| {
        let playability = response.get("playabilityStatus")?;
        let state = playability.get("status").and_then(Value::as_str)?;
        let reason = playability
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or_default();
        Some((state, reason))
    });
    match status {
        Some(("LOGIN_REQUIRED", reason)) if !reason.is_empty() => format!(
            "YouTube requires verification or sign-in for this request: {reason}. Apply or refresh a manually copied session cookie in Settings > General > YouTube, then retry."
        ),
        Some((_, reason)) if !reason.is_empty() => {
            format!("YouTube cannot play this video: {reason}")
        }
        _ => {
            "YouTube metadata is missing. The video page did not provide playable metadata.".into()
        }
    }
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
                    .map(|value| value.div_ceil(1000)),
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
    let url = parts.iter().find(|(key, _)| key == "url")?.1.to_owned();
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
        if let Some((item_key, value)) = part.split_once('=')
            && item_key.eq_ignore_ascii_case(key)
        {
            return Some(value);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_youtube_urls() {
        for url in [
            "https://youtu.be/sy3nQLseSmc?list=RDsy3nQLseSmc",
            "https://www.youtube.com/watch?v=sy3nQLseSmc&list=RDsy3nQLseSmc&start_radio=1",
        ] {
            assert_eq!(extract_video_id(url).as_deref(), Some("sy3nQLseSmc"));
        }
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
    fn extracts_balanced_json_with_strings() -> Result<(), String> {
        let text = r#"x ytInitialPlayerResponse = {"a":"};","b":{"c":1}}; y"#;
        let value = extract_json_after_marker(text, "ytInitialPlayerResponse")
            .ok_or_else(|| "no json".to_string())?;
        assert_eq!(value, r#"{"a":"};","b":{"c":1}}"#);
        Ok(())
    }

    #[test]
    fn explains_youtube_verification_errors() {
        let responses = vec![json!({
            "playabilityStatus": {
                "status": "LOGIN_REQUIRED",
                "reason": "Sign in to confirm you’re not a bot"
            }
        })];
        let error = youtube_status_error(&responses);
        assert!(error.contains("verification or sign-in"));
        assert!(error.contains("not a bot"));
        let mut mixed = responses;
        mixed.push(json!({"playabilityStatus":{"status":"OK"}}));
        assert_eq!(youtube_status_error(&mixed), no_stream_error());
    }

    #[test]
    fn accepts_a_manual_cookie_header_without_exposing_it() {
        let headers = session_headers("Cookie: SID=example; HSID=another-secret").unwrap();
        assert_eq!(
            headers[reqwest::header::COOKIE],
            "SID=example; HSID=another-secret"
        );
        assert!(headers[reqwest::header::COOKIE].is_sensitive());
        assert!(!format!("{headers:?}").contains("another-secret"));
        assert!(
            !session_headers("")
                .unwrap()
                .contains_key(reqwest::header::COOKIE)
        );
    }

    #[test]
    fn rejects_malformed_cookie_without_echoing_credentials() {
        for input in [
            "SID=secret\r\nInjected: x",
            "SID=secret\n",
            "Set-Cookie: SID=secret",
            "SID=secret; invalid",
            "secret",
        ] {
            let error = normalize_cookie(input).unwrap_err();
            assert!(!error.contains("secret"));
        }
        assert_eq!(
            normalize_cookie(" cookie: SID=a=b; PREF=hl=en ").unwrap(),
            "SID=a=b; PREF=hl=en"
        );
    }

    #[tokio::test]
    async fn sends_session_header_but_does_not_follow_redirects() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                let mut buffer = [0; 1024];
                let n = socket.read(&mut buffer).await.unwrap();
                assert_ne!(n, 0);
                request.extend_from_slice(&buffer[..n]);
            }
            assert!(
                String::from_utf8(request)
                    .unwrap()
                    .to_lowercase()
                    .contains("cookie: sid=test-session")
            );
            socket.write_all(format!("HTTP/1.1 302 Found\r\nLocation: http://{address}/other\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
        });
        let response = client("SID=test-session")
            .unwrap()
            .get(format!("http://{address}/"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::FOUND);
        server.await.unwrap();
    }

    #[test]
    fn signs_web_session_with_timestamp_and_origin() {
        let header = session_authorization("SID=irrelevant; SAPISID=test-sapisid", 123).unwrap();
        assert_eq!(
            header,
            "SAPISIDHASH 123_26915822e4c31253bfd791dae5f386f3ffea4b4d"
        );
        assert!(header.is_sensitive());
        assert_eq!(
            Some(header),
            session_authorization("__Secure-3PAPISID=test-sapisid", 123)
        );
        assert!(session_authorization("SID=irrelevant", 123).is_none());
    }

    #[test]
    fn preview_stream_is_reused_once_without_crossing_sessions_or_expiry() {
        let now = Instant::now();
        let mut cache = PreviewAudioCache::default();
        let entry = || PreviewAudio {
            video_id: "x2OfvEnMviU".into(),
            session: [1; 20],
            created: now,
            urls: vec!["https://example.test/audio".into()],
        };
        cache.insert(entry());
        assert!(cache.take("x2OfvEnMviU", &[2; 20], now).is_none());
        assert!(cache.take("YJDt1rSFCms", &[1; 20], now).is_none());
        assert!(cache.take("x2OfvEnMviU", &[1; 20], now).is_some());
        assert!(cache.take("x2OfvEnMviU", &[1; 20], now).is_none());
        cache.insert(entry());
        assert!(
            cache
                .take("x2OfvEnMviU", &[1; 20], now + Duration::from_secs(120))
                .is_none()
        );
        for i in 0..12 {
            cache.insert(PreviewAudio {
                video_id: i.to_string(),
                ..entry()
            });
        }
        assert_eq!(cache.0.len(), 8);
    }

    #[tokio::test]
    #[ignore]
    async fn live_resolves_youtube_stream() -> Result<(), String> {
        let video = resolve_video("iKbylgzysHw", "").await?;
        let stream = best_stream(&video)?;
        assert!(stream.bandwidth > 0);

        let client = client("")?;
        let chunk = client
            .get(&stream.url)
            .header(reqwest::header::REFERER, video.webpage_url.as_str())
            .header(reqwest::header::RANGE, "bytes=0-63")
            .send()
            .await
            .map_err(|err| format!("stream request: {err}"))?
            .chunk()
            .await
            .map_err(|err| format!("stream read: {err}"))?
            .ok_or_else(|| "no stream bytes".to_string())?;
        if chunk.is_empty() {
            return Err("empty stream chunk".to_string());
        }
        Ok(())
    }
}
