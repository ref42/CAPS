use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use rodio::Source;

#[derive(Debug)]
pub enum AudioCommand {
    LoadFile {
        path: String,
        title: String,
        detail: String,
    },
    PlayPause,
    Seek(f64),
    SetVolume(f32),
    Stop,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct AudioState {
    pub is_playing: bool,
    pub is_finished: bool,
    pub position: f64,
    pub duration: f64,
    pub path: String,
    pub title: String,
    pub detail: String,
}

#[derive(Clone)]
pub struct AudioPlayer {
    pub cmd_tx: std::sync::mpsc::Sender<AudioCommand>,
    pub state: Arc<Mutex<AudioState>>,
}

impl AudioPlayer {
    pub fn spawn() -> Self {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<AudioCommand>();
        let state = Arc::new(Mutex::new(AudioState::default()));
        let state_clone = Arc::clone(&state);

        std::thread::spawn(move || {
            audio_thread(cmd_rx, state_clone);
        });

        Self { cmd_tx, state }
    }

    pub fn send(&self, cmd: AudioCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn get_state(&self) -> AudioState {
        self.state.lock().unwrap().clone()
    }
}

fn audio_thread(rx: std::sync::mpsc::Receiver<AudioCommand>, state: Arc<Mutex<AudioState>>) {
    let Ok(mut output) = rodio::DeviceSinkBuilder::open_default_sink() else {
        log::error!("audio: failed to open output stream");
        return;
    };
    output.log_on_drop(false);

    let mut player: Option<rodio::Player> = None;
    let mut duration_secs: f64 = 0.0;
    let mut current_path = String::new();
    let mut current_title = String::new();
    let mut current_detail = String::new();
    let mut volume = 1.0_f32;

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(AudioCommand::LoadFile {
                path,
                title,
                detail,
            }) => {
                let file = match std::fs::File::open(&path) {
                    Ok(file) => file,
                    Err(e) => {
                        log::error!("audio: open stream file: {e}");
                        continue;
                    }
                };
                let byte_len = file.metadata().map(|meta| meta.len()).unwrap_or(0);
                let source = match rodio::Decoder::builder()
                    .with_data(file)
                    .with_byte_len(byte_len)
                    .build()
                {
                    Ok(source) => source,
                    Err(e) => {
                        log::error!("audio: decode stream: {e}");
                        continue;
                    }
                };
                duration_secs = source
                    .total_duration()
                    .map(|duration| duration.as_secs_f64())
                    .unwrap_or(0.0);
                current_path = path;
                current_title = title;
                current_detail = detail;

                let new_player = rodio::Player::connect_new(output.mixer());
                new_player.set_volume(volume);
                new_player.append(source);
                player = Some(new_player);
                update_state(
                    &state,
                    &player,
                    duration_secs,
                    &current_path,
                    &current_title,
                    &current_detail,
                );
            }
            Ok(AudioCommand::PlayPause) => {
                if let Some(player) = &player {
                    if player.is_paused() {
                        player.play();
                    } else {
                        player.pause();
                    }
                }
                update_state(
                    &state,
                    &player,
                    duration_secs,
                    &current_path,
                    &current_title,
                    &current_detail,
                );
            }
            Ok(AudioCommand::SetVolume(value)) => {
                volume = value.clamp(0.0, 2.0);
                if let Some(player) = &player {
                    player.set_volume(volume);
                }
            }
            Ok(AudioCommand::Seek(position)) => {
                if let Some(player) = &player {
                    let target = if duration_secs > 0.0 {
                        position.clamp(0.0, duration_secs)
                    } else {
                        position.max(0.0)
                    };
                    let _ = player.try_seek(Duration::from_secs_f64(target));
                }
                update_state(
                    &state,
                    &player,
                    duration_secs,
                    &current_path,
                    &current_title,
                    &current_detail,
                );
            }
            Ok(AudioCommand::Stop) => {
                if let Some(player) = player.take() {
                    player.stop();
                }
                duration_secs = 0.0;
                current_path.clear();
                current_title.clear();
                current_detail.clear();
                *state.lock().unwrap() = AudioState::default();
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                update_state(
                    &state,
                    &player,
                    duration_secs,
                    &current_path,
                    &current_title,
                    &current_detail,
                );
            }
        }
    }
}

fn update_state(
    state: &Arc<Mutex<AudioState>>,
    player: &Option<rodio::Player>,
    duration: f64,
    path: &str,
    title: &str,
    detail: &str,
) {
    let mut s = state.lock().unwrap();
    if let Some(player) = player {
        let is_empty = player.empty();
        s.is_playing = !player.is_paused() && !is_empty;
        s.is_finished = is_empty && !title.is_empty();
        s.position = player.get_pos().as_secs_f64();
    } else {
        s.is_playing = false;
        s.is_finished = false;
        s.position = 0.0;
    }
    s.duration = duration;
    s.path = path.to_owned();
    s.title = title.to_owned();
    if !detail.is_empty() {
        s.detail = detail.to_owned();
    }
}
