use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};
use serde_json::Value;
use std::collections::BTreeMap;

const SEARCH_URL: &str = "https://songsearch.kugou.com/song_search_v2";
const PLAY_URL: &str = "https://gateway.kugou.com/v5/url";
const UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/124.0 Safari/537.36";

#[derive(Clone, Debug)]
pub struct KugouSong {
    pub hash: String,
    pub quality_hash: String,
    pub album_id: String,
    pub album_audio_id: String,
    pub name: String,
    pub artist: String,
    pub album: String,
    pub cover: String,
    pub duration: u64,
}

fn client() -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(UA));
    headers.insert(REFERER, HeaderValue::from_static("https://www.kugou.com/"));
    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|err| format!("KuGou client unavailable: {err}"))
}

pub async fn search(keywords: String, limit: u32) -> Result<Vec<KugouSong>, String> {
    let keywords = keywords.trim();
    if keywords.is_empty() {
        return Ok(Vec::new());
    }
    let url = format!(
        "{SEARCH_URL}?keyword={}&page=1&pagesize={}",
        urlencoding::encode(keywords),
        limit.clamp(1, 30)
    );
    let body: Value = client()?
        .get(url)
        .send()
        .await
        .map_err(|err| format!("KuGou search unavailable: {err}"))?
        .json()
        .await
        .map_err(|err| format!("Invalid KuGou search response: {err}"))?;
    Ok(body
        .pointer("/data/lists")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(map_song)
                .collect::<Vec<KugouSong>>()
        })
        .unwrap_or_default())
}

fn map_song(value: &Value) -> Option<KugouSong> {
    let hash = value
        .get("FileHash")
        .and_then(Value::as_str)?
        .trim()
        .to_string();
    let name = value
        .get("SongName")
        .and_then(Value::as_str)
        .or_else(|| value.get("OriSongName").and_then(Value::as_str))?
        .trim()
        .to_string();
    if hash.is_empty() || name.is_empty() {
        return None;
    }
    let cover = value
        .get("Image")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .replace("{size}", "400");
    Some(KugouSong {
        hash,
        quality_hash: value
            .get("HQFileHash")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string(),
        album_id: value
            .get("AlbumID")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        album_audio_id: value
            .get("ID")
            .or_else(|| value.get("MixSongID"))
            .map(|value| value.to_string().trim_matches('"').to_string())
            .unwrap_or_default(),
        name,
        artist: value
            .get("SingerName")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string(),
        album: value
            .get("AlbumName")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string(),
        cover,
        duration: value.get("Duration").and_then(Value::as_u64).unwrap_or(0),
    })
}

pub async fn stream_url(
    hash: &str,
    quality_hash: &str,
    album_id: &str,
    album_audio_id: &str,
) -> Result<String, String> {
    let hash = hash.trim();
    if hash.is_empty() {
        return Err("Missing KuGou song hash.".to_string());
    }
    let hash = if quality_hash.trim().is_empty() {
        hash
    } else {
        quality_hash.trim()
    };
    let client = client()?;
    let mut params: BTreeMap<String, String> = BTreeMap::from([
        ("IsFreePart", "0".to_string()),
        ("appid", "1005".to_string()),
        ("album_audio_id", album_audio_id.to_string()),
        ("album_id", album_id.to_string()),
        ("area_code", "1".to_string()),
        ("behavior", "play".to_string()),
        ("cdnBackup", "1".to_string()),
        ("clienttime", chrono_like_timestamp()),
        ("clientver", "20489".to_string()),
        ("cmd", "26".to_string()),
        ("dfid", "-".to_string()),
        ("hash", hash.to_lowercase()),
        (
            "key",
            md5_hex(&format!(
                "{}57ae12eb6890223e355ccfcb74edf70d{}{}0",
                hash.to_lowercase(),
                "1005",
                device_mid()
            )),
        ),
        ("mid", device_mid()),
        ("module", String::new()),
        ("page_id", "151369488".to_string()),
        ("pid", "2".to_string()),
        ("pidversion", "3001".to_string()),
        ("ppage_id", "463467626,350369493,788954147".to_string()),
        ("quality", "128".to_string()),
        ("signature", String::new()),
        ("ssa_flag", "is_fromtrack".to_string()),
        ("uuid", "-".to_string()),
        ("version", "11430".to_string()),
    ])
    .into_iter()
    .map(|(key, value)| (key.to_string(), value))
    .collect();
    let signature_input = params
        .iter()
        .filter(|(key, _)| key != &"signature")
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<String>();
    params.insert(
        "signature".to_string(),
        md5_hex(&format!(
            "OIlwieks28dk2k092lksi2UIkp{signature_input}OIlwieks28dk2k092lksi2UIkp"
        )),
    );
    let query = params
        .iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                urlencoding::encode(key),
                urlencoding::encode(value)
            )
        })
        .collect::<Vec<_>>()
        .join("&");
    let body: Value = client
        .get(format!("{PLAY_URL}?{query}"))
        .header("x-router", "trackercdn.kugou.com")
        .header("dfid", "-")
        .header(
            "mid",
            params.get("mid").map(String::as_str).unwrap_or_default(),
        )
        .header(
            "clienttime",
            params
                .get("clienttime")
                .map(String::as_str)
                .unwrap_or_default(),
        )
        .header("kg-rc", "1")
        .header("kg-thash", "5d816a0")
        .header("kg-rec", "1")
        .header("kg-rf", "B9EDA08A64250DEFFBCADDEE00F8F25F")
        .send()
        .await
        .map_err(|err| format!("KuGou stream unavailable: {err}"))?
        .json()
        .await
        .map_err(|err| format!("Invalid KuGou stream response: {err}"))?;
    for pointer in ["/url/0", "/backupUrl/0", "/data/url", "/data/play_url"] {
        if let Some(url) = body
            .pointer(pointer)
            .and_then(Value::as_str)
            .filter(|url| !url.is_empty())
        {
            return Ok(url.replacen("http://", "https://", 1));
        }
    }
    Err("KuGou has no playable stream for this track.".to_string())
}

pub async fn lyric(
    hash: &str,
    title: &str,
    artist: &str,
    duration: Option<u64>,
) -> Result<String, String> {
    let duration_ms = duration.unwrap_or_default().saturating_mul(1000);
    let keyword = format!("{artist} - {title}");
    let search_url = format!(
        "https://lyrics.kugou.com/search?ver=1&man=yes&client=pc&keyword={}&duration={duration_ms}&hash={}&album_audio_id=0",
        urlencoding::encode(&keyword),
        urlencoding::encode(hash.trim())
    );
    let body: Value = client()?
        .get(search_url)
        .send()
        .await
        .map_err(|err| format!("KuGou lyric search unavailable: {err}"))?
        .json()
        .await
        .map_err(|err| format!("Invalid KuGou lyric response: {err}"))?;
    let candidates = body
        .get("candidates")
        .and_then(Value::as_array)
        .ok_or_else(|| "KuGou returned no lyric candidates.".to_string())?;
    let selected = candidates
        .iter()
        .filter_map(|candidate| {
            let id = candidate.get("id").and_then(Value::as_str)?;
            let accesskey = candidate.get("accesskey").and_then(Value::as_str)?;
            let candidate_duration = candidate
                .get("duration")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let distance = candidate_duration.abs_diff(duration_ms);
            Some((distance, id, accesskey))
        })
        .min_by_key(|(distance, _, _)| *distance)
        .ok_or_else(|| "KuGou returned no usable lyric candidate.".to_string())?;
    let download_url = format!(
        "https://lyrics.kugou.com/download?ver=1&client=pc&id={}&accesskey={}&fmt=lrc&charset=utf8",
        urlencoding::encode(selected.1),
        urlencoding::encode(selected.2)
    );
    let body: Value = client()?
        .get(download_url)
        .send()
        .await
        .map_err(|err| format!("KuGou lyric download unavailable: {err}"))?
        .json()
        .await
        .map_err(|err| format!("Invalid KuGou lyric download response: {err}"))?;
    let encoded = body
        .get("content")
        .and_then(Value::as_str)
        .filter(|content| !content.is_empty())
        .ok_or_else(|| "KuGou returned empty lyrics.".to_string())?;
    let decoded = STANDARD
        .decode(encoded)
        .map_err(|err| format!("KuGou lyric decode failed: {err}"))?;
    String::from_utf8(decoded).map_err(|err| format!("KuGou lyric text is invalid UTF-8: {err}"))
}

fn chrono_like_timestamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn device_mid() -> String {
    hex_to_decimal(&md5_hex("CAPS-KuGou-device"))
}

fn md5_hex(value: &str) -> String {
    use md5::{Digest, Md5};
    let mut hasher = Md5::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn hex_to_decimal(value: &str) -> String {
    let mut digits = vec![0_u8];
    for nibble in value
        .bytes()
        .filter_map(|byte| char::from(byte).to_digit(16).map(|value| value as u8))
    {
        let mut carry = nibble as u16;
        for digit in digits.iter_mut().rev() {
            let next = (*digit as u16) * 16 + carry;
            *digit = (next % 10) as u8;
            carry = next / 10;
        }
        while carry > 0 {
            digits.insert(0, (carry % 10) as u8);
            carry /= 10;
        }
    }
    digits
        .into_iter()
        .map(|digit| char::from(b'0' + digit))
        .collect()
}
