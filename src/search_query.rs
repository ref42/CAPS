//! A trailing provider is a ranking preference, never a source filter.
use crate::track::{
    SOURCE_KUGOU, SOURCE_NETEASE, SOURCE_QISHUI, SOURCE_QQMUSIC, SOURCE_SHITEASE, Track,
};

#[derive(Debug, PartialEq)]
pub struct SearchQuery {
    pub text: String,
    pub preferred: Option<&'static str>,
}

impl SearchQuery {
    pub fn parse(input: &str) -> Result<Self, String> {
        let input = input.trim();
        if input.is_empty() {
            return Err("Type a song name first.".into());
        }
        let token = input.split_whitespace().last().unwrap_or_default();
        let lower = token.to_lowercase();
        let (name, explicit) = if let Some(value) = lower
            .strip_prefix("platform:")
            .or_else(|| lower.strip_prefix("平台:"))
            .or_else(|| lower.strip_prefix('@'))
        {
            (value, true)
        } else {
            (lower.as_str(), false)
        };
        let preferred = match name {
            "netease" | "neteast" | "shitease" | "网易" | "网易云" | "网易云音乐" => {
                Some(SOURCE_NETEASE)
            }
            "qq" | "qqmusic" | "qq音乐" => Some(SOURCE_QQMUSIC),
            "kugou" | "酷狗" | "酷狗音乐" => Some(SOURCE_KUGOU),
            "qishui" | "汽水" | "汽水音乐" => Some(SOURCE_QISHUI),
            _ => None,
        };
        if explicit && preferred.is_none() {
            return Err("Unknown platform. Use netease, kugou, qq, or qishui.".into());
        }
        let rest = input[..input.len() - token.len()].trim_end();
        if explicit && rest.is_empty() {
            return Err("Type a song name before the platform.".into());
        }
        // A single word may itself be a song/artist. Quoted trailing names are
        // literal too; never strip a substring from a title or artist.
        if preferred.is_some() && !rest.is_empty() {
            Ok(Self {
                text: rest.into(),
                preferred,
            })
        } else {
            Ok(Self {
                text: input.into(),
                preferred: None,
            })
        }
    }

    pub fn prefers(&self, track: &Track) -> bool {
        self.preferred.is_some_and(|preferred| {
            track.source == preferred
                || (preferred == SOURCE_NETEASE && track.source == SOURCE_SHITEASE)
        })
    }

    pub fn rank(&self, tracks: &mut [Track]) {
        if self.preferred.is_some() {
            // Stable sorting preserves each provider's relevance ordering and
            // keeps all other sources in their original relative order.
            tracks.sort_by_key(|track| !self.prefers(track));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strips_only_a_complete_trailing_provider() {
        for (alias, expected) in [
            ("neteast", SOURCE_NETEASE),
            ("NETEASE", SOURCE_NETEASE),
            ("网易云", SOURCE_NETEASE),
            ("酷狗", SOURCE_KUGOU),
            ("kugou", SOURCE_KUGOU),
            ("QQ", SOURCE_QQMUSIC),
            ("qqmusic", SOURCE_QQMUSIC),
            ("汽水音乐", SOURCE_QISHUI),
            ("platform:qishui", SOURCE_QISHUI),
        ] {
            let query = SearchQuery::parse(&format!("  晴天 周杰伦  {alias}  ")).unwrap();
            assert_eq!(query.text, "晴天 周杰伦");
            assert_eq!(query.preferred, Some(expected));
        }
        for text in [
            "qq",
            "The Sound of Silence Simon & Garfunkel",
            "a song QQLover",
            "a song \"qq\"",
        ] {
            assert_eq!(
                SearchQuery::parse(text).unwrap(),
                SearchQuery {
                    text: text.into(),
                    preferred: None
                }
            );
        }
        assert_eq!(SearchQuery::parse("晴天 qq").unwrap().text, "晴天");
        assert!(SearchQuery::parse("platform:qq").is_err());
        assert!(SearchQuery::parse("晴天 platform:unknown").is_err());
    }
    #[test]
    fn preference_keeps_all_results_and_original_relevance() {
        let track = |source: &str, id: &str| Track {
            source: source.into(),
            id: id.into(),
            media_id: String::new(),
            stream_url: String::new(),
            name: "song".into(),
            artist: "artist".into(),
            album: String::new(),
            cover: String::new(),
            duration: None,
        };
        let mut tracks = vec![
            track(SOURCE_NETEASE, "n1"),
            track(SOURCE_QQMUSIC, "q1"),
            track(SOURCE_KUGOU, "k1"),
            track(SOURCE_QQMUSIC, "q2"),
            track(SOURCE_QISHUI, "s1"),
        ];
        SearchQuery::parse("song artist qq")
            .unwrap()
            .rank(&mut tracks);
        assert_eq!(
            tracks
                .iter()
                .map(|track| track.id.as_str())
                .collect::<Vec<_>>(),
            ["q1", "q2", "n1", "k1", "s1"]
        );
        let before = tracks.clone();
        SearchQuery::parse("song artist").unwrap().rank(&mut tracks);
        assert_eq!(tracks, before);
        let mut legacy = vec![track(SOURCE_QQMUSIC, "q"), track(SOURCE_SHITEASE, "n")];
        SearchQuery::parse("song netease")
            .unwrap()
            .rank(&mut legacy);
        assert_eq!(legacy[0].id, "n");
    }
}
