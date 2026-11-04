//! Qishui's public catalogue and public track playback. No private session is used.
use crate::{
    music::track::{SOURCE_QISHUI, Track},
    providers::streaming::RemoteMedia,
};
use reqwest::Url;
use serde_json::Value;
use std::time::Duration;

const CATALOG: &str = "https://api-vehicle.volcengine.com/v2";
const SEO: &str = "https://beta-luna.douyin.com/luna/h5/seo_track";

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())
}

fn with_query(base: &str, pairs: &[(&str, &str)]) -> Result<Url, String> {
    let mut url = Url::parse(base).map_err(|e| e.to_string())?;
    {
        let mut query = url.query_pairs_mut();
        for (key, value) in pairs {
            query.append_pair(key, value);
        }
    }
    Ok(url)
}

async fn json(request: reqwest::RequestBuilder) -> Result<Value, String> {
    let mut response = request
        .send()
        .await
        .map_err(|e| format!("Qishui request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Qishui unavailable: {e}"))?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > 8 * 1024 * 1024 {
            return Err("Qishui response is too large.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Qishui returned an invalid response.")?;
    if value["errno"].as_i64().is_some_and(|code| code != 0)
        || value["status_info"]["code"]
            .as_i64()
            .is_some_and(|code| code != 0)
        || value.pointer("/ResponseMetadata/Error").is_some()
    {
        return Err("Qishui did not make this content available in the public session.".into());
    }
    Ok(value)
}

fn valid_id(id: &str) -> bool {
    (15..=24).contains(&id.len()) && id.bytes().all(|c| c.is_ascii_digit())
}

fn parse_song(item: &Value) -> Option<Track> {
    // The anonymous catalogue also returns VIP-only entries without usable audio.
    if item["source"].as_str() != Some("qishui")
        || item["unavailable"].as_bool() == Some(true)
        || item["qishui_label_info"]["only_vip_playable"].as_bool() == Some(true)
    {
        return None;
    }
    let id = item["item_id"].as_str()?;
    let name = item["title"].as_str()?.trim();
    if !valid_id(id) || name.is_empty() {
        return None;
    }
    Some(Track {
        source: SOURCE_QISHUI.into(),
        id: id.into(),
        media_id: String::new(),
        stream_url: String::new(),
        name: name.into(),
        artist: item["author_info"]["name"]
            .as_str()
            .unwrap_or("Unknown artist")
            .into(),
        album: item["album_info"]["name"]
            .as_str()
            .unwrap_or("Qishui Music")
            .into(),
        cover: item["cover_url"].as_str().unwrap_or_default().into(),
        duration: item["duration"].as_u64().filter(|n| *n > 0),
    })
}

pub async fn search(text: String, limit: u32) -> Result<Vec<Track>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let keyword = text;
    let limit = limit.clamp(1, 100).to_string();
    let url = with_query(
        &format!("{CATALOG}/search/type"),
        &[
            ("keyword", &keyword),
            ("search_type", "music"),
            ("search_source", "qishui"),
            ("limit", &limit),
            ("real_offset", "0"),
        ],
    )?;
    let body = json(client()?.get(url)).await?;
    let items = body
        .pointer("/data/list")
        .and_then(Value::as_array)
        .ok_or("Qishui search did not return a catalogue.")?;
    let mut seen = std::collections::HashSet::new();
    Ok(items
        .iter()
        .filter_map(parse_song)
        .filter(|song| seen.insert(song.id.clone()))
        .collect())
}

pub async fn random(limit: u32) -> Result<Vec<Track>, String> {
    let terms = [
        "流行",
        "民谣",
        "轻音乐",
        "摇滚",
        "电子",
        "钢琴",
        "古风",
        "爵士",
    ];
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as usize;
    search(terms[seed % terms.len()].into(), limit).await
}

fn trusted_url(text: &str, suffixes: &[&str]) -> Option<String> {
    let url = Url::parse(text).ok()?;
    let host = url.host_str()?;
    (url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && suffixes
            .iter()
            .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}"))))
    .then(|| url.into())
}

fn player_url(payload: &Value, id: &str) -> Result<(String, f64), String> {
    let track = &payload["seo_track"]["track"];
    if track["id"].as_str() != Some(id) {
        return Err("Qishui returned a different track.".into());
    }
    let duration = track["duration"]
        .as_f64()
        .filter(|v| *v > 0.0)
        .ok_or("Qishui did not provide a track duration.")?
        / 1000.0;
    let url = payload["track_player"]["url_player_info"]
        .as_str()
        .and_then(|url| trusted_url(url, &["douyin.com", "qishui.com"]))
        .ok_or("Qishui playback requires login or is not available for this track.")?;
    Ok((url, duration))
}

fn audio_urls(payload: &Value, duration: f64) -> Result<Vec<String>, String> {
    let list = payload
        .pointer("/Result/Data/PlayInfoList")
        .and_then(Value::as_array)
        .ok_or("Qishui did not return any public audio streams.")?;
    let mut candidates: Vec<_> = list
        .iter()
        .filter(|item| {
            let encrypted = item["PlayAuth"].as_str().is_some_and(|s| !s.is_empty())
                || item["EncryptionMethod"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty() && s != "none");
            !encrypted
                && item["Duration"]
                    .as_f64()
                    .is_some_and(|n| n >= duration - 2.0)
                && matches!(
                    item["Format"].as_str(),
                    Some("m4a" | "mp3" | "mp4" | "flac")
                )
        })
        .collect();
    candidates.sort_by_key(|item| std::cmp::Reverse(item["Bitrate"].as_u64().unwrap_or(0)));
    let urls: Vec<_> = candidates
        .into_iter()
        .flat_map(|item| {
            ["MainPlayUrl", "BackupPlayUrl"]
                .into_iter()
                .filter_map(move |key| item[key].as_str())
        })
        .filter_map(|url| {
            trusted_url(
                url,
                &[
                    "douyinvod.com",
                    "douyin.com",
                    "bytecdn.cn",
                    "byteoversea.com",
                    "muscdn.com",
                ],
            )
        })
        .collect();
    if urls.is_empty() {
        return Err("Qishui only offers a preview or protected audio for this track. Full playback is unavailable in the public session.".into());
    }
    Ok(urls)
}

pub async fn resolve_audio(id: &str) -> Result<RemoteMedia, String> {
    if !valid_id(id) {
        return Err("Invalid Qishui track ID.".into());
    }
    let client = client()?;
    let url = with_query(SEO, &[("track_id", id), ("device_platform", "web")])?;
    let payload = json(client.get(url).header("Referer", "https://www.douyin.com/")).await?;
    let (url, duration) = player_url(&payload, id)?;
    let urls = audio_urls(&json(client.get(url)).await?, duration)?;
    Ok(RemoteMedia {
        client: crate::providers::streaming::media_client("https://www.qishui.com/")?,
        urls,
    })
}

pub async fn lyric(id: &str) -> Result<String, String> {
    if !valid_id(id) {
        return Err("Invalid Qishui track ID.".into());
    }
    let url = with_query(
        &format!("{CATALOG}/custom/contents"),
        &[
            ("sources", "qishui"),
            ("need_author", "true"),
            ("need_album", "true"),
            ("item_ids", id),
        ],
    )?;
    let payload = json(client()?.get(url)).await?;
    let item = payload
        .pointer("/data/list")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|item| item["item_id"].as_str() == Some(id))
        })
        .ok_or("Qishui lyrics are unavailable.")?;
    Ok(item["lyric_info"]["lyric_text"]
        .as_str()
        .unwrap_or_default()
        .into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn catalogue_keeps_stable_ids_and_skips_vip_only_entries() {
        let mut item = json!({"item_id":"7676337516358617115","source":"qishui","title":"晴天","duration":237,"author_info":{"name":"晚风纯音"}});
        let song = parse_song(&item).unwrap();
        assert_eq!(song.source, SOURCE_QISHUI);
        assert_eq!(song.artist, "晚风纯音");
        assert_eq!(song.duration, Some(237));
        assert!(song.stream_url.is_empty());
        item["qishui_label_info"] = json!({"only_vip_playable":true});
        assert!(parse_song(&item).is_none());
    }
    #[test]
    fn refuses_previews_protected_audio_and_wrong_tracks() {
        let url = "https://example.douyinvod.com/audio.m4a";
        let mut item =
            json!({"Duration":237.336,"Format":"m4a","Bitrate":128000,"MainPlayUrl":url});
        let wrap = |item: Value| json!({"Result":{"Data":{"PlayInfoList":[item]}}});
        assert_eq!(audio_urls(&wrap(item.clone()), 237.336).unwrap(), vec![url]);
        item["Duration"] = json!(30);
        assert!(audio_urls(&wrap(item.clone()), 237.336).is_err());
        item["Duration"] = json!(237.336);
        item["PlayAuth"] = json!("protected");
        assert!(audio_urls(&wrap(item), 237.336).is_err());
        assert!(
            player_url(
                &json!({"seo_track":{"track":{"id":"wrong"}}}),
                "7676337516358617115"
            )
            .is_err()
        );
        assert!(trusted_url("https://douyin.com.evil.test/a", &["douyin.com"]).is_none());
    }
}
