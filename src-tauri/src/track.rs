use crate::shitease;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SOURCE_SHITEASE: &str = "shitease";
pub const SOURCE_LOCAL: &str = "local";
pub const SOURCE_BILIBILI: &str = "bilibili";
pub const SOURCE_YOUTUBE: &str = "youtube";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    #[serde(default = "default_source")]
    pub source: String,
    pub id: String,
    pub name: String,
    pub artist: String,
    pub album: String,
    pub cover: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<u64>,
}

impl From<shitease::ShiteaseSong> for Track {
    fn from(song: shitease::ShiteaseSong) -> Self {
        Self {
            source: song
                .source
                .or(song.provider)
                .unwrap_or_else(|| SOURCE_SHITEASE.to_string()),
            id: value_id(&song.id),
            name: song.name,
            artist: clean_or(song.artist, "Unknown artist"),
            album: clean_or(song.album, "Unknown album"),
            cover: song.cover.unwrap_or_default(),
            duration: song.duration.and_then(normalized_duration_seconds),
        }
    }
}

fn default_source() -> String {
    SOURCE_SHITEASE.to_string()
}

fn clean_or(value: Option<String>, fallback: &str) -> String {
    value
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn value_id(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        _ => String::new(),
    }
}

fn normalized_duration_seconds(value: u64) -> Option<u64> {
    if value == 0 {
        None
    } else if value > 10_000 {
        Some((value + 999) / 1000)
    } else {
        Some(value)
    }
}
