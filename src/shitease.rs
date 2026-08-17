use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

const SHITEASE_ORIGIN: &str = "https://music.163.com";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShiteaseSong {
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
pub struct ShiteaseSongUrl {
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShiteaseLyricResponse {
    pub lyric: Option<String>,
    pub tlyric: Option<String>,
    pub yrc: Option<String>,
    pub source: Option<String>,
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

fn map_song(song: &Value) -> Option<ShiteaseSong> {
    let id = song.get("id")?.clone();
    let name = text(song.get("name"));
    if name.is_empty() {
        return None;
    }
    let album = song
        .get("al")
        .or_else(|| song.get("album"))
        .unwrap_or(&Value::Null);
    Some(ShiteaseSong {
        provider: Some("shitease".to_string()),
        source: Some("shitease".to_string()),
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
) -> Result<Vec<ShiteaseSong>, String> {
    search_direct_offset(client, keywords, limit, 0).await
}

async fn search_direct_offset(
    client: &reqwest::Client,
    keywords: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<ShiteaseSong>, String> {
    let params = [
        ("s", keywords.to_string()),
        ("type", "1".to_string()),
        ("offset", offset.to_string()),
        ("total", "true".to_string()),
        ("limit", limit.to_string()),
        ("csrf_token", String::new()),
    ];
    let body: Value = client
        .post(format!("{SHITEASE_ORIGIN}/api/cloudsearch/pc"))
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Music search unavailable: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Invalid music search response: {e}"))?;

    let songs = body
        .get("result")
        .and_then(|result| result.get("songs"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(songs.iter().filter_map(map_song).collect())
}

fn next_random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn shuffle<T>(items: &mut [T], state: &mut u64) {
    for index in (1..items.len()).rev() {
        let swap = (next_random(state) as usize) % (index + 1);
        items.swap(index, swap);
    }
}

async fn playable_song_ids(
    client: &reqwest::Client,
    songs: &[ShiteaseSong],
    quality: &str,
) -> Result<HashSet<String>, String> {
    let ids = songs
        .iter()
        .map(|song| song_id(&song.id))
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();
    let mut playable = HashSet::new();

    if ids.is_empty() {
        return Ok(playable);
    }

    let br = bitrate(quality);
    for chunk in ids.chunks(50) {
        let ids_json = format!("[{}]", chunk.join(","));
        let body: Value = client
            .get(format!(
                "{SHITEASE_ORIGIN}/api/song/enhance/player/url?ids={}&br={}",
                urlencoding::encode(&ids_json),
                br
            ))
            .send()
            .await
            .map_err(|e| format!("Playable filter unavailable: {e}"))?
            .json()
            .await
            .unwrap_or(Value::Null);

        if let Some(items) = body.get("data").and_then(Value::as_array) {
            for item in items {
                let id = item.get("id").map(song_id).unwrap_or_default();
                let url = item.get("url").and_then(Value::as_str).unwrap_or_default();
                let code_ok = item.get("code").and_then(Value::as_i64).unwrap_or(200) == 200;
                if !id.is_empty() && !url.is_empty() && code_ok {
                    playable.insert(id);
                }
            }
        }
    }

    Ok(playable)
}

async fn filter_playable_songs(
    client: &reqwest::Client,
    songs: Vec<ShiteaseSong>,
) -> Result<Vec<ShiteaseSong>, String> {
    let playable = playable_song_ids(client, &songs, "exhigh").await?;
    Ok(songs
        .into_iter()
        .filter(|song| playable.contains(&song_id(&song.id)))
        .collect())
}

fn bitrate(quality: &str) -> u32 {
    match quality {
        "lossless" => 1_411_000,
        "hires" => 1_999_000,
        "standard" => 128_000,
        _ => 320_000,
    }
}

pub async fn search_shitease_songs(
    keywords: String,
    limit: Option<u32>,
    _api_base_url: Option<String>,
) -> Result<Vec<ShiteaseSong>, String> {
    let keywords = keywords.trim().to_string();
    if keywords.is_empty() {
        return Ok(Vec::new());
    }
    let client = client()?;
    let target = limit.unwrap_or(12).clamp(1, 99);
    let raw_limit = target.saturating_mul(3).clamp(target, 99);
    let songs = search_direct(&client, &keywords, raw_limit).await?;
    let mut playable = filter_playable_songs(&client, songs).await?;
    playable.truncate(target as usize);
    Ok(playable)
}

pub async fn get_shitease_song_url(
    id: String,
    quality: Option<String>,
    _api_base_url: Option<String>,
) -> Result<ShiteaseSongUrl, String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("Missing song id".to_string());
    }

    let quality = quality.unwrap_or_else(|| "exhigh".to_string());
    let br = bitrate(&quality);

    let body: Value = client()?
        .get(format!(
            "{SHITEASE_ORIGIN}/api/song/enhance/player/url?ids=%5B{}%5D&br={}",
            urlencoding::encode(id),
            br
        ))
        .send()
        .await
        .map_err(|e| format!("Song URL unavailable: {e}"))?
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
        return Ok(ShiteaseSongUrl {
            url: Some(url),
            playable: Some(true),
            trial: Some(data.get("freeTrialInfo").is_some_and(|v| !v.is_null())),
            level: Some(quality),
            quality: Some(format!("{}k", br / 1000)),
            reason: None,
            message: None,
            logged_in: Some(false),
            vip_label: None,
        });
    }

    Err(data
        .get("code")
        .and_then(Value::as_i64)
        .map(|code| format!("This track is not directly playable. Code: {code}."))
        .unwrap_or_else(|| "This track is not directly playable.".to_string()))
}

pub async fn get_shitease_lyric(
    id: String,
    _api_base_url: Option<String>,
) -> Result<ShiteaseLyricResponse, String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("Missing song id".to_string());
    }
    let body: Value = client()?
        .get(format!(
            "{SHITEASE_ORIGIN}/api/song/lyric?id={}&lv=-1&kv=-1&tv=-1",
            urlencoding::encode(id)
        ))
        .send()
        .await
        .map_err(|e| format!("Lyric unavailable: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Invalid lyric response: {e}"))?;

    Ok(ShiteaseLyricResponse {
        lyric: body
            .get("lrc")
            .and_then(|lrc| lrc.get("lyric"))
            .and_then(Value::as_str)
            .map(str::to_string),
        tlyric: body
            .get("tlyric")
            .and_then(|lrc| lrc.get("lyric"))
            .and_then(Value::as_str)
            .map(str::to_string),
        yrc: body
            .get("yrc")
            .and_then(|lrc| lrc.get("lyric"))
            .and_then(Value::as_str)
            .map(str::to_string),
        source: Some("shitease_direct".to_string()),
    })
}

pub async fn random_shitease_queue(
    count: Option<u32>,
    _api_base_url: Option<String>,
) -> Result<Vec<ShiteaseSong>, String> {
    let target = count.unwrap_or(50).clamp(1, 999);
    let seeds = [
        "热门歌曲",
        "经典歌曲",
        "流行音乐",
        "华语歌曲",
        "英文歌曲",
        "粤语歌曲",
        "日语歌曲",
        "韩语歌曲",
        "欧美流行",
        "怀旧金曲",
        "古风歌曲",
        "国风音乐",
        "古风纯音乐",
        "民谣",
        "摇滚",
        "电子音乐",
        "R&B",
        "说唱",
        "爵士",
        "轻音乐",
        "纯音乐",
        "影视原声",
        "动漫歌曲",
        "游戏原声",
        "治愈系",
        "夜晚听歌",
        "工作学习",
        "运动节奏",
        "旅行音乐",
        "新歌热门",
    ];
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut random_state = now ^ ((std::process::id() as u64) << 32) ^ 0x9e37_79b9_7f4a_7c15;
    let client = client()?;
    let mut songs = Vec::new();
    let mut seed_pool = seeds.to_vec();

    shuffle(&mut seed_pool, &mut random_state);

    let max_rounds =
        ((target as usize * 3).div_ceil(50) + seed_pool.len()).max(seed_pool.len() * 4);
    for seed in seed_pool.iter().cycle().take(max_rounds) {
        if songs.len() >= target as usize * 3 {
            break;
        }
        let limit = target.max(30).min(50);
        let page = (next_random(&mut random_state) % 8) as u32;
        let offset = page * limit;
        if let Ok(mut found) = search_direct_offset(&client, seed, limit, offset).await {
            shuffle(&mut found, &mut random_state);
            songs.append(&mut found);
        }
    }

    let mut seen = HashSet::new();
    songs.retain(|song| {
        let id = song_id(&song.id);
        !id.is_empty() && seen.insert(id)
    });
    let mut playable = filter_playable_songs(&client, songs).await?;
    shuffle(&mut playable, &mut random_state);
    playable.truncate(target as usize);
    Ok(playable)
}
