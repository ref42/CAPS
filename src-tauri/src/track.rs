use crate::netease;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SOURCE_NETEASE: &str = "netease";
pub const SOURCE_LOCAL: &str = "local";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    #[serde(default = "default_source")]
    pub source: String,
    pub id: String,
    pub name: String,
    pub artist: String,
    pub album: String,
    pub cover: String,
}

impl From<netease::NeteaseSong> for Track {
    fn from(song: netease::NeteaseSong) -> Self {
        Self {
            source: song
                .source
                .or(song.provider)
                .unwrap_or_else(|| SOURCE_NETEASE.to_string()),
            id: value_id(&song.id),
            name: song.name,
            artist: clean_or(song.artist, "Unknown artist"),
            album: clean_or(song.album, "Unknown album"),
            cover: song.cover.unwrap_or_default(),
        }
    }
}

fn default_source() -> String {
    SOURCE_NETEASE.to_string()
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
