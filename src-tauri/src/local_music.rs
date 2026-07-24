use crate::track::{SOURCE_LOCAL, Track};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "ogg", "m4a", "aac"];
const MAX_SCAN_ITEMS: usize = 8_000;

pub fn load_all(folder: &str) -> Result<Vec<Track>, String> {
    if folder.trim().is_empty() {
        return Ok(Vec::new());
    }
    scan(folder, MAX_SCAN_ITEMS)
}

pub fn read_audio(path: &str) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|err| format!("Local file read failed: {err}"))
}

pub fn read_lyrics(path: &str) -> String {
    let audio_path = Path::new(path);
    let Some(stem) = audio_path.file_stem().and_then(|value| value.to_str()) else {
        return String::new();
    };
    let lrc_path = audio_path.with_file_name(format!("{stem}.lrc"));
    fs::read_to_string(lrc_path).unwrap_or_default()
}

fn scan(folder: &str, limit: usize) -> Result<Vec<Track>, String> {
    let root = PathBuf::from(folder.trim());
    if !root.is_dir() {
        return Err("Local music folder is not valid.".to_string());
    }
    let mut tracks = Vec::new();
    let mut seen = HashSet::new();
    let mut stack = vec![root];
    while let Some(path) = stack.pop() {
        let Ok(entries) = fs::read_dir(path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !is_audio_file(&path) {
                continue;
            }
            let id = path.to_string_lossy().to_string();
            if !seen.insert(id.clone()) {
                continue;
            }
            tracks.push(track_from_path(&path, id));
            if tracks.len() >= limit {
                return Ok(tracks);
            }
        }
    }
    Ok(tracks)
}

fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|ext| {
            AUDIO_EXTENSIONS
                .iter()
                .any(|candidate| ext.eq_ignore_ascii_case(candidate))
        })
        .unwrap_or(false)
}

fn track_from_path(path: &Path, id: String) -> Track {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Local track")
        .trim();
    let (artist, name) = stem
        .split_once(" - ")
        .map(|(artist, name)| (artist.trim(), name.trim()))
        .unwrap_or(("Local file", stem));
    let album = path
        .parent()
        .and_then(|value| value.file_name())
        .and_then(|value| value.to_str())
        .unwrap_or("Local library")
        .to_string();
    Track {
        source: SOURCE_LOCAL.to_string(),
        id,
        name: name.to_string(),
        artist: artist.to_string(),
        album,
        cover: String::new(),
    }
}
