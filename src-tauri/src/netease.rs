use serde::{Deserialize, Serialize};
use serde_json::Value;

const NETEASE_ORIGIN: &str = "https://music.163.com";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NeteaseSong {
    pub provider: Option<String>,
    pub source: Option<String>,
    #[serde(rename = "type")]
    pub item_type: Option<String>,
    pub id: Value,
    pub name: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub cover: Option<String>,
    pub duration: Option<u64>,
    pub fee: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NeteaseSongUrl {
    pub url: Option<String>,
    pub playable: Option<bool>,
    pub trial: Option<bool>,
    pub level: Option<String>,
    pub quality: Option<String>,
    pub reason: Option<String>,
    pub message: Option<String>,
    pub logged_in: Option<bool>,
    pub vip_label: Option<String>,
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(UA)
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::REFERER,
                reqwest::header::HeaderValue::from_static("https://music.163.com/"),
            );
            headers
        })
        .build()
        .map_err(|e| e.to_string())
}

fn song_id(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn text(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn first_string(value: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .unwrap_or_default()
        .to_string()
}

fn artists(song: &Value) -> String {
    let raw = song.get("ar").or_else(|| song.get("artists"));
    raw.and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|artist| artist.get("name").and_then(Value::as_str))
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .unwrap_or_default()
}

fn map_song(song: &Value) -> Option<NeteaseSong> {
    let id = song.get("id")?.clone();
    let name = text(song.get("name"));
    if name.is_empty() {
        return None;
    }
    let album = song
        .get("al")
        .or_else(|| song.get("album"))
        .unwrap_or(&Value::Null);
    Some(NeteaseSong {
        provider: Some("netease".to_string()),
        source: Some("netease".to_string()),
        item_type: Some("song".to_string()),
        id,
        name,
        artist: Some(artists(song)),
        album: Some(first_string(album, &["name"])),
        cover: Some(first_string(album, &["picUrl", "coverUrl"])),
        duration: song
            .get("dt")
            .or_else(|| song.get("duration"))
            .and_then(Value::as_u64),
        fee: song.get("fee").and_then(Value::as_i64),
    })
}

async fn search_direct(
    client: &reqwest::Client,
    keywords: &str,
    limit: u32,
) -> Result<Vec<NeteaseSong>, String> {
    let params = [
        ("s", keywords.to_string()),
        ("type", "1".to_string()),
        ("offset", "0".to_string()),
        ("total", "true".to_string()),
        ("limit", limit.to_string()),
        ("csrf_token", String::new()),
    ];
    let body: Value = client
        .post(format!("{NETEASE_ORIGIN}/api/cloudsearch/pc"))
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("NetEase search unavailable: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Invalid NetEase search response: {e}"))?;

    let songs = body
        .get("result")
        .and_then(|result| result.get("songs"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(songs.iter().filter_map(map_song).collect())
}

pub async fn search_netease_songs(
    keywords: String,
    limit: Option<u32>,
    _api_base_url: Option<String>,
) -> Result<Vec<NeteaseSong>, String> {
    let keywords = keywords.trim().to_string();
    if keywords.is_empty() {
        return Ok(Vec::new());
    }
    search_direct(&client()?, &keywords, limit.unwrap_or(12).clamp(1, 50)).await
}

pub async fn get_netease_song_url(
    id: String,
    quality: Option<String>,
    _api_base_url: Option<String>,
) -> Result<NeteaseSongUrl, String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("Missing NetEase song id".to_string());
    }

    let br = match quality.as_deref().unwrap_or("exhigh") {
        "lossless" => 1_411_000,
        "hires" => 1_999_000,
        "standard" => 128_000,
        _ => 320_000,
    };

    let body: Value = client()?
        .get(format!(
            "{NETEASE_ORIGIN}/api/song/enhance/player/url?ids=%5B{}%5D&br={}",
            urlencoding::encode(id),
            br
        ))
        .send()
        .await
        .map_err(|e| format!("NetEase song URL unavailable: {e}"))?
        .json()
        .await
        .unwrap_or(Value::Null);

    let data = body
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .cloned()
        .unwrap_or(Value::Null);
    let direct_url = data.get("url").and_then(Value::as_str).map(str::to_string);
    if let Some(url) = direct_url {
        return Ok(NeteaseSongUrl {
            url: Some(url),
            playable: Some(true),
            trial: Some(data.get("freeTrialInfo").is_some_and(|v| !v.is_null())),
            level: Some(quality.unwrap_or_else(|| "exhigh".to_string())),
            quality: Some(format!("{}k", br / 1000)),
            reason: None,
            message: None,
            logged_in: Some(false),
            vip_label: None,
        });
    }

    Ok(NeteaseSongUrl {
        url: Some(format!("{NETEASE_ORIGIN}/song/media/outer/url?id={id}.mp3")),
        playable: Some(true),
        trial: Some(false),
        level: Some("outer".to_string()),
        quality: Some("public".to_string()),
        reason: data
            .get("code")
            .and_then(Value::as_i64)
            .map(|code| format!("netease_code_{code}")),
        message: Some(
            "Using NetEase public stream fallback; restricted tracks may not play.".to_string(),
        ),
        logged_in: Some(false),
        vip_label: None,
    })
}

pub async fn random_netease_queue(
    count: Option<u32>,
    _api_base_url: Option<String>,
) -> Result<Vec<NeteaseSong>, String> {
    let target = count.unwrap_or(50).clamp(1, 100);
    let seeds = [
        "华语 流行",
        "古风 器乐",
        "独立 民谣",
        "夜晚 轻音乐",
        "电子 氛围",
        "治愈 女声",
        "摇滚 现场",
        "爵士 piano",
        "国风 原声",
        "新歌 热门",
    ];
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as usize)
        .unwrap_or(0);
    let client = client()?;
    let mut songs = Vec::new();

    for offset in 0..seeds.len() {
        if songs.len() >= target as usize {
            break;
        }
        let seed = seeds[(now + offset * 3) % seeds.len()];
        let remaining = target as usize - songs.len();
        let limit = remaining.max(12).min(50) as u32;
        if let Ok(mut found) = search_direct(&client, seed, limit).await {
            songs.append(&mut found);
        }
    }

    let mut seen = std::collections::HashSet::new();
    songs.retain(|song| {
        let id = song_id(&song.id);
        !id.is_empty() && seen.insert(id)
    });
    if !songs.is_empty() {
        let rotate = now % songs.len();
        songs.rotate_left(rotate);
    }
    songs.truncate(target as usize);
    Ok(songs)
}
