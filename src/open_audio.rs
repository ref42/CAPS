use crate::track::{SOURCE_ARCHIVE, SOURCE_AUDIUS, SOURCE_WIKIMEDIA, Track};
use serde_json::Value;

const AUDIUS_API: &str = "https://api.audius.co/v1";
const ARCHIVE_API: &str = "https://archive.org/advancedsearch.php";
const WIKIMEDIA_API: &str = "https://commons.wikimedia.org/w/api.php";

pub async fn search(query: String, limit: u32) -> Result<Vec<Track>, String> {
    let (audius, archive, wikimedia) = tokio::join!(
        search_audius(&query, limit),
        search_archive(&query, limit),
        search_wikimedia(&query, limit),
    );
    let mut tracks = Vec::new();
    if let Ok(items) = audius {
        tracks.extend(items);
    }
    if let Ok(items) = archive {
        tracks.extend(items);
    }
    if let Ok(items) = wikimedia {
        tracks.extend(items);
    }
    if tracks.is_empty() {
        return Err("No playable open-audio results found.".to_string());
    }
    Ok(tracks)
}

async fn search_wikimedia(query: &str, limit: u32) -> Result<Vec<Track>, String> {
    let url = format!(
        "{WIKIMEDIA_API}?action=query&format=json&generator=search&gsrsearch={}&gsrnamespace=6&gsrlimit={}&prop=imageinfo&iiprop=url|mime",
        urlencoding::encode(&format!("{query} filetype:audio")),
        limit.min(20)
    );
    let body: Value = reqwest::Client::new()
        .get(url)
        .header("User-Agent", "CAPS/1.0 (open audio search)")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(body
        .pointer("/query/pages")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|pages| pages.values())
        .filter_map(|item| {
            let id = item.get("pageid")?.as_i64()?.to_string();
            let title = item
                .get("title")?
                .as_str()?
                .trim_start_matches("File:")
                .to_string();
            let info = item.pointer("/imageinfo/0")?;
            let stream_url = info.get("url")?.as_str()?.to_string();
            if !info
                .get("mime")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .starts_with("audio/")
            {
                return None;
            }
            Some(Track {
                source: SOURCE_WIKIMEDIA.to_string(),
                id,
                media_id: String::new(),
                stream_url,
                name: title,
                artist: "Wikimedia Commons".to_string(),
                album: "Wikimedia Commons".to_string(),
                cover: String::new(),
                duration: None,
            })
        })
        .collect())
}

async fn search_audius(query: &str, limit: u32) -> Result<Vec<Track>, String> {
    let url = format!(
        "{AUDIUS_API}/tracks/search?query={}&limit={}",
        urlencoding::encode(query),
        limit.min(20)
    );
    let body: Value = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(body
        .pointer("/data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?.to_string();
            let title = item.get("title")?.as_str()?.to_string();
            let artist = item
                .pointer("/user/name")
                .and_then(Value::as_str)
                .unwrap_or("Audius");
            let stream_url = format!("{AUDIUS_API}/tracks/{id}/stream");
            Some(Track {
                source: SOURCE_AUDIUS.to_string(),
                id,
                media_id: String::new(),
                stream_url,
                name: title,
                artist: artist.to_string(),
                album: "Audius".to_string(),
                cover: item
                    .get("artwork")
                    .and_then(|v| v.get("150x150"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                duration: item.get("duration").and_then(Value::as_u64),
            })
        })
        .collect())
}

async fn search_archive(query: &str, limit: u32) -> Result<Vec<Track>, String> {
    let q = format!("title:({query}) AND mediatype:audio");
    let url = format!(
        "{ARCHIVE_API}?q={}&fl[]=identifier,title,creator,album,downloads&rows={}&page=1&output=json",
        urlencoding::encode(&q),
        limit.min(20)
    );
    let body: Value = reqwest::get(url)
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let docs = body
        .pointer("/response/docs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let client = reqwest::Client::new();
    let mut tracks = Vec::new();
    for item in docs {
        let Some(id) = item.get("identifier").and_then(Value::as_str) else {
            continue;
        };
        let metadata: Value = match client
            .get(format!("https://archive.org/metadata/{id}"))
            .send()
            .await
            .and_then(|response| response.error_for_status())
        {
            Ok(response) => match response.json().await {
                Ok(value) => value,
                Err(_) => continue,
            },
            Err(_) => continue,
        };
        let Some(file) = metadata
            .pointer("/files")
            .and_then(Value::as_array)
            .and_then(|files| {
                files.iter().find(|file| {
                    let name = file.get("name").and_then(Value::as_str).unwrap_or_default();
                    let format = file
                        .get("format")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    (format.starts_with("VBR MP3") || format == "MP3" || format == "Ogg Vorbis")
                        && !name.contains("_files.xml")
                })
            })
        else {
            continue;
        };
        let Some(file_name) = file.get("name").and_then(Value::as_str) else {
            continue;
        };
        let title = item.get("title").and_then(Value::as_str).unwrap_or(id);
        tracks.push(Track {
            source: SOURCE_ARCHIVE.to_string(),
            id: format!("{id}/{file_name}"),
            media_id: String::new(),
            stream_url: format!(
                "https://archive.org/download/{id}/{}",
                urlencoding::encode(file_name)
            ),
            name: title.to_string(),
            artist: item
                .get("creator")
                .and_then(Value::as_str)
                .unwrap_or("Internet Archive")
                .to_string(),
            album: "Internet Archive".to_string(),
            cover: String::new(),
            duration: None,
        });
    }
    Ok(tracks)
}
