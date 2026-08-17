use serde_json::Value;
use std::time::Duration;

const API_URL: &str = "https://api.qishui.com/luna/pc/search/track";
const UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/124.0 Safari/537.36";

#[derive(Clone, Debug)]
pub struct QishuiSong {
    pub id: String,
    pub name: String,
    pub artist: String,
    pub album: String,
    pub cover: String,
    pub duration: u64,
    pub stream_url: String,
}

pub async fn search(keywords: String, limit: u32) -> Result<Vec<QishuiSong>, String> {
    let keywords = keywords.trim();
    if keywords.is_empty() {
        return Ok(Vec::new());
    }
    let url = format!(
        "{API_URL}?aid=386088&app_name=luna_pc&region=cn&device_platform=windows&device_type=Windows&version_name=3.0.0&version_code=30000000&q={}&cursor=0",
        urlencoding::encode(keywords)
    );
    let body: Value = reqwest::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|err| format!("Qishui client unavailable: {err}"))?
        .get(url)
        .send()
        .await
        .map_err(|err| format!("Qishui search unavailable: {err}"))?
        .json()
        .await
        .map_err(|err| format!("Invalid Qishui search response: {err}"))?;
    Ok(extract_tracks(&body)
        .into_iter()
        .take(limit as usize)
        .collect())
}

fn extract_tracks(body: &Value) -> Vec<QishuiSong> {
    let groups = body
        .get("result_groups")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    groups
        .flat_map(|group| {
            group
                .get("data")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(|item| {
            let track = item
                .pointer("/entity/track_wrapper/track")
                .or_else(|| item.pointer("/entity/track"))
                .or_else(|| item.get("track"))?;
            let id = track.get("id")?.to_string().trim_matches('"').to_string();
            let name = track
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if id.is_empty() || name.is_empty() {
                return None;
            }
            let artists = track
                .get("artists")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|artist| artist.get("name").and_then(Value::as_str))
                        .collect::<Vec<_>>()
                        .join(" / ")
                })
                .unwrap_or_default();
            let album = track.get("album").unwrap_or(&Value::Null);
            Some(QishuiSong {
                id,
                name,
                artist: artists,
                album: album
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                cover: album
                    .get("url_cover")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                duration: track
                    .get("duration")
                    .and_then(Value::as_u64)
                    .map(|value| if value > 10_000 { value / 1000 } else { value })
                    .unwrap_or(0),
                stream_url: String::new(),
            })
        })
        .collect()
}
