use crate::track::Track;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub opacity: u32,
    pub volume: u32,
    pub island_size: u32,
    pub random_count: u32,
    pub spring_style: String,
    pub active_tab: String,
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
            spring_style: "smooth".to_string(),
            active_tab: "search".to_string(),
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

impl AppState {
    pub fn normalized(mut self) -> Self {
        self.settings.opacity = self.settings.opacity.clamp(10, 100);
        self.settings.volume = self.settings.volume.clamp(0, 100);
        self.settings.island_size = self.settings.island_size.clamp(85, 135);
        self.settings.random_count = self.settings.random_count.clamp(1, 100);
        if self.settings.spring_style != "bouncy" {
            self.settings.spring_style = "smooth".to_string();
        }
        if !matches!(
            self.settings.active_tab.as_str(),
            "search" | "queue" | "stats" | "settings"
        ) {
            self.settings.active_tab = "search".to_string();
        }
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

pub fn save_state(state: &AppState) {
    let Some(path) = state_path() else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    if let Ok(text) = serde_json::to_string_pretty(&state.clone().normalized()) {
        let _ = fs::write(path, text);
    }
}

pub fn clean_song_cache() {
    let Some(path) = song_cache_path() else {
        return;
    };
    let _ = fs::remove_dir_all(path);
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
