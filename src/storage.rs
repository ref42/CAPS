use crate::track::{SOURCE_LOCAL, Track};
use serde::{Deserialize, Serialize};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

const MAX_PERSISTED_QUEUE: usize = 500;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub opacity: u32,
    pub volume: u32,
    /// Scale of the capsule and its panel, in percent. It was called
    /// `island_size` before the app settled on "capsule"; the alias keeps state
    /// files written by those versions readable.
    #[serde(alias = "island_size")]
    pub capsule_size: u32,
    pub random_count: u32,
    pub active_tab: String,
    pub local_music_folder: String,
    pub language: String,
    /// Top-left of the window, in physical pixels, after the user dragged the
    /// capsule. `None` means "top centre of the primary display".
    pub window_position: Option<(f32, f32)>,
    /// Latin/UI family override. `None` keeps the platform's own UI face.
    pub latin_font: Option<String>,
    /// Han family override. `None` keeps the platform's Han companion for the
    /// UI face, which is what makes mixed lines render as one typeface.
    pub han_font: Option<String>,
    /// What plays after the current track ends; see `mode::PlayOrder`.
    pub play_order: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppState {
    pub settings: AppSettings,
    pub queue: Vec<Track>,
    pub current_index: Option<usize>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            opacity: 92,
            volume: 100,
            capsule_size: 100,
            random_count: 50,
            active_tab: "search".to_string(),
            local_music_folder: String::new(),
            language: "en".to_string(),
            window_position: None,
            latin_font: None,
            han_font: None,
            play_order: "all".to_string(),
        }
    }
}

/// Cleans a family name coming from the state file. An empty or blank name
/// means "no override", so a hand-edited file cannot blank out the typeface.
fn normalized_family(family: Option<String>) -> Option<String> {
    family
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

impl AppSettings {
    fn normalized(mut self) -> Self {
        // The surface may be faded to nothing: the palette keeps the text white,
        // so the capsule stays readable over whatever is behind it. Only the
        // ends of the range are enforced.
        self.opacity = self.opacity.min(100);
        self.volume = self.volume.clamp(0, 100);
        self.capsule_size = self.capsule_size.clamp(85, 150);
        self.random_count = self.random_count.clamp(1, 999);
        if !matches!(self.active_tab.as_str(), "search" | "queue" | "settings") {
            self.active_tab = "search".to_string();
        }
        if !matches!(self.language.as_str(), "en" | "zh") {
            self.language = "en".to_string();
        }
        self.latin_font = normalized_family(self.latin_font);
        self.han_font = normalized_family(self.han_font);
        if !matches!(
            self.play_order.as_str(),
            "sequential" | "all" | "one" | "shuffle"
        ) {
            self.play_order = "all".to_string();
        }
        self
    }
}

impl AppState {
    pub fn normalized(mut self) -> Self {
        self.settings = self.settings.normalized();
        if self
            .current_index
            .is_some_and(|index| index >= self.queue.len())
        {
            self.current_index = None;
        }
        self
    }
}

pub fn load_state() -> AppState {
    let Some(path) = state_path() else {
        return AppState::default();
    };
    let Ok(bytes) = fs::read(&path) else {
        return AppState::default();
    };
    let text = String::from_utf8_lossy(strip_bom(&bytes));
    match serde_json::from_str::<AppState>(&text) {
        Ok(state) => state.normalized(),
        Err(err) => {
            log::error!("Could not read {}: {err}", path.display());
            // Keep the unreadable file. Hand-edited state (an editor that adds a
            // byte order mark, a half-written file) must not be silently
            // replaced by defaults, because the next save would erase the queue.
            let _ = fs::write(path.with_extension("invalid.json"), &bytes);
            AppState::default()
        }
    }
}

/// Editors on Windows like to prefix UTF-8 with a byte order mark, which
/// `serde_json` rejects outright. Stripping it here keeps a hand-edited
/// `state.json` from looking like a corrupt one.
fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)
}

pub fn save_state_parts(settings: AppSettings, queue: &[Track], current_index: Option<usize>) {
    let Some(path) = state_path() else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let state = persisted_state(settings, queue, current_index);
    if let Ok(text) = serde_json::to_string_pretty(&state) {
        let _ = fs::write(path, text);
    }
}

fn persisted_state(
    settings: AppSettings,
    queue: &[Track],
    current_index: Option<usize>,
) -> AppState {
    let mut persisted_queue = Vec::with_capacity(queue.len().min(MAX_PERSISTED_QUEUE));
    let mut persisted_index = None;

    for (source_index, track) in queue.iter().enumerate() {
        if track.source == SOURCE_LOCAL {
            continue;
        }
        if persisted_queue.len() >= MAX_PERSISTED_QUEUE {
            break;
        }
        if current_index == Some(source_index) {
            persisted_index = Some(persisted_queue.len());
        }
        persisted_queue.push(track.to_owned());
    }

    AppState {
        settings: settings.normalized(),
        queue: persisted_queue,
        current_index: persisted_index,
    }
}

#[allow(dead_code)]
pub fn clean_song_cache() -> Result<(), String> {
    let Some(path) = song_cache_path() else {
        return Err("Song cache path is not available.".to_string());
    };
    if !path.exists() {
        return Ok(());
    }
    fs::remove_dir_all(path).map_err(|err| format!("Song cache cleanup failed: {err}"))
}

pub fn song_cache_file(source: &str, id: &str) -> Option<PathBuf> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hasher);
    id.hash(&mut hasher);
    Some(song_cache_path()?.join(format!("{:016x}.audio", hasher.finish())))
}

pub fn cover_cache_path() -> Option<PathBuf> {
    Some(app_dir()?.join("cover-cache"))
}

fn app_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("LOCALAPPDATA"))
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())?;
    Some(base.join("CAPS"))
}

fn state_path() -> Option<PathBuf> {
    Some(app_dir()?.join("state.json"))
}

fn song_cache_path() -> Option<PathBuf> {
    Some(app_dir()?.join("song-cache"))
}
