//! Detect media from the URL host, not words or embedded IDs in share captions.
use reqwest::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Bilibili,
    Youtube,
    Douyin,
}

impl Provider {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bilibili => "Bilibili",
            Self::Youtube => "YouTube",
            Self::Douyin => "Douyin",
        }
    }
}

fn provider(url: &Url) -> Option<Provider> {
    if !matches!(url.scheme(), "https" | "http")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return None;
    }
    match url.host_str()? {
        "bilibili.com" | "www.bilibili.com" | "m.bilibili.com" | "b23.tv" => {
            Some(Provider::Bilibili)
        }
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com" | "youtu.be" => {
            Some(Provider::Youtube)
        }
        "douyin.com" | "www.douyin.com" | "m.douyin.com" | "v.douyin.com" | "iesdouyin.com"
        | "www.iesdouyin.com" => Some(Provider::Douyin),
        _ => None,
    }
}

pub fn detect(text: &str) -> Result<(Provider, String), String> {
    let mut candidates = Vec::new();
    // Share messages commonly have a caption and Chinese punctuation around the URL.
    for word in text.split(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '，' | '。' | '；' | '！' | '【' | '】' | '「' | '」' | '<' | '>' | '"'
            )
    }) {
        let word = word.trim_matches(['(', ')', '[', ']', ',', '.', ';', '!']);
        if let Ok(mut url) = Url::parse(word)
            && let Some(source) = provider(&url)
        {
            // Share links from older clients may use HTTP; providers use HTTPS.
            let _ = url.set_scheme("https");
            let pair = (source, url.to_string());
            if !candidates.contains(&pair) {
                candidates.push(pair);
            }
        }
    }
    match candidates.len() {
        1 => Ok(candidates.remove(0)),
        0 => Err("Paste a Bilibili, YouTube, or Douyin link or share message.".into()),
        _ => Err("More than one media link found. Import one link at a time.".into()),
    }
}

pub async fn resolve_share(source: Provider, text: String) -> Result<String, String> {
    let url = Url::parse(&text).map_err(|e| e.to_string())?;
    if url.host_str() != Some("b23.tv") {
        return Ok(text);
    }
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() < 8 && provider(attempt.url()) == Some(source) {
                attempt.follow()
            } else {
                attempt.error("Unexpected media share redirect")
            }
        }))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    Ok(response.url().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_share_hosts_and_preserves_page_selection() {
        for (text, expected) in [
            (
                "【歌曲】https://www.bilibili.com/video/BV1xx411c7mD?p=2 分享",
                Provider::Bilibili,
            ),
            ("https://b23.tv/example", Provider::Bilibili),
            (
                "Listen https://youtu.be/iKbylgzysHw?t=12!",
                Provider::Youtube,
            ),
            (
                "https://music.youtube.com/watch?v=iKbylgzysHw",
                Provider::Youtube,
            ),
            (
                "1.28 fOX:/ https://v.douyin.com/gYsL-kehIgg/ 复制此链接",
                Provider::Douyin,
            ),
            ("http://v.douyin.com/gYsL-kehIgg/", Provider::Douyin),
        ] {
            assert_eq!(detect(text).unwrap().0, expected);
        }
        assert!(
            detect("https://www.bilibili.com/video/BV1xx411c7mD?p=2")
                .unwrap()
                .1
                .ends_with("?p=2")
        );
        assert!(
            detect("http://v.douyin.com/example/")
                .unwrap()
                .1
                .starts_with("https://")
        );
    }
    #[test]
    fn rejects_lookalikes_nested_urls_and_ambiguous_messages() {
        for text in [
            "https://youtube.com.evil.test/watch?v=iKbylgzysHw",
            "https://evil.test/?v=iKbylgzysHw",
            "https://evil.test/?url=https://youtu.be/iKbylgzysHw",
            "https://user@v.douyin.com/foo",
            "https://youtu.be/a https://b23.tv/b",
            "not a link",
        ] {
            assert!(detect(text).is_err(), "{text}");
        }
    }
}
