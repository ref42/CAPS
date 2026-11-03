use serde_json::Value;

const LRCLIB_URL: &str = "https://lrclib.net/api/get";

pub async fn find(title: &str, artist: &str, duration: Option<u64>) -> Option<String> {
    let mut url = format!(
        "{LRCLIB_URL}?track_name={}&artist_name={}",
        urlencoding::encode(title),
        urlencoding::encode(artist)
    );
    if let Some(duration) = duration {
        url.push_str(&format!("&duration={duration}"));
    }
    let body: Value = reqwest::Client::new()
        .get(url)
        .header("User-Agent", "CAPS/1.0 (lyrics lookup)")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;
    body.get("syncedLyrics")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .or_else(|| {
            body.get("plainLyrics")
                .and_then(Value::as_str)
                .filter(|text| !text.trim().is_empty())
        })
        .map(str::to_string)
}
