use std::{path::PathBuf, sync::Mutex};
use tauri::State;

use crate::{
    audio::{AudioCommand, AudioPlayer, AudioState},
    lyrics::{self, LyricLine},
    queue::{self, QueueEntry, QueueItem},
};

pub struct LocalPlayerState {
    pub player: AudioPlayer,
    pub queue: Mutex<Vec<QueueItem>>,
    pub queue_idx: Mutex<usize>,
    pub lyrics: Mutex<Vec<LyricLine>>,
    pub volume: Mutex<f32>,
}

impl LocalPlayerState {
    pub fn new() -> Self {
        Self {
            player: AudioPlayer::spawn(),
            queue: Mutex::new(Vec::new()),
            queue_idx: Mutex::new(0),
            lyrics: Mutex::new(Vec::new()),
            volume: Mutex::new(1.0),
        }
    }
}



#[tauri::command]
pub fn load_local_file(path: String, state: State<'_, LocalPlayerState>) {
    let pb = PathBuf::from(&path);
    if !queue::is_audio_path(&pb) {
        return;
    }
    let item = QueueItem::from_path(pb.clone());

    *state.queue.lock().unwrap() = vec![item];
    *state.queue_idx.lock().unwrap() = 0;

    *state.lyrics.lock().unwrap() = lyrics::from_sidecar(&pb).unwrap_or_default();

    state.player.send(AudioCommand::Load(pb));
}

#[tauri::command]
pub fn enqueue_files(paths: Vec<String>, state: State<'_, LocalPlayerState>) {
    let mut queue = state.queue.lock().unwrap();
    for p in paths {
        let pb = PathBuf::from(&p);
        if queue::is_audio_path(&pb) {
            queue.push(QueueItem::from_path(pb));
        }
    }
}

#[tauri::command]
pub fn play_pause_local(state: State<'_, LocalPlayerState>) {
    state.player.send(AudioCommand::PlayPause);
}

#[tauri::command]
pub fn seek_local(seconds: f64, state: State<'_, LocalPlayerState>) {
    state.player.send(AudioCommand::Seek(seconds));
}

#[tauri::command]
pub fn set_volume_local(volume: f32, state: State<'_, LocalPlayerState>) {
    *state.volume.lock().unwrap() = volume;
    state.player.send(AudioCommand::SetVolume(volume));
}

#[tauri::command]
pub fn stop_local(state: State<'_, LocalPlayerState>) {
    state.player.send(AudioCommand::Stop);
    *state.lyrics.lock().unwrap() = Vec::new();
}

#[tauri::command]
pub fn get_local_state(state: State<'_, LocalPlayerState>) -> AudioState {
    state.player.get_state()
}

#[tauri::command]
pub fn get_current_lyric(seconds: f64, state: State<'_, LocalPlayerState>) -> Option<String> {
    let lines = state.lyrics.lock().unwrap();
    let idx = lyrics::current_line(&lines, seconds)?;
    Some(lines[idx].text.clone())
}

#[tauri::command]
pub fn get_queue(state: State<'_, LocalPlayerState>) -> Vec<QueueEntry> {
    state
        .queue
        .lock()
        .unwrap()
        .iter()
        .map(QueueEntry::from)
        .collect()
}

#[tauri::command]
pub fn play_queue_index(index: usize, state: State<'_, LocalPlayerState>) {
    let queue = state.queue.lock().unwrap();
    if let Some(item) = queue.get(index) {
        let pb = item.path.clone();
        drop(queue);
        *state.queue_idx.lock().unwrap() = index;
        *state.lyrics.lock().unwrap() = lyrics::from_sidecar(&pb).unwrap_or_default();
        state.player.send(AudioCommand::Load(pb));
    }
}

#[tauri::command]
pub fn next_track(state: State<'_, LocalPlayerState>) {
    let idx = {
        let queue = state.queue.lock().unwrap();
        let current = *state.queue_idx.lock().unwrap();
        if queue.is_empty() {
            return;
        }
        (current + 1) % queue.len()
    };
    play_queue_index(idx, state);
}

#[tauri::command]
pub fn prev_track(state: State<'_, LocalPlayerState>) {
    let idx = {
        let queue = state.queue.lock().unwrap();
        let current = *state.queue_idx.lock().unwrap();
        if queue.is_empty() {
            return;
        }
        if current == 0 {
            queue.len() - 1
        } else {
            current - 1
        }
    };
    play_queue_index(idx, state);
}
