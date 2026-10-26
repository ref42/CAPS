//! Portable queue metadata. Audio files themselves are not embedded.
use crate::track::Track;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Serialize, Deserialize)]
struct QueueFile {
    version: u32,
    tracks: Vec<Track>,
}

pub fn export(path: &Path, tracks: Vec<Track>) -> Result<(), String> {
    let bytes = encode(tracks)?;
    fs::write(path, bytes).map_err(|err| format!("Could not export queue: {err}"))
}

pub fn import(path: &Path) -> Result<Vec<Track>, String> {
    let bytes = fs::read(path).map_err(|err| format!("Could not read queue: {err}"))?;
    decode(&bytes)
}

fn encode(tracks: Vec<Track>) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(&QueueFile { version: 1, tracks })
        .map_err(|err| format!("Could not encode queue: {err}"))
}

fn decode(bytes: &[u8]) -> Result<Vec<Track>, String> {
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let file: QueueFile =
        serde_json::from_slice(bytes).map_err(|err| format!("Invalid queue JSON: {err}"))?;
    if file.version != 1 {
        return Err(format!("Unsupported queue version: {}", file.version));
    }
    if file
        .tracks
        .iter()
        .any(|track| track.id.trim().is_empty() || track.source.trim().is_empty())
    {
        return Err("Invalid queue: every track needs a source and an ID.".into());
    }
    Ok(file.tracks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{actions::append_unique_tracks, track::*};

    fn track(source: &str, id: &str) -> Track {
        Track {
            source: source.into(),
            id: id.into(),
            media_id: "media-id".into(),
            stream_url: "https://example.com/audio".into(),
            name: "歌曲 🎵".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            cover: "cover.jpg".into(),
            duration: Some(123),
        }
    }

    #[test]
    fn full_queue_round_trip_preserves_order_and_legacy_metadata() {
        let sources = [
            SOURCE_LOCAL,
            SOURCE_NETEASE,
            SOURCE_SHITEASE,
            SOURCE_QQMUSIC,
            SOURCE_KUGOU,
            // Older queue files stay readable after video import is removed.
            "bilibili",
            "youtube",
            "douyin",
            SOURCE_QISHUI,
        ];
        let tracks: Vec<_> = (0..600)
            .map(|i| track(sources[i % sources.len()], &i.to_string()))
            .collect();
        assert_eq!(decode(&encode(tracks.clone()).unwrap()).unwrap(), tracks);
        assert!(decode(&encode(vec![]).unwrap()).unwrap().is_empty());
    }

    #[test]
    fn import_accepts_bom_and_appends_only_new_tracks_in_order() {
        let first = track(SOURCE_NETEASE, "1");
        let second = track(SOURCE_QQMUSIC, "1");
        let third = track(SOURCE_LOCAL, r"C:\Music\歌曲.flac");
        let mut bytes = vec![0xef, 0xbb, 0xbf];
        bytes.extend(
            encode(vec![
                first.clone(),
                second.clone(),
                second.clone(),
                third.clone(),
            ])
            .unwrap(),
        );
        let mut queue = vec![first.clone()];
        assert_eq!(append_unique_tracks(&mut queue, decode(&bytes).unwrap()), 2);
        assert_eq!(queue, vec![first, second, third]);
    }

    #[test]
    fn rejects_malformed_incompatible_and_incomplete_files() {
        for bytes in [
            b"not json".as_slice(),
            br#"{}"#,
            br#"{"version":2,"tracks":[]}"#,
            br#"{"version":1,"tracks":[{}]}"#,
        ] {
            assert!(decode(bytes).is_err());
        }
        assert!(decode(&encode(vec![track(SOURCE_LOCAL, " ")]).unwrap()).is_err());
    }
}
