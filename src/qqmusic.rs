use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};
use serde_json::Value;

// This endpoint still serves anonymous song metadata. The newer
// `client_search_cp` endpoint returns an empty list without a web session.
const SEARCH_URL: &str = "https://c.y.qq.com/soso/fcgi-bin/search_cp";
const PLAY_URL: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";
const STREAM_HOST: &str = "https://isure.stream.qqmusic.qq.com/";
const COVER_URL: &str = "https://y.gtimg.cn/music/photo_new/T002R300x300M000";
const UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/124.0 Safari/537.36";

#[derive(Clone, Debug)]
pub struct QqMusicSong {
    pub songmid: String,
    pub name: String,
    pub artist: String,
    pub album: String,
    pub cover: String,
    pub duration: u64,
}

fn client() -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(UA));
    headers.insert(REFERER, HeaderValue::from_static("https://y.qq.com/"));
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|err| format!("QQ Music client unavailable: {err}"))
}

pub async fn search(keywords: String, limit: u32) -> Result<Vec<QqMusicSong>, String> {
    let keywords = keywords.trim();
    if keywords.is_empty() {
        return Ok(Vec::new());
    }
    let limit = limit.clamp(1, 50).to_string();
    let url = format!(
        "{SEARCH_URL}?format=json&w={}&n={}&p=1&catZhida=1",
        urlencoding::encode(keywords),
        limit
    );
    let body: Value = client()?
        .get(url)
        .send()
        .await
        .map_err(|err| format!("QQ Music search unavailable: {err}"))?
        .json()
        .await
        .map_err(|err| format!("Invalid QQ Music search response: {err}"))?;
    body.pointer("/data/song/list")
        .and_then(Value::as_array)
        .ok_or_else(|| "QQ Music returned no song results.".to_string())
        .map(|songs| songs.iter().filter_map(map_song).collect())
}

fn map_song(value: &Value) -> Option<QqMusicSong> {
    let songmid = value.get("songmid").and_then(Value::as_str)?.to_string();
    let name = value
        .get("songname")
        .and_then(Value::as_str)?
        .trim()
        .to_string();
    if songmid.is_empty() || name.is_empty() {
        return None;
    }
    let artist = value
        .get("singer")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("name").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .unwrap_or_default();
    let album = value
        .get("albumname")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let album_mid = value
        .get("albummid")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let cover = (!album_mid.is_empty())
        .then(|| format!("{COVER_URL}{album_mid}.jpg"))
        .unwrap_or_default();
    Some(QqMusicSong {
        songmid,
        name,
        artist,
        album,
        cover,
        duration: value.get("interval").and_then(Value::as_u64).unwrap_or(0),
    })
}

pub async fn stream_url(songmid: &str) -> Result<String, String> {
    let songmid = songmid.trim();
    if songmid.is_empty() {
        return Err("Missing QQ Music song id.".to_string());
    }
    let filename = format!("M500{songmid}{songmid}.mp3");
    let data = serde_json::json!({
        "req": {
            "module": "CDN.SrfCdnDispatchServer",
            "method": "GetCdnDispatch",
            "param": {"guid": "658650575", "calltype": 0, "userip": ""}
        },
        "req_0": {
            "module": "vkey.GetVkeyServer",
            "method": "CgiGetVkey",
            "param": {
                "filename": [filename],
                "guid": "658650575",
                "songmid": [songmid],
                "songtype": [0],
                "uin": "0",
                "loginflag": 1,
                "platform": "20"
            }
        },
        "comm": {"uin": 0, "format": "json", "ct": 24, "cv": 0}
    });
    let url = format!(
        "{PLAY_URL}?format=json&data={}",
        urlencoding::encode(&data.to_string())
    );
    let body: Value = client()?
        .get(url)
        .send()
        .await
        .map_err(|err| format!("QQ Music stream unavailable: {err}"))?
        .json()
        .await
        .map_err(|err| format!("Invalid QQ Music stream response: {err}"))?;
    let data = body.pointer("/req_0/data").unwrap_or(&Value::Null);
    let path = data
        .get("midurlinfo")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("purl"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if path.is_empty() {
        return Err("QQ Music did not provide a playable stream. The song may require a login or may be unavailable.".to_string());
    }
    if path.starts_with("http://") || path.starts_with("https://") {
        Ok(path.to_string())
    } else {
        Ok(format!("{STREAM_HOST}{path}"))
    }
}
