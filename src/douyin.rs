//! Public Douyin video audio resolved with direct Rust HTTP requests.

use reqwest::Url;
use serde_json::Value;
use std::{path::Path, time::Duration};

const UA: &str =
    "Mozilla/5.0 (Linux; Android 10) AppleWebKit/537.36 Chrome/124.0.0.0 Mobile Safari/537.36";
const UNAVAILABLE: &str = "Douyin did not include playable audio/video metadata in its public page. This link cannot currently be resolved through direct HTTP requests.";

#[derive(Debug)]
pub struct Preview {
    pub id: String,
    pub title: String,
    pub author: String,
    pub cover: String,
    pub duration: Option<u64>,
    pub size: Option<u64>,
    urls: Vec<String>,
}

#[allow(dead_code)]
pub fn is_supported_url(text: &str) -> bool {
    share_url(text).is_some()
}

fn page_host(host: &str) -> bool {
    matches!(
        host,
        "v.douyin.com"
            | "www.douyin.com"
            | "douyin.com"
            | "m.douyin.com"
            | "www.iesdouyin.com"
            | "iesdouyin.com"
    )
}

fn video_id(url: &Url) -> Option<String> {
    if !page_host(url.host_str()?) {
        return None;
    }
    let parts: Vec<_> = url
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect();
    let id = match parts.as_slice() {
        ["video", id] | ["share", "video", id] => *id,
        _ => return None,
    };
    (id.len() >= 15 && id.len() <= 24 && id.bytes().all(|b| b.is_ascii_digit())).then(|| id.into())
}

/// Accept the entire copied share message, while restricting requests to Douyin.
fn share_url(text: &str) -> Option<Url> {
    for (index, _) in text.match_indices("https://") {
        let candidate = text[index..]
            .split(|c: char| {
                c.is_whitespace() || matches!(c, '"' | '<' | '>' | '）' | '，' | '。' | '！')
            })
            .next()?;
        let candidate = candidate.trim_end_matches([')', ']', ',', '.', '!', ';']);
        let Ok(url) = Url::parse(candidate) else {
            continue;
        };
        if !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
            continue;
        }
        if video_id(&url).is_some() {
            return Some(url);
        }
        if url.host_str() == Some("v.douyin.com") {
            let token = url.path().trim_matches('/');
            if !token.is_empty()
                && token.len() <= 80
                && token
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            {
                return Some(url);
            }
        }
    }
    None
}

fn page_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 8 {
                attempt.error("Too many Douyin redirects")
            } else if attempt.url().scheme() == "https"
                && attempt.url().host_str().is_some_and(page_host)
            {
                attempt.follow()
            } else {
                attempt.error("Unexpected Douyin redirect")
            }
        }))
        .build()
        .map_err(|error| error.to_string())
}

pub async fn preview(text: &str) -> Result<Preview, String> {
    let url = share_url(text).ok_or("Paste a Douyin video link or its copied share message.")?;
    let client = page_client()?;
    let (id, initial) = if let Some(id) = video_id(&url) {
        (id, None)
    } else {
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|error| format!("Douyin share link unavailable: {error}"))?
            .error_for_status()
            .map_err(|error| error.to_string())?;
        let id =
            video_id(response.url()).ok_or("This Douyin share link does not point to a video.")?;
        (id, Some(read_page(response).await?))
    };
    let page = format!("https://m.douyin.com/share/video/{id}");
    let mut resolved = initial
        .as_deref()
        .and_then(router_data)
        .and_then(|value| from_data(&value, &id));
    if resolved.is_none() {
        resolved = direct_detail(&client, &id).await;
    }
    if resolved.is_none()
        && let Ok(response) = client.get(&page).send().await
        && response.status().is_success()
        && let Ok(html) = read_page(response).await
    {
        resolved = router_data(&html).and_then(|value| from_data(&value, &id));
    }
    let mut result = resolved.ok_or(UNAVAILABLE)?;
    result.size = probe(&result.urls).await?;
    Ok(result)
}

// Douyin occasionally exposes this response without its browser-only request
// signature. Keep it as a direct HTTP attempt so the resolver remains pure Rust;
// a 403 or an empty response simply falls through to the public-page parser.
async fn direct_detail(client: &reqwest::Client, id: &str) -> Option<Preview> {
    let url = Url::parse_with_params(
        "https://www.douyin.com/aweme/v1/web/aweme/detail/",
        [
            ("device_platform", "webapp"),
            ("aid", "6383"),
            ("channel", "channel_pc_web"),
            ("aweme_id", id),
            ("request_source", "600"),
            ("origin_type", "video_page"),
            ("update_version_code", "170400"),
            ("pc_client_type", "1"),
            ("support_h265", "1"),
            ("support_dash", "0"),
            ("version_code", "190500"),
            ("version_name", "19.5.0"),
        ],
    )
    .ok()?;
    let response = client
        .get(url)
        .header("Referer", "https://www.douyin.com/")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let body = response.bytes().await.ok()?;
    if body.len() > 8 * 1024 * 1024 {
        return None;
    }
    let value: Value = serde_json::from_slice(&body).ok()?;
    from_detail(&value, id)
}

fn from_detail(value: &Value, id: &str) -> Option<Preview> {
    let item = value.get("aweme_detail").or_else(|| value.get("item"))?;
    if item["aweme_id"].as_str()? != id {
        return None;
    }
    let urls: Vec<_> = item["video"]["play_addr"]["url_list"]
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .filter_map(media_url)
        .collect();
    if urls.is_empty() {
        return None;
    }
    Some(Preview {
        id: id.into(),
        title: item["desc"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("Douyin video")
            .into(),
        author: item["author"]["nickname"]
            .as_str()
            .unwrap_or("Douyin")
            .into(),
        cover: item["video"]["cover"]["url_list"][0]
            .as_str()
            .unwrap_or_default()
            .into(),
        duration: item["video"]["duration"]
            .as_u64()
            .filter(|n| *n > 0)
            .map(|ms| ms.div_ceil(1000)),
        size: None,
        urls,
    })
}

async fn read_page(mut response: reqwest::Response) -> Result<String, String> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        if bytes.len() + chunk.len() > 8 * 1024 * 1024 {
            return Err("Douyin page is too large.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

fn router_data(html: &str) -> Option<Value> {
    let rest = html.split_once("window._ROUTER_DATA")?.1;
    let rest = rest.split_once('=')?.1.trim_start();
    // serde consumes just the first complete JSON value, preserving braces in strings.
    serde_json::Deserializer::from_str(rest)
        .into_iter::<Value>()
        .next()?
        .ok()
}

fn from_data(data: &Value, id: &str) -> Option<Preview> {
    let item = data["loaderData"]
        .as_object()?
        .values()
        .filter_map(|page| page["videoInfoRes"]["item_list"].as_array())
        .flatten()
        .find(|item| item["aweme_id"].as_str() == Some(id))?;
    // music.play_url may be a short BGM clip. Use the video's own soundtrack.
    let urls: Vec<_> = item["video"]["play_addr"]["url_list"]
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .filter_map(media_url)
        .collect();
    if urls.is_empty() {
        return None;
    }
    Some(Preview {
        id: id.into(),
        title: item["desc"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("Douyin video")
            .into(),
        author: item["author"]["nickname"]
            .as_str()
            .unwrap_or("Douyin")
            .into(),
        cover: item["video"]["cover"]["url_list"][0]
            .as_str()
            .unwrap_or("")
            .into(),
        duration: item["video"]["duration"]
            .as_u64()
            .filter(|duration| *duration > 0)
            .map(|ms| ms.div_ceil(1000)),
        size: None,
        urls,
    })
}

fn media_url(text: &str) -> Option<String> {
    let url = Url::parse(text).ok()?;
    let host = url.host_str()?;
    let allowed = [
        "douyin.com",
        "douyinvod.com",
        "snssdk.com",
        "amemv.com",
        "bytecdn.cn",
    ]
    .iter()
    .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")));
    (allowed
        && url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none())
    .then(|| url.into())
}

fn media_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(UA)
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::REFERER,
                reqwest::header::HeaderValue::from_static("https://m.douyin.com/"),
            );
            headers
        })
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| error.to_string())
}

async fn probe(urls: &[String]) -> Result<Option<u64>, String> {
    let client = media_client()?;
    for url in urls {
        let Ok(response) = client
            .get(url)
            .header("Referer", "https://m.douyin.com/")
            .header("Range", "bytes=0-31")
            .send()
            .await
        else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        if !content_type.starts_with("video/")
            && !content_type.starts_with("audio/")
            && !content_type.starts_with("application/octet-stream")
        {
            continue;
        }
        let total = response
            .headers()
            .get("content-range")
            .and_then(|value| value.to_str().ok())
            .and_then(|range| range.rsplit('/').next())
            .and_then(|total| total.parse().ok());
        return Ok(total.or_else(|| {
            (response.status() == reqwest::StatusCode::OK)
                .then(|| response.content_length())
                .flatten()
        }));
    }
    Err("Douyin supplied a media URL, but the stream is unavailable in this session.".into())
}

pub fn remote(preview: &Preview) -> Result<crate::streaming::RemoteMedia, String> {
    Ok(crate::streaming::RemoteMedia {
        client: media_client()?,
        urls: preview.urls.clone(),
    })
}

#[allow(dead_code)] // Explicit full-download CLI, separate from progressive playback.
pub async fn download<F: FnMut(u64, Option<u64>)>(
    preview: &Preview,
    path: &Path,
    mut progress: F,
) -> Result<(), String> {
    let client = media_client()?;
    let temp = path.with_extension(format!(
        "{}.download",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    // Also removes the incomplete fallback download if its loading task is aborted.
    struct Partial(std::path::PathBuf);
    impl Drop for Partial {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _partial = Partial(temp.clone());
    let mut last = String::from("No Douyin video stream is available.");
    for url in &preview.urls {
        let result = crate::download::download_url_to_path_with_progress(
            &client,
            url,
            "https://m.douyin.com/",
            preview.size,
            &temp,
            "Douyin video audio",
            "Douyin returned an empty video.",
            &mut progress,
        )
        .await;
        match result {
            Ok(()) => {
                let check_path = temp.clone();
                let valid = tokio::task::spawn_blocking(move || {
                    let file =
                        std::fs::File::open(check_path).map_err(|error| error.to_string())?;
                    let mut audio = rodio::Decoder::try_from(file)
                        .map_err(|error| format!("Douyin audio cannot be decoded: {error}"))?;
                    audio
                        .next()
                        .ok_or_else(|| "Douyin video has no decodable audio.".to_string())?;
                    Ok::<_, String>(())
                })
                .await
                .map_err(|error| error.to_string())
                .and_then(|result| result);
                match valid {
                    Ok(()) => {
                        if let Err(error) = tokio::fs::rename(&temp, path).await {
                            let _ = tokio::fs::remove_file(&temp).await;
                            return Err(format!("Could not save Douyin audio cache: {error}"));
                        }
                        return Ok(());
                    }
                    Err(error) => last = error,
                }
            }
            Err(error) => last = error,
        }
        let _ = tokio::fs::remove_file(&temp).await;
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_share_text_and_canonical_links_but_rejects_other_hosts() {
        let url = share_url(
            "1.28 fOX:/ 【中文 DJ 舞曲 100 首】 https://v.douyin.com/gYsL-kehIgg/ 复制此链接",
        )
        .unwrap();
        assert_eq!(url.as_str(), "https://v.douyin.com/gYsL-kehIgg/");
        let url = share_url("https://www.douyin.com/video/7676446085899963684").unwrap();
        assert_eq!(video_id(&url).as_deref(), Some("7676446085899963684"));
        for input in [
            "https://douyin.com.evil.test/video/7676446085899963684",
            "https://evil.test/?https://v.douyin.com/",
            "https://v.douyin.com/",
            "https://user@v.douyin.com/test",
            "https://www.douyin.com/user/123",
        ] {
            assert!(!is_supported_url(input), "{input}");
        }
    }

    #[test]
    fn parses_matching_video_not_the_short_music_clip() {
        let data = serde_json::json!({"loaderData":{"video_(id)/page":{"videoInfoRes":{"item_list":[{
            "aweme_id":"7676446085899963684","desc":"DJ {mix}","author":{"nickname":"音乐法师"},
            "video":{"duration":2336234,"play_addr":{"url_list":["https://m.douyin.com/aweme/v1/playwm/?video_id=test"]}},
            "music":{"duration":30,"play_url":{"url_list":["https://example.com/clip.mp3"]}}
        }]}}}});
        let html = format!("<script>window._ROUTER_DATA = {data};</script>");
        let parsed = router_data(&html).unwrap();
        let preview = from_data(&parsed, "7676446085899963684").unwrap();
        assert_eq!(preview.duration, Some(2337));
        assert_eq!(preview.author, "音乐法师");
        assert_eq!(preview.title, "DJ {mix}");
        assert!(from_data(&parsed, "1234567890123456789").is_none());
        assert!(from_data(&serde_json::json!({}), "7676446085899963684").is_none());
    }

    #[test]
    fn parses_direct_detail_shape() {
        let value = serde_json::json!({"aweme_detail": {
            "aweme_id": "7681254988844174642", "desc": "DJ mix",
            "author": {"nickname": "DJ"},
            "video": {"duration": 1000, "play_addr": {"url_list": ["https://v26-web.douyinvod.com/a/media.mp4"]}, "cover": {"url_list": ["https://p.douyinpic.com/cover.jpg"]}}
        }});
        let preview = from_detail(&value, "7681254988844174642").unwrap();
        assert_eq!(preview.title, "DJ mix");
        assert_eq!(preview.duration, Some(1));
    }
}
