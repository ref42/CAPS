use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use rodio::Source;

#[derive(Debug)]
pub enum AudioCommand {
    Load(PathBuf),
    PlayPause,
    Seek(f64),
    SetVolume(f32),
    Stop,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct AudioState {
    pub is_playing: bool,
    pub position: f64,
    pub duration: f64,
    pub path: String,
    pub title: String,
}

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
    let mut volume = 1.0_f32;

    loop {
        
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(AudioCommand::Load(path)) => {
                let file = match std::fs::File::open(&path) {
                    Ok(file) => file,
                    Err(e) => {
                        log::error!("audio: open {:?}: {e}", path);
                        continue;
                    }
                };
                let source = match rodio::Decoder::try_from(file) {
                    Ok(source) => source,
                    Err(e) => {
                        log::error!("audio: decode {:?}: {e}", path);
                        continue;
                    }
                };
                duration_secs = source
                    .total_duration()
                    .map(|duration| duration.as_secs_f64())
                    .or_else(|| probe_duration(&path))
                    .unwrap_or(0.0);
                current_path = path.to_string_lossy().into_owned();
                current_title = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();

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
                );
            }
            Ok(AudioCommand::Seek(secs)) => {
                if let Some(player) = &player {
                    let _ = player.try_seek(Duration::from_secs_f64(secs.max(0.0)));
                }
                update_state(
                    &state,
                    &player,
                    duration_secs,
                    &current_path,
                    &current_title,
                );
            }
            Ok(AudioCommand::SetVolume(v)) => {
                volume = v.clamp(0.0, 2.0);
                if let Some(player) = &player {
                    player.set_volume(volume);
                }
            }
            Ok(AudioCommand::Stop) => {
                if let Some(player) = player.take() {
                    player.stop();
                }
                duration_secs = 0.0;
                current_path.clear();
                current_title.clear();
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
) {
    let mut s = state.lock().unwrap();
    if let Some(player) = player {
        s.is_playing = !player.is_paused() && !player.empty();
        s.position = player.get_pos().as_secs_f64();
    } else {
        s.is_playing = false;
        s.position = 0.0;
    }
    s.duration = duration;
    s.path = path.to_owned();
    s.title = title.to_owned();
}


fn probe_duration(path: &PathBuf) -> Option<f64> {
    use symphonia::core::{
        formats::probe::Hint,
        formats::{FormatOptions, TrackType},
        io::MediaSourceStream,
        meta::MetadataOptions,
    };

    let file = std::fs::File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let meta_opts = MetadataOptions::default();
    let fmt_opts = FormatOptions::default();
    let probed = symphonia::default::get_probe()
        .probe(&hint, mss, fmt_opts, meta_opts)
        .ok()?;
    let track = probed.default_track(TrackType::Audio)?;
    let tb = track.time_base?;
    let duration = track
        .duration
        .map(|duration| duration.get())
        .or(track.num_frames)?;
    Some(duration as f64 * f64::from(tb.numer.get()) / f64::from(tb.denom.get()))
}
