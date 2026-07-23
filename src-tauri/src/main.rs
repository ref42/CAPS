#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod audio_spectrum;
mod netease;

use audio::{AudioCommand, AudioPlayer};
use dioxus::desktop::tao::window::Window;
use dioxus::desktop::{Config, DesktopContext, LogicalPosition, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use serde_json::Value;
use std::sync::Arc;
use sysinfo::Networks;

#[cfg(target_os = "windows")]
use dioxus::desktop::tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};

const LOGO: &str = include_str!("../../src/assets/qiuniu.logo");
const COLLAPSED_W: f64 = 210.0;
const COLLAPSED_H: f64 = 40.0;
const EXPANDED_W: f64 = 392.0;
const EXPANDED_H: f64 = 454.0;

#[derive(Clone, Debug, PartialEq)]
struct Track {
    id: String,
    name: String,
    artist: String,
    album: String,
    cover: String,
}

fn main() {
    audio_spectrum::start_monitor();
    dioxus::LaunchBuilder::desktop()
        .with_cfg(desktop_config())
        .launch(App);
}

fn desktop_config() -> Config {
    let mut window = WindowBuilder::new()
        .with_title("QiuNiu Island")
        .with_inner_size(LogicalSize::new(COLLAPSED_W, COLLAPSED_H))
        .with_resizable(false)
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top(true)
        .with_visible(true);

    #[cfg(target_os = "windows")]
    {
        window = window
            .with_skip_taskbar(true)
            .with_undecorated_shadow(false);
    }

    Config::new()
        .with_window(window)
        .with_background_color((0, 0, 0, 0))
        .with_disable_context_menu(true)
        .with_on_window(|window, _| {
            window.set_always_on_top(true);
            #[cfg(target_os = "windows")]
            {
                let _ = window.set_skip_taskbar(true);
                window.set_undecorated_shadow(false);
            }
            place_top_center(&window, COLLAPSED_W);
        })
}

#[component]
fn App() -> Element {
    let desktop = dioxus::desktop::window();
    let player = use_hook(|| Arc::new(AudioPlayer::spawn()));
    let mut expanded = use_signal(|| false);
    let mut pointer_inside = use_signal(|| false);
    let mut input_focused = use_signal(|| false);
    let mut active_tab = use_signal(|| "search".to_string());
    let mut opacity = use_signal(|| 92_u32);
    let mut media_island = use_signal(|| true);
    let mut glow_border = use_signal(|| true);
    let mut spring_style = use_signal(|| "smooth".to_string());
    let mut query = use_signal(String::new);
    let results = use_signal(Vec::<Track>::new);
    let mut queue = use_signal(Vec::<Track>::new);
    let mut current_index = use_signal(|| None::<usize>);
    let mut status =
        use_signal(|| "Search NetEase, add songs, then play from the island.".to_string());
    let mut audio_state = use_signal(|| player.get_state());
    let mut spectrum = use_signal(audio_spectrum::get_audio_spectrum);
    let mut upload = use_signal(|| "0 B/s".to_string());
    let mut download = use_signal(|| "0 B/s".to_string());
    let mut total_upload = use_signal(|| 0_u64);
    let mut total_download = use_signal(|| 0_u64);

    let player_for_state = player.clone();
    use_effect(move || {
        let player = player_for_state.clone();
        spawn(async move {
            loop {
                audio_state.set(player.get_state());
                spectrum.set(audio_spectrum::get_audio_spectrum());
                tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            }
        });
    });

    use_effect(move || {
        spawn(async move {
            let mut networks = Networks::new_with_refreshed_list();
            let mut last_rx = 0_u64;
            let mut last_tx = 0_u64;
            loop {
                networks.refresh_list();
                let mut rx = 0_u64;
                let mut tx = 0_u64;
                for (_, data) in networks.iter() {
                    rx += data.total_received();
                    tx += data.total_transmitted();
                }
                if last_rx != 0 || last_tx != 0 {
                    download.set(format_rate(rx.saturating_sub(last_rx)));
                    upload.set(format_rate(tx.saturating_sub(last_tx)));
                }
                total_download.set(rx);
                total_upload.set(tx);
                last_rx = rx;
                last_tx = tx;
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });
    });

    let mut expand_window = {
        let desktop = desktop.clone();
        move || {
            expanded.set(true);
            set_island_window(&desktop, true);
        }
    };

    let mut collapse_window = {
        let desktop = desktop.clone();
        move || {
            if !*input_focused.read() {
                expanded.set(false);
                set_island_window(&desktop, false);
            }
        }
    };

    let load_random = move |count: u32| {
        let mut queue = queue;
        let mut status = status;
        spawn(async move {
            status.set(format!("Loading random {count}..."));
            match netease::random_netease_queue(Some(count), None).await {
                Ok(items) => {
                    let tracks = items.into_iter().map(Track::from).collect::<Vec<_>>();
                    let loaded = tracks.len();
                    queue.set(tracks);
                    status.set(format!("Queued {loaded} random tracks."));
                }
                Err(err) => status.set(err),
            }
        });
    };

    let player_for_next = player.clone();
    let play_next = move |_| {
        let len = queue.read().len();
        if len == 0 {
            status.set("Queue is empty.".to_string());
            return;
        }
        let next = (*current_index.read()).map(|i| (i + 1) % len).unwrap_or(0);
        current_index.set(Some(next));
        if let Some(track) = queue.read().get(next).cloned() {
            spawn_play(track, player_for_next.clone(), status);
        }
    };

    let player_for_prev = player.clone();
    let play_prev = move |_| {
        let len = queue.read().len();
        if len == 0 {
            status.set("Queue is empty.".to_string());
            return;
        }
        let prev = (*current_index.read())
            .map(|i| if i == 0 { len - 1 } else { i - 1 })
            .unwrap_or(0);
        current_index.set(Some(prev));
        if let Some(track) = queue.read().get(prev).cloned() {
            spawn_play(track, player_for_prev.clone(), status);
        }
    };

    let state = audio_state.read().clone();
    let active_track = current_index
        .read()
        .and_then(|index| queue.read().get(index).cloned());
    let has_music = media_island() && (!state.title.is_empty() || active_track.is_some());
    let current_title = if state.title.is_empty() {
        active_track
            .as_ref()
            .map(|track| track.name.clone())
            .unwrap_or_else(|| "QiuNiu".to_string())
    } else {
        state.title.clone()
    };
    let current_detail = if state.detail.is_empty() {
        active_track
            .as_ref()
            .map(|track| track.artist.clone())
            .unwrap_or_else(|| "NetEase island player".to_string())
    } else {
        state.detail.clone()
    };
    let cover_style = active_track
        .as_ref()
        .filter(|track| !track.cover.is_empty())
        .map(|track| format!("background-image: url('{}');", track.cover))
        .unwrap_or_default();
    let progress = if state.duration > 0.0 {
        (state.position / state.duration * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let tab = active_tab.read().clone();
    let is_expanded = *expanded.read();
    let queue_len = queue.read().len();
    let opacity_css = (*opacity.read() as f64 / 100.0).clamp(0.2, 1.0);
    let stage_style = format!("--island-opacity: {opacity_css:.2};");
    let spring_class = spring_style.read().clone();
    let stage_class = if is_expanded {
        "stage expanded"
    } else {
        "stage"
    };
    let glow_class = if glow_border() {
        "island glow"
    } else {
        "island"
    };
    let cover_class = if state.is_playing {
        "cover playing"
    } else {
        "cover"
    };

    rsx! {
        style { "{APP_CSS}" }
        main {
            class: "{stage_class} {spring_class}",
            style: "{stage_style}",
            onmouseenter: move |_| {
                pointer_inside.set(true);
                expand_window();
            },
            onmouseleave: move |_| {
                pointer_inside.set(false);
                collapse_window();
            },
            oncontextmenu: {
                let desktop = desktop.clone();
                move |_| desktop.close()
            },
            section {
                class: "{glow_class}",
                onmousedown: {
                    let desktop = desktop.clone();
                    move |_| desktop.drag()
                },
                div { class: "core",
                    if has_music {
                        div { class: "{cover_class}",
                            style: "{cover_style}",
                            if cover_style.is_empty() {
                                div { class: "logo", dangerous_inner_html: LOGO }
                            }
                        }
                        div { class: "music-copy",
                            strong { "{current_title}" }
                            span { "{current_detail}" }
                        }
                    } else {
                        div { class: "logo speed-logo", dangerous_inner_html: LOGO }
                        div { class: "speed-copy",
                            strong { "{download}" }
                            span { "DOWN" }
                        }
                    }
                }
                if is_expanded {
                    div { class: "mini-controls",
                        button { onclick: play_prev, title: "Previous", "⏮" }
                        button {
                            onclick: {
                                let player = player.clone();
                                move |_| player.send(AudioCommand::PlayPause)
                            },
                            title: "Play/Pause",
                            if state.is_playing { "Ⅱ" } else { "▶" }
                        }
                        button { onclick: play_next, title: "Next", "⏭" }
                        button {
                            onclick: {
                                let player = player.clone();
                                move |_| {
                                    current_index.set(None);
                                    player.send(AudioCommand::Stop);
                                    status.set("Stopped.".to_string());
                                }
                            },
                            title: "Stop",
                            "■"
                        }
                    }
                } else {
                    div { class: "speed-chip",
                        span { "UP {upload}" }
                    }
                }
                div { class: "spectrum",
                    for value in spectrum.read().iter() {
                        i { style: "transform: scaleY({value});" }
                    }
                }
                if is_expanded {
                    div { class: "playback-progress",
                        span { style: "width: {progress}%;" }
                    }
                }
            }

            section { class: "panel",
                div { class: "tabs",
                    button {
                        class: if tab == "search" { "tab active" } else { "tab" },
                        onclick: move |_| active_tab.set("search".to_string()),
                        "Search"
                    }
                    button {
                        class: if tab == "queue" { "tab active" } else { "tab" },
                        onclick: move |_| active_tab.set("queue".to_string()),
                        "Queue"
                    }
                    button {
                        class: if tab == "stats" { "tab active" } else { "tab" },
                        onclick: move |_| active_tab.set("stats".to_string()),
                        "Stats"
                    }
                    button {
                        class: if tab == "settings" { "tab active" } else { "tab" },
                        onclick: move |_| active_tab.set("settings".to_string()),
                        "Settings"
                    }
                }

                if tab == "search" {
                    div { class: "panel-section",
                        div { class: "search-row",
                            input {
                                value: "{query}",
                                placeholder: "Search NetEase",
                                onfocus: move |_| input_focused.set(true),
                                onblur: {
                                    let desktop = desktop.clone();
                                    move |_| {
                                        input_focused.set(false);
                                        if !*pointer_inside.read() {
                                            expanded.set(false);
                                            set_island_window(&desktop, false);
                                        }
                                    }
                                },
                                oninput: move |event| query.set(event.value()),
                                onkeydown: move |event| {
                                    if event.key() == Key::Enter {
                                        spawn_search(query.read().trim().to_string(), results, status);
                                    }
                                }
                            }
                            button {
                                class: "icon-button",
                                onclick: move |_| spawn_search(query.read().trim().to_string(), results, status),
                                "⌕"
                            }
                        }
                        div { class: "random-row",
                            button { onclick: move |_| load_random(50), "Random 50" }
                            button { onclick: move |_| load_random(100), "Random 100" }
                        }
                        div { class: "song-list",
                            for track in results.read().iter().cloned() {
                                TrackRow {
                                    track: track.clone(),
                                    action: "+",
                                    active: false,
                                    onclick: move |_| {
                                        let mut next = queue.read().clone();
                                        next.push(track.clone());
                                        let added = next.len();
                                        queue.set(next);
                                        status.set(format!("Queued {added} tracks."));
                                    }
                                }
                            }
                            if results.read().is_empty() {
                                div { class: "empty", "{status}" }
                            }
                        }
                    }
                } else if tab == "queue" {
                    div { class: "panel-section",
                        div { class: "queue-toolbar",
                            span { "{queue_len} tracks" }
                            button {
                                onclick: move |_| {
                                    queue.set(Vec::new());
                                    current_index.set(None);
                                    status.set("Queue cleared.".to_string());
                                },
                                "Clear"
                            }
                        }
                        div { class: "song-list",
                            for (index, track) in queue.read().iter().cloned().enumerate() {
                                TrackRow {
                                    track,
                                    action: "Play",
                                    active: current_index.read().is_some_and(|i| i == index),
                                    onclick: {
                                        let player = player.clone();
                                        move |_| {
                                            let list = queue.read().clone();
                                            if let Some(track) = list.get(index).cloned() {
                                                current_index.set(Some(index));
                                                spawn_play(track, player.clone(), status);
                                            }
                                        }
                                    }
                                }
                            }
                            if queue.read().is_empty() {
                                div { class: "empty", "Queue is empty." }
                            }
                        }
                    }
                } else if tab == "stats" {
                    div { class: "panel-section stats",
                        div { class: "speed-grid",
                            div { span { "Upload" } strong { "{upload}" } }
                            div { span { "Download" } strong { "{download}" } }
                        }
                        div { class: "stat-line", span { "Total up" } strong { "{format_bytes(*total_upload.read())}" } }
                        div { class: "stat-line", span { "Total down" } strong { "{format_bytes(*total_download.read())}" } }
                        div { class: "stat-line", span { "This month" } strong { "{format_bytes(total_upload() + total_download())}" } }
                        div { class: "status-text", "{status}" }
                    }
                } else {
                    div { class: "panel-section settings",
                        label { class: "setting",
                            span { "Opacity" }
                            input {
                                r#type: "range",
                                min: "20",
                                max: "100",
                                value: "{opacity}",
                                oninput: move |event| {
                                    if let Ok(value) = event.value().parse::<u32>() {
                                        opacity.set(value.clamp(20, 100));
                                    }
                                }
                            }
                        }
                        label { class: "setting",
                            span { "Media island" }
                            input {
                                r#type: "checkbox",
                                checked: media_island(),
                                onchange: move |_| media_island.set(!media_island())
                            }
                        }
                        label { class: "setting",
                            span { "Glow border" }
                            input {
                                r#type: "checkbox",
                                checked: glow_border(),
                                onchange: move |_| glow_border.set(!glow_border())
                            }
                        }
                        div { class: "setting",
                            span { "Animation" }
                            div { class: "segmented",
                                button {
                                    class: if spring_style.read().as_str() == "smooth" { "active" } else { "" },
                                    onclick: move |_| spring_style.set("smooth".to_string()),
                                    "Smooth"
                                }
                                button {
                                    class: if spring_style.read().as_str() == "bouncy" { "active" } else { "" },
                                    onclick: move |_| spring_style.set("bouncy".to_string()),
                                    "Bouncy"
                                }
                            }
                        }
                        div { class: "status-text", "Right-click the island to exit QiuNiu." }
                    }
                }
            }
        }
    }
}

#[component]
fn TrackRow(
    track: Track,
    action: &'static str,
    active: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    let cover_style = if track.cover.is_empty() {
        String::new()
    } else {
        format!("background-image: url('{}');", track.cover)
    };
    rsx! {
        button { class: if active { "song active" } else { "song" }, onclick,
            span { class: "song-cover", style: "{cover_style}" }
            span { class: "song-copy",
                strong { "{track.name}" }
                small { "{track.artist}" }
            }
            span { class: "song-action", "{action}" }
        }
    }
}

fn spawn_play(track: Track, player: Arc<AudioPlayer>, mut status: Signal<String>) {
    spawn(async move {
        status.set(format!("Loading {}...", track.name));
        let url =
            match netease::get_netease_song_url(track.id.clone(), Some("exhigh".to_string()), None)
                .await
            {
                Ok(info) => info.url.unwrap_or_default(),
                Err(err) => {
                    status.set(err);
                    return;
                }
            };
        if url.is_empty() {
            status.set("No playable stream for this track.".to_string());
            return;
        }
        match reqwest::get(&url).await {
            Ok(response) => match response.bytes().await {
                Ok(bytes) => {
                    player.send(AudioCommand::LoadBytes {
                        bytes: bytes.to_vec(),
                        title: track.name.clone(),
                        detail: track.artist.clone(),
                    });
                    status.set(format!("Playing {}.", track.name));
                }
                Err(err) => status.set(format!("Stream read failed: {err}")),
            },
            Err(err) => status.set(format!("Stream request failed: {err}")),
        }
    });
}

fn spawn_search(text: String, mut results: Signal<Vec<Track>>, mut status: Signal<String>) {
    if text.is_empty() {
        status.set("Type a song name first.".to_string());
        return;
    }
    spawn(async move {
        status.set("Searching NetEase...".to_string());
        match netease::search_netease_songs(text, Some(18), None).await {
            Ok(items) => {
                let tracks = items.into_iter().map(Track::from).collect::<Vec<_>>();
                let count = tracks.len();
                results.set(tracks);
                status.set(format!("Found {count} tracks."));
            }
            Err(err) => status.set(err),
        }
    });
}

fn set_island_window(desktop: &DesktopContext, expanded: bool) {
    let (width, height) = if expanded {
        (EXPANDED_W, EXPANDED_H)
    } else {
        (COLLAPSED_W, COLLAPSED_H)
    };
    desktop.set_inner_size(LogicalSize::new(width, height));
    desktop.set_always_on_top(true);
    place_top_center(desktop, width);
}

fn place_top_center(window: &Window, width: f64) {
    if let Some(monitor) = window
        .current_monitor()
        .or_else(|| window.primary_monitor())
    {
        let scale = monitor.scale_factor();
        let size = monitor.size().to_logical::<f64>(scale);
        let position = monitor.position().to_logical::<f64>(scale);
        let x = position.x + ((size.width - width) / 2.0).max(0.0);
        window.set_outer_position(LogicalPosition::new(x.round(), position.y + 8.0));
    }
}

fn format_rate(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let value = bytes as f64;
    if value >= MB {
        format!("{:.1} MB/s", value / MB)
    } else if value >= KB {
        format!("{:.0} KB/s", value / KB)
    } else {
        format!("{bytes} B/s")
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let value = bytes as f64;
    if value >= GB {
        format!("{:.2} GB", value / GB)
    } else if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.0} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

impl From<netease::NeteaseSong> for Track {
    fn from(song: netease::NeteaseSong) -> Self {
        Self {
            id: value_id(&song.id),
            name: song.name,
            artist: clean_or(song.artist, "Unknown artist"),
            album: clean_or(song.album, "Unknown album"),
            cover: song.cover.unwrap_or_default(),
        }
    }
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

const APP_CSS: &str = r#"
html,
body,
#main {
  margin: 0;
  width: 100%;
  height: 100%;
  background: transparent;
  overflow: hidden;
}

* {
  box-sizing: border-box;
}

body,
button,
input {
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI Variable", "Segoe UI", sans-serif;
}

button,
input {
  border: 0;
  color: inherit;
}

button {
  cursor: pointer;
}

.stage {
  width: 210px;
  height: 40px;
  color: rgba(248, 255, 252, 0.96);
  user-select: none;
  -webkit-font-smoothing: antialiased;
  overflow: hidden;
  background: transparent;
}

.stage.expanded {
  width: 392px;
  height: 454px;
}

.island {
  position: relative;
  width: 210px;
  height: 40px;
  display: grid;
  grid-template-columns: 1fr auto 42px;
  align-items: center;
  gap: 8px;
  padding: 4px 8px 4px 5px;
  border-radius: 999px;
  background: rgba(5, 8, 9, var(--island-opacity));
  border: 1px solid rgba(118, 244, 207, 0.2);
  box-shadow: 0 10px 34px rgba(0, 0, 0, 0.34), inset 0 1px rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(22px) saturate(1.25);
  overflow: hidden;
}

.island.glow::before {
  content: "";
  position: absolute;
  inset: -1px;
  border-radius: inherit;
  padding: 1px;
  background: linear-gradient(90deg, rgba(121, 244, 205, 0.75), rgba(226, 194, 91, 0.65), rgba(121, 244, 205, 0.5));
  mask: linear-gradient(#000 0 0) content-box, linear-gradient(#000 0 0);
  mask-composite: exclude;
  pointer-events: none;
  opacity: 0.55;
}

.expanded .island {
  width: 392px;
  height: 76px;
  grid-template-columns: 1fr 116px 54px;
  padding: 9px 14px 9px 10px;
  border-radius: 30px;
}

.smooth .island,
.smooth .panel {
  transition: width 260ms cubic-bezier(0.2, 0, 0, 1), height 260ms cubic-bezier(0.2, 0, 0, 1), border-radius 260ms cubic-bezier(0.2, 0, 0, 1), opacity 180ms ease, transform 260ms cubic-bezier(0.2, 0, 0, 1);
}

.bouncy .island,
.bouncy .panel {
  transition: width 420ms cubic-bezier(0.18, 1.15, 0.22, 1), height 420ms cubic-bezier(0.18, 1.15, 0.22, 1), border-radius 420ms cubic-bezier(0.18, 1.15, 0.22, 1), opacity 190ms ease, transform 420ms cubic-bezier(0.18, 1.15, 0.22, 1);
}

.core {
  min-width: 0;
  height: 100%;
  display: grid;
  grid-template-columns: 30px 1fr;
  align-items: center;
  gap: 8px;
}

.expanded .core {
  grid-template-columns: 46px 1fr;
  gap: 11px;
}

.logo,
.logo svg {
  width: 100%;
  height: 100%;
  display: block;
}

.speed-logo {
  width: 30px;
  height: 30px;
}

.cover {
  width: 30px;
  height: 30px;
  border-radius: 50%;
  background: linear-gradient(135deg, #14352f, #d8b45b);
  background-position: center;
  background-size: cover;
  display: grid;
  place-items: center;
  overflow: hidden;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.16);
}

.expanded .cover {
  width: 46px;
  height: 46px;
  border-radius: 11px;
}

.cover.playing {
  animation: coverPulse 1800ms ease-in-out infinite;
}

.cover .logo {
  width: 26px;
  height: 26px;
}

.music-copy,
.speed-copy {
  min-width: 0;
  display: grid;
  gap: 2px;
}

.music-copy strong,
.speed-copy strong {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  font-size: 12.5px;
  line-height: 1.05;
  font-weight: 760;
  letter-spacing: 0;
}

.expanded .music-copy strong,
.expanded .speed-copy strong {
  font-size: 15px;
}

.music-copy span,
.speed-copy span {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: rgba(248, 255, 252, 0.58);
  font-size: 11px;
  line-height: 1.1;
  font-weight: 650;
  letter-spacing: 0;
}

.speed-chip {
  height: 23px;
  min-width: 72px;
  display: grid;
  place-items: center;
  padding: 0 8px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.08);
  color: #7df2ca;
  font-size: 10px;
  font-weight: 800;
}

.mini-controls {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 6px;
}

.mini-controls button {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.09);
  color: rgba(248, 255, 252, 0.94);
  font-size: 12px;
  font-weight: 900;
  transition: transform 80ms ease-out, background-color 160ms ease;
}

.mini-controls button:hover {
  background: rgba(255, 255, 255, 0.16);
}

.mini-controls button:active,
.tab:active,
.song:active,
.random-row button:active,
.queue-toolbar button:active,
.segmented button:active,
.icon-button:active {
  transform: scale(0.94);
}

.spectrum {
  height: 26px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
}

.spectrum i {
  width: 3px;
  height: 20px;
  min-height: 6px;
  border-radius: 999px;
  background: linear-gradient(180deg, #fff0a0, #78f2ca);
  transform-origin: center;
  transition: transform 80ms linear;
}

.expanded .spectrum i {
  height: 30px;
}

.playback-progress {
  position: absolute;
  left: 18px;
  right: 18px;
  bottom: 9px;
  height: 3px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.1);
  overflow: hidden;
}

.playback-progress span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, #78f2ca, #fff0a0);
}

.panel {
  width: 392px;
  height: 366px;
  margin-top: 8px;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 9px;
  border-radius: 22px;
  background: rgba(19, 24, 26, 0.86);
  border: 1px solid rgba(255, 255, 255, 0.12);
  box-shadow: 0 24px 80px rgba(0, 0, 0, 0.34), inset 0 1px rgba(255, 255, 255, 0.08);
  backdrop-filter: blur(24px) saturate(1.2);
  opacity: 0;
  transform: translateY(-8px) scale(0.985);
  pointer-events: none;
}

.expanded .panel {
  opacity: 1;
  transform: translateY(0) scale(1);
  pointer-events: auto;
}

.tabs {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 5px;
  padding: 3px;
  border-radius: 12px;
  background: rgba(142, 142, 147, 0.15);
}

.tab {
  height: 28px;
  border-radius: 9px;
  background: transparent;
  color: rgba(248, 255, 252, 0.62);
  font-size: 11px;
  font-weight: 800;
  transition: background-color 160ms ease, color 160ms ease, transform 80ms ease;
}

.tab.active {
  background: rgba(255, 255, 255, 0.16);
  color: rgba(248, 255, 252, 0.96);
}

.panel-section {
  min-height: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.search-row {
  display: grid;
  grid-template-columns: 1fr 34px;
  gap: 7px;
}

.search-row input {
  height: 34px;
  min-width: 0;
  padding: 0 10px;
  border-radius: 10px;
  outline: none;
  background: rgba(142, 142, 147, 0.16);
  color: rgba(248, 255, 252, 0.94);
  font-size: 12px;
  font-weight: 650;
}

.search-row input:focus {
  box-shadow: inset 0 0 0 1px rgba(121, 244, 205, 0.55);
}

.icon-button {
  width: 34px;
  height: 34px;
  border-radius: 10px;
  background: rgba(48, 209, 88, 0.2);
  color: #7df2ca;
  font-size: 18px;
  font-weight: 800;
}

.random-row,
.queue-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.random-row button,
.queue-toolbar button {
  height: 30px;
  padding: 0 12px;
  border-radius: 10px;
  background: rgba(142, 142, 147, 0.18);
  color: rgba(248, 255, 252, 0.88);
  font-size: 12px;
  font-weight: 800;
  transition: background-color 160ms ease, transform 80ms ease;
}

.random-row button:hover,
.queue-toolbar button:hover,
.song:hover,
.segmented button:hover {
  background: rgba(255, 255, 255, 0.15);
}

.queue-toolbar span {
  color: rgba(248, 255, 252, 0.62);
  font-size: 12px;
  font-weight: 800;
}

.song-list {
  min-height: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 5px;
  overflow-y: auto;
  padding-right: 2px;
}

.song-list::-webkit-scrollbar {
  width: 4px;
}

.song-list::-webkit-scrollbar-thumb {
  border-radius: 999px;
  background: rgba(142, 142, 147, 0.45);
}

.song {
  width: 100%;
  min-height: 42px;
  display: grid;
  grid-template-columns: 32px 1fr auto;
  align-items: center;
  gap: 9px;
  padding: 5px 7px;
  border-radius: 11px;
  background: transparent;
  text-align: left;
  transition: background-color 160ms ease, transform 80ms ease;
}

.song.active {
  background: rgba(48, 209, 88, 0.18);
}

.song-cover {
  width: 32px;
  height: 32px;
  border-radius: 8px;
  background: linear-gradient(135deg, #12352f, #d9b65f);
  background-position: center;
  background-size: cover;
}

.song-copy {
  min-width: 0;
  display: grid;
  gap: 2px;
}

.song-copy strong,
.song-copy small {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  letter-spacing: 0;
}

.song-copy strong {
  color: rgba(248, 255, 252, 0.94);
  font-size: 12.5px;
  font-weight: 790;
}

.song-copy small {
  color: rgba(248, 255, 252, 0.56);
  font-size: 11px;
  font-weight: 650;
}

.song-action {
  color: #7df2ca;
  font-size: 12px;
  font-weight: 900;
}

.empty {
  margin: auto;
  max-width: 270px;
  color: rgba(248, 255, 252, 0.54);
  text-align: center;
  font-size: 12px;
  font-weight: 650;
  line-height: 1.36;
}

.speed-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.speed-grid div,
.stat-line {
  border-radius: 12px;
  background: rgba(142, 142, 147, 0.15);
}

.speed-grid div {
  min-height: 62px;
  display: grid;
  align-content: center;
  gap: 5px;
  padding: 10px;
}

.speed-grid span,
.stat-line span {
  color: rgba(248, 255, 252, 0.55);
  font-size: 11px;
  font-weight: 800;
}

.speed-grid strong {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 16px;
  letter-spacing: 0;
}

.stat-line {
  min-height: 42px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 0 11px;
}

.stat-line strong {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 12px;
  letter-spacing: 0;
  white-space: nowrap;
}

.settings {
  overflow-y: auto;
  padding-right: 2px;
}

.setting {
  min-height: 42px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  color: rgba(248, 255, 252, 0.84);
  font-size: 12.5px;
  font-weight: 780;
}

.setting input[type="range"] {
  width: 154px;
  accent-color: #34c759;
}

.setting input[type="checkbox"] {
  width: 42px;
  height: 24px;
  accent-color: #34c759;
}

.segmented {
  display: flex;
  gap: 4px;
  padding: 3px;
  border-radius: 10px;
  background: rgba(142, 142, 147, 0.16);
}

.segmented button {
  height: 26px;
  min-width: 62px;
  padding: 0 9px;
  border-radius: 8px;
  background: transparent;
  color: rgba(248, 255, 252, 0.68);
  font-size: 12px;
  font-weight: 800;
}

.segmented button.active {
  background: rgba(255, 255, 255, 0.17);
  color: rgba(248, 255, 252, 0.96);
}

.status-text {
  margin-top: auto;
  color: rgba(248, 255, 252, 0.54);
  font-size: 12px;
  font-weight: 650;
  line-height: 1.36;
}

@keyframes coverPulse {
  0%, 100% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.055);
  }
}

@media (prefers-reduced-motion: reduce) {
  .island,
  .panel,
  .cover,
  .spectrum i,
  button {
    animation: none;
    transition: opacity 120ms ease, background-color 120ms ease;
  }
}
"#;
