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
    pub island_size: u32,
    pub random_count: u32,
    pub active_tab: String,
    pub local_music_folder: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
            island_size: 100,
            random_count: 50,
            active_tab: "search".to_string(),
            local_music_folder: String::new(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            settings: AppSettings::default(),
            queue: Vec::new(),
            current_index: None,
        }
    }
}

impl AppSettings {
    fn normalized(mut self) -> Self {
        self.opacity = self.opacity.clamp(10, 100);
        self.volume = self.volume.clamp(0, 100);
        self.island_size = self.island_size.clamp(85, 150);
        self.random_count = self.random_count.clamp(1, 100);
        if !matches!(
            self.active_tab.as_str(),
            "search" | "queue" | "stats" | "settings"
        ) {
            self.active_tab = "search".to_string();
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
    let Ok(text) = fs::read_to_string(path) else {
        return AppState::default();
    };
    serde_json::from_str::<AppState>(&text)
        .map(AppState::normalized)
        .unwrap_or_default()
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
        persisted_queue.push(track.clone());
    }

    AppState {
        settings: settings.normalized(),
        queue: persisted_queue,
        current_index: persisted_index,
    }
}

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
