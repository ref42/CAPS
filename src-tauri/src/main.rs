#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod audio_spectrum;
mod netease;

use audio::{AudioCommand, AudioPlayer};
use dioxus::desktop::tao::dpi::PhysicalPosition;
use dioxus::desktop::tao::window::Window;
use dioxus::desktop::{Config, DesktopContext, LogicalPosition, LogicalSize, WindowBuilder};
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use serde_json::Value;
use std::sync::Arc;
use sysinfo::Networks;

#[cfg(target_os = "windows")]
use dioxus::desktop::tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};

const COLLAPSED_W: f64 = 300.0;
const COLLAPSED_H: f64 = 56.0;
const EXPANDED_W: f64 = 430.0;
const EXPANDED_H: f64 = 490.0;
const MUSIC_COLLAPSED_W: f64 = EXPANDED_W;
const ISLAND_BLEED: f64 = 18.0;

#[derive(Clone, Debug, PartialEq)]
struct Track {
    id: String,
    name: String,
    artist: String,
    album: String,
    cover: String,
}

#[derive(Clone, Debug, PartialEq)]
struct LyricLine {
    time: f64,
    text: String,
}

#[derive(Clone)]
struct LyricTransition {
    current: String,
    outgoing: Option<String>,
    id: u64,
}

fn main() {
    audio_spectrum::start_monitor();
    dioxus::LaunchBuilder::desktop()
        .with_cfg(desktop_config())
        .launch(App);
}

fn desktop_config() -> Config {
    let mut window = WindowBuilder::new()
        .with_title("CAPS")
        .with_inner_size(LogicalSize::new(
            COLLAPSED_W + ISLAND_BLEED * 2.0,
            COLLAPSED_H + ISLAND_BLEED * 2.0,
        ))
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
            place_top_center(&window, COLLAPSED_W + ISLAND_BLEED * 2.0);
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
    let mut spring_style = use_signal(|| "smooth".to_string());
    let mut volume = use_signal(|| 100_u32);
    let mut island_size = use_signal(|| 100_u32);
    let mut random_count = use_signal(|| 50_u32);
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
    let mut lyrics = use_signal(Vec::<LyricLine>::new);

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

    let mut expand_window = move || {
        expanded.set(true);
    };

    let mut collapse_window = move || {
        if !*input_focused.read() {
            expanded.set(false);
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
            spawn_play(track, player_for_next.clone(), status, lyrics);
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
            spawn_play(track, player_for_prev.clone(), status, lyrics);
        }
    };

    let state = audio_state.read().clone();
    let active_track = current_index
        .read()
        .and_then(|index| queue.read().get(index).cloned());
    let has_music = !state.title.is_empty() || active_track.is_some();
    let current_title = if state.title.is_empty() {
        active_track
            .as_ref()
            .map(|track| track.name.clone())
            .unwrap_or_else(|| "CAPS".to_string())
    } else {
        state.title.clone()
    };
    let current_lyric = current_lyric_line(&lyrics.read(), state.position).unwrap_or_default();
    let primary_text = if has_music && !current_lyric.is_empty() {
        current_lyric.clone()
    } else if has_music {
        current_title.clone()
    } else {
        current_title.clone()
    };
    let primary_class = if has_music && !current_lyric.is_empty() {
        "lyric-title"
    } else {
        "plain-title"
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
    let opacity_css = (*opacity.read() as f64 / 100.0).clamp(0.1, 1.0);
    let island_scale = (*island_size.read() as f64 / 100.0).clamp(0.85, 1.35);
    let collapsed_width = collapsed_width_for_text(&primary_text, has_music);
    let stage_width = collapsed_width + ISLAND_BLEED * 2.0;
    let stage_height = COLLAPSED_H + ISLAND_BLEED * 2.0;
    let island_alpha = (opacity_css * 0.92).clamp(0.08, 0.92);
    let panel_alpha = (opacity_css * 0.86).clamp(0.08, 0.86);
    let soft_alpha = (opacity_css * 0.16).clamp(0.02, 0.16);
    let softer_alpha = (opacity_css * 0.08).clamp(0.01, 0.08);
    let hover_alpha = (opacity_css * 0.15).clamp(0.02, 0.15);
    let active_alpha = (opacity_css * 0.18).clamp(0.02, 0.18);
    let green_alpha = (opacity_css * 0.2).clamp(0.03, 0.2);
    let red_alpha = (opacity_css * 0.22).clamp(0.03, 0.22);
    let stage_style = format!(
        "--island-bg-alpha: {island_alpha:.3}; --panel-bg-alpha: {panel_alpha:.3}; --soft-alpha: {soft_alpha:.3}; --softer-alpha: {softer_alpha:.3}; --hover-alpha: {hover_alpha:.3}; --active-alpha: {active_alpha:.3}; --green-alpha: {green_alpha:.3}; --red-alpha: {red_alpha:.3}; --island-scale: {island_scale:.2}; --collapsed-width: {collapsed_width:.0}px; --stage-width: {stage_width:.0}px; --stage-height: {stage_height:.0}px; --island-bleed: {ISLAND_BLEED:.0}px;"
    );
    let spring_class = spring_style.read().clone();
    let stage_class = if is_expanded {
        "stage expanded"
    } else {
        "stage"
    };
    let core_class = if has_music { "core" } else { "core idle-core" };
    let cover_class = if state.is_playing {
        "cover playing"
    } else {
        "cover"
    };
    let lyric_transition = use_hook(|| {
        std::cell::RefCell::new(LyricTransition {
            current: primary_text.clone(),
            outgoing: None,
            id: 0,
        })
    });
    let (visible_primary_text, outgoing_primary_text, transition_key) = {
        let mut transition = lyric_transition.borrow_mut();
        if transition.current != primary_text {
            transition.outgoing =
                (!transition.current.is_empty()).then(|| transition.current.clone());
            transition.current = primary_text.clone();
            transition.id = transition.id.wrapping_add(1);
        }
        (
            transition.current.clone(),
            transition.outgoing.clone(),
            transition.id,
        )
    };
    let window_size_cache = use_hook(|| std::cell::RefCell::new(None::<(bool, i32, u32)>));

    {
        let desktop = desktop.clone();
        let cache = window_size_cache.clone();
        use_effect(move || {
            let width_key = if expanded() {
                EXPANDED_W.round() as i32
            } else {
                collapsed_width.round() as i32
            };
            let size_key = island_size();
            let expanded_key = expanded();
            let next = (expanded_key, width_key, size_key);
            if *cache.borrow() != Some(next) {
                *cache.borrow_mut() = Some(next);
                set_island_window(
                    &desktop,
                    expanded_key,
                    size_key as f64 / 100.0,
                    collapsed_width,
                );
            }
        });
    }

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
                class: "island",
                onmousedown: {
                    let desktop = desktop.clone();
                    move |event| {
                        if event.modifiers().shift()
                            && event
                                .trigger_button()
                                .is_some_and(|button| button == MouseButton::Primary)
                        {
                            desktop.drag();
                        }
                    }
                },
                div { class: "{core_class}",
                    if has_music {
                        div { class: "{cover_class}",
                            div { class: "cover-art", style: "{cover_style}" }
                        }
                        div { class: "music-copy",
                            div { class: "lyric-viewport",
                                if let Some(outgoing_text) = outgoing_primary_text {
                                    strong {
                                        class: "{primary_class} lyric-layer lyric-out",
                                        key: "out-{transition_key}",
                                        "{outgoing_text}"
                                    }
                                }
                                strong {
                                    class: "{primary_class} lyric-layer lyric-in",
                                    key: "in-{transition_key}-{visible_primary_text}",
                                    "{visible_primary_text}"
                                }
                            }
                        }
                    } else {
                        div { class: "speed-copy idle-speeds",
                            div { class: "speed-stat",
                                strong { "{download}" }
                                span { "DOWN" }
                            }
                            div { class: "speed-stat",
                                strong { "{upload}" }
                                span { "UP" }
                            }
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
                            if state.is_playing {
                                "Ⅱ"
                            } else {
                                "▶"
                            }
                        }
                        button { onclick: play_next, title: "Next", "⏭" }
                        button {
                            onclick: {
                                let player = player.clone();
                                move |_| {
                                    current_index.set(None);
                                    lyrics.set(Vec::new());
                                    player.send(AudioCommand::Stop);
                                    status.set("Stopped.".to_string());
                                }
                            },
                            title: "Stop",
                            "■"
                        }
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
                                placeholder: "Search NetEase",
                                onfocus: move |_| input_focused.set(true),
                                onblur: {
                                    let desktop = desktop.clone();
                                    move |_| {
                                        input_focused.set(false);
                                        if !*pointer_inside.read() {
                                            expanded.set(false);
                                            set_island_window(&desktop, false, island_size() as f64 / 100.0, collapsed_width);
                                        }
                                    }
                                },
                                oninput: move |event| query.set(event.value()),
                                onkeydown: move |event| {
                                    if event.key() == Key::Enter && !event.is_composing() {
                                        spawn_search(query.read().trim().to_string(), results, status);
                                    }
                                },
                            }
                            button {
                                class: "icon-button",
                                onclick: move |_| spawn_search(query.read().trim().to_string(), results, status),
                                "⌕"
                            }
                        }
                        div { class: "random-row",
                            button { onclick: move |_| load_random(random_count()),
                                "Random {random_count}"
                            }
                            button { onclick: move |_| load_random(50), "50" }
                            button { onclick: move |_| load_random(100), "100" }
                        }
                        div { class: "random-control",
                            span { "Random N" }
                            input {
                                r#type: "range",
                                min: "1",
                                max: "100",
                                value: "{random_count}",
                                oninput: move |event| {
                                    if let Ok(value) = event.value().parse::<u32>() {
                                        random_count.set(value.clamp(1, 100));
                                    }
                                },
                            }
                            input {
                                class: "number-input",
                                r#type: "number",
                                min: "1",
                                max: "100",
                                value: "{random_count}",
                                oninput: move |event| {
                                    if let Ok(value) = event.value().parse::<u32>() {
                                        random_count.set(value.clamp(1, 100));
                                    }
                                },
                            }
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
                                    },
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
                            for (index , track) in queue.read().iter().cloned().enumerate() {
                                QueueTrackRow {
                                    track,
                                    active: current_index.read().is_some_and(|i| i == index),
                                    onplay: {
                                        let player = player.clone();
                                        move |_| {
                                            let list = queue.read().clone();
                                            if let Some(track) = list.get(index).cloned() {
                                                current_index.set(Some(index));
                                                spawn_play(track, player.clone(), status, lyrics);
                                            }
                                        }
                                    },
                                    onremove: {
                                        let player = player.clone();
                                        move |_| {
                                            let mut list = queue.read().clone();
                                            if index < list.len() {
                                                list.remove(index);
                                                queue.set(list);
                                                let current = *current_index.read();
                                                match current {
                                                    Some(i) if i == index => {
                                                        current_index.set(None);
                                                        lyrics.set(Vec::new());
                                                        player.send(AudioCommand::Stop);
                                                        status.set("Removed current track.".to_string());
                                                    }
                                                    Some(i) if i > index => current_index.set(Some(i - 1)),
                                                    _ => {}
                                                }
                                            }
                                        }
                                    },
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
                            div {
                                span { "Upload" }
                                strong { "{upload}" }
                            }
                            div {
                                span { "Download" }
                                strong { "{download}" }
                            }
                        }
                        div { class: "stat-line",
                            span { "Total up" }
                            strong { "{format_bytes(*total_upload.read())}" }
                        }
                        div { class: "stat-line",
                            span { "Total down" }
                            strong { "{format_bytes(*total_download.read())}" }
                        }
                        div { class: "stat-line",
                            span { "This month" }
                            strong { "{format_bytes(total_upload() + total_download())}" }
                        }
                        div { class: "status-text", "{status}" }
                    }
                } else {
                    div { class: "panel-section settings",
                        label { class: "setting",
                            span { "Opacity" }
                            div { class: "setting-control",
                                input {
                                    r#type: "range",
                                    min: "10",
                                    max: "100",
                                    value: "{opacity}",
                                    oninput: move |event| {
                                        if let Ok(value) = event.value().parse::<u32>() {
                                            opacity.set(value.clamp(10, 100));
                                        }
                                    },
                                }
                                output { "{opacity}%" }
                            }
                        }
                        label { class: "setting",
                            span { "Volume" }
                            div { class: "setting-control",
                                input {
                                    r#type: "range",
                                    min: "0",
                                    max: "100",
                                    value: "{volume}",
                                    oninput: {
                                        let player = player.clone();
                                        move |event| {
                                            if let Ok(value) = event.value().parse::<u32>() {
                                                let value = value.clamp(0, 100);
                                                volume.set(value);
                                                player.send(AudioCommand::SetVolume(value as f32 / 100.0));
                                            }
                                        }
                                    },
                                }
                                output { "{volume}%" }
                            }
                        }
                        label { class: "setting",
                            span { "Island size" }
                            div { class: "setting-control",
                                input {
                                    r#type: "range",
                                    min: "85",
                                    max: "135",
                                    value: "{island_size}",
                                    oninput: {
                                        let desktop = desktop.clone();
                                        move |event| {
                                            if let Ok(value) = event.value().parse::<u32>() {
                                                let value = value.clamp(85, 135);
                                                island_size.set(value);
                                                set_island_window(&desktop, expanded(), value as f64 / 100.0, collapsed_width);
                                            }
                                        }
                                    },
                                }
                                output { "{island_size}%" }
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
                        div { class: "status-text", "Right-click the island to exit CAPS." }
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

#[component]
fn QueueTrackRow(
    track: Track,
    active: bool,
    onplay: EventHandler<MouseEvent>,
    onremove: EventHandler<MouseEvent>,
) -> Element {
    let cover_style = if track.cover.is_empty() {
        String::new()
    } else {
        format!("background-image: url('{}');", track.cover)
    };
    rsx! {
        div { class: if active { "song queue-song active" } else { "song queue-song" },
            button { class: "queue-main", onclick: onplay,
                span { class: "song-cover", style: "{cover_style}" }
                span { class: "song-copy",
                    strong { "{track.name}" }
                    small { "{track.artist}" }
                }
            }
            button { class: "remove-song", onclick: onremove, title: "Remove", "×" }
        }
    }
}

fn spawn_play(
    track: Track,
    player: Arc<AudioPlayer>,
    mut status: Signal<String>,
    mut lyrics: Signal<Vec<LyricLine>>,
) {
    spawn(async move {
        status.set(format!("Loading {}...", track.name));
        lyrics.set(Vec::new());
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
                    if let Ok(response) = netease::get_netease_lyric(track.id.clone(), None).await {
                        lyrics.set(parse_lrc(response.lyric.as_deref().unwrap_or_default()));
                    }
                }
                Err(err) => status.set(format!("Stream read failed: {err}")),
            },
            Err(err) => status.set(format!("Stream request failed: {err}")),
        }
    });
}

fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(after_open) = rest.strip_prefix('[') {
            let Some((stamp, after_stamp)) = after_open.split_once(']') else {
                break;
            };
            if let Some(time) = parse_lrc_time(stamp) {
                times.push(time);
            }
            rest = after_stamp.trim_start();
        }
        let lyric = rest.trim();
        if lyric.is_empty() {
            continue;
        }
        for time in times {
            lines.push(LyricLine {
                time,
                text: lyric.to_string(),
            });
        }
    }
    lines.sort_by(|a, b| a.time.total_cmp(&b.time));
    lines
}

fn parse_lrc_time(text: &str) -> Option<f64> {
    let (minutes, seconds) = text.split_once(':')?;
    let minutes = minutes.parse::<f64>().ok()?;
    let seconds = seconds.parse::<f64>().ok()?;
    Some(minutes * 60.0 + seconds)
}

fn current_lyric_line(lines: &[LyricLine], position: f64) -> Option<String> {
    let target = position + 0.55;
    lines
        .iter()
        .take_while(|line| line.time <= target)
        .last()
        .map(|line| line.text.clone())
}

fn collapsed_width_for_text(_text: &str, has_music: bool) -> f64 {
    if has_music {
        MUSIC_COLLAPSED_W
    } else {
        COLLAPSED_W
    }
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

fn set_island_window(
    desktop: &DesktopContext,
    expanded: bool,
    size_scale: f64,
    collapsed_width: f64,
) {
    let size_scale = size_scale.clamp(0.85, 1.35);
    let (base_width, base_height) = if expanded {
        (EXPANDED_W, EXPANDED_H)
    } else {
        (
            collapsed_width.max(COLLAPSED_W) + ISLAND_BLEED * 2.0,
            COLLAPSED_H + ISLAND_BLEED * 2.0,
        )
    };
    let width = base_width * size_scale;
    let height = base_height * size_scale;
    let old_size = desktop.inner_size();
    let old_position = desktop.outer_position().ok();
    let scale = desktop.scale_factor();
    desktop.set_inner_size(LogicalSize::new(width, height));
    desktop.set_always_on_top(true);
    if let Some(position) = old_position {
        let old_width = old_size.width as i32;
        let new_width = (width * scale).round() as i32;
        let x = position.x + (old_width - new_width) / 2;
        desktop.set_outer_position(PhysicalPosition::new(x, position.y));
    }
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
  width: var(--stage-width);
  height: var(--stage-height);
  padding: var(--island-bleed);
  zoom: var(--island-scale);
  color: rgba(248, 255, 252, 0.96);
  user-select: none;
  -webkit-font-smoothing: antialiased;
  overflow: visible;
  background: transparent;
}

.stage.expanded {
  width: 430px;
  height: 490px;
  padding: 0;
}

.island {
  position: relative;
  width: var(--collapsed-width);
  height: 56px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto 38px;
  align-items: center;
  gap: 10px;
  padding: 7px 10px 8px 7px;
  border-radius: 999px;
  background: rgba(5, 8, 9, var(--island-bg-alpha));
  border: 1px solid rgba(255, 255, 255, 0.06);
  box-shadow: inset 0 1px rgba(255, 255, 255, 0.08);
  backdrop-filter: blur(22px) saturate(1.25);
  overflow: hidden;
}

.expanded .island {
  width: 430px;
  height: 86px;
  grid-template-columns: minmax(0, 1fr) 142px 40px;
  padding: 10px 13px 13px 10px;
  border-radius: 34px;
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
  grid-column: 1;
  min-width: 0;
  height: 100%;
  display: grid;
  grid-template-columns: 42px minmax(0, 1fr);
  align-items: center;
  gap: 12px;
}

.expanded .core {
  grid-template-columns: 52px minmax(0, 1fr);
  gap: 14px;
}

.idle-core {
  grid-template-columns: minmax(0, 1fr);
  gap: 0;
}

.cover {
  width: 42px;
  height: 42px;
  border-radius: 50%;
  background: linear-gradient(135deg, #14352f, #d8b45b);
  position: relative;
  display: grid;
  place-items: center;
  overflow: hidden;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.16);
}

.expanded .cover {
  width: 52px;
  height: 52px;
  border-radius: 50%;
}

.cover-art {
  position: absolute;
  inset: 0;
  border-radius: inherit;
  background-position: center;
  background-size: cover;
  will-change: transform;
}

.cover.playing .cover-art {
  animation: albumSpin 9s linear infinite;
}

.music-copy,
.speed-copy {
  min-width: 0;
  display: grid;
  gap: 2px;
}

.idle-speeds {
  height: 42px;
  display: flex;
  align-items: center;
  gap: 12px;
}

.speed-stat {
  min-width: 0;
  display: grid;
  gap: 2px;
}

.music-copy {
  align-self: center;
  display: flex;
  align-items: center;
  height: 42px;
  overflow: hidden;
}

.expanded .music-copy {
  height: 52px;
}

.lyric-viewport {
  position: relative;
  min-width: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
  mask-image: linear-gradient(90deg, #000 0, #000 calc(100% - 22px), transparent 100%);
}

.expanded .lyric-viewport {
  height: 100%;
}

.music-copy strong,
.speed-copy strong {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  font-size: 12.5px;
  max-width: 100%;
  line-height: 1.05;
  font-weight: 760;
  letter-spacing: 0;
}

.music-copy .lyric-layer {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  min-width: 0;
  max-width: 100%;
  pointer-events: none;
  transform-origin: center;
  line-height: 1;
}

.expanded .music-copy strong,
.expanded .speed-copy strong {
  font-size: 16px;
}

.music-copy span,
.speed-copy span {
  display: block;
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

.idle-speeds span {
  color: rgba(248, 255, 252, 0.7);
}

.lyric-line {
  color: rgba(248, 255, 252, 0.74);
  clip-path: inset(0 0 0 0);
  will-change: opacity, filter, transform, clip-path;
}

.lyric-title {
  color: rgba(248, 255, 252, 0.98);
  font-size: 15px !important;
  clip-path: inset(0 0 0 0);
  will-change: opacity, filter, transform, clip-path;
}

.lyric-in {
  animation: lyricWipeIn 620ms cubic-bezier(0.2, 0, 0, 1) both;
}

.lyric-out {
  z-index: 1;
  animation: lyricWipeOut 520ms cubic-bezier(0.36, 0, 0.2, 1) both;
}

.expanded .lyric-title {
  font-size: 19px !important;
}

.mini-controls {
  grid-column: 2;
  justify-self: end;
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 5px;
}

.mini-controls button {
  width: 31px;
  height: 31px;
  border-radius: 50%;
  background: rgba(255, 255, 255, var(--softer-alpha));
  color: rgba(248, 255, 252, 0.94);
  font-size: 11px;
  font-weight: 900;
  transition: transform 80ms ease-out, background-color 160ms ease;
}

.mini-controls button:hover {
  background: rgba(255, 255, 255, var(--soft-alpha));
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
  grid-column: 3;
  justify-self: end;
  width: 38px;
  height: 34px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 2px;
}

.spectrum i {
  width: 4px;
  height: 24px;
  min-height: 6px;
  border-radius: 999px;
  background: linear-gradient(180deg, #fff0a0, #78f2ca);
  transform-origin: center;
  transition: transform 110ms cubic-bezier(0.16, 1.28, 0.28, 1);
  will-change: transform;
}

.expanded .spectrum i {
  height: 30px;
}

.playback-progress {
  position: absolute;
  left: 18px;
  right: 18px;
  bottom: 8px;
  height: 3px;
  border-radius: 999px;
  background: rgba(255, 255, 255, var(--softer-alpha));
  overflow: hidden;
}

.playback-progress span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, #78f2ca, #fff0a0);
}

.panel {
  width: 430px;
  height: 396px;
  margin-top: 8px;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 9px;
  border-radius: 22px;
  background: rgba(19, 24, 26, var(--panel-bg-alpha));
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
  background: rgba(142, 142, 147, var(--hover-alpha));
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
  background: rgba(255, 255, 255, var(--soft-alpha));
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
  background: rgba(142, 142, 147, var(--soft-alpha));
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
  background: rgba(48, 209, 88, var(--green-alpha));
  color: #7df2ca;
  font-size: 18px;
  font-weight: 800;
}

.random-row,
.queue-toolbar,
.random-control {
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
  background: rgba(142, 142, 147, var(--active-alpha));
  color: rgba(248, 255, 252, 0.88);
  font-size: 12px;
  font-weight: 800;
  transition: background-color 160ms ease, transform 80ms ease;
}

.random-control {
  min-height: 34px;
  padding: 0 2px;
}

.random-control span {
  color: rgba(248, 255, 252, 0.62);
  font-size: 12px;
  font-weight: 800;
  white-space: nowrap;
}

.random-control input[type="range"] {
  flex: 1;
  min-width: 0;
  accent-color: #34c759;
}

.number-input {
  width: 58px;
  height: 28px;
  border-radius: 8px;
  background: rgba(142, 142, 147, var(--soft-alpha));
  color: rgba(248, 255, 252, 0.92);
  text-align: center;
  font-size: 12px;
  font-weight: 800;
  outline: none;
}

.random-row button:hover,
.queue-toolbar button:hover,
.song:hover,
.segmented button:hover {
  background: rgba(255, 255, 255, var(--hover-alpha));
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

.queue-song {
  grid-template-columns: 1fr 30px;
  padding: 4px 5px 4px 7px;
}

.queue-main {
  min-width: 0;
  min-height: 34px;
  display: grid;
  grid-template-columns: 32px 1fr;
  align-items: center;
  gap: 9px;
  background: transparent;
  text-align: left;
}

.remove-song {
  width: 26px;
  height: 26px;
  border-radius: 50%;
  background: rgba(255, 255, 255, var(--softer-alpha));
  color: rgba(248, 255, 252, 0.62);
  font-size: 18px;
  line-height: 1;
  transition: background-color 160ms ease, color 160ms ease, transform 80ms ease;
}

.remove-song:hover {
  background: rgba(255, 69, 58, var(--red-alpha));
  color: #ff9b94;
}

.song.active {
  background: rgba(48, 209, 88, var(--active-alpha));
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
  background: rgba(142, 142, 147, var(--hover-alpha));
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
  display: grid;
  grid-template-columns: 100px minmax(0, 1fr);
  align-items: center;
  gap: 12px;
  padding: 0 2px;
  color: rgba(248, 255, 252, 0.84);
  font-size: 12.5px;
  font-weight: 780;
}

.setting-control {
  min-width: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 46px;
  align-items: center;
  gap: 10px;
}

.setting output {
  width: 46px;
  text-align: right;
  color: rgba(248, 255, 252, 0.62);
  font-size: 12px;
  font-weight: 800;
}

.setting input[type="range"] {
  width: 100%;
  min-width: 0;
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
  background: rgba(142, 142, 147, var(--soft-alpha));
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
  background: rgba(255, 255, 255, var(--active-alpha));
  color: rgba(248, 255, 252, 0.96);
}

.status-text {
  margin-top: auto;
  color: rgba(248, 255, 252, 0.54);
  font-size: 12px;
  font-weight: 650;
  line-height: 1.36;
}

@keyframes albumSpin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

@keyframes lyricWipeIn {
  from {
    opacity: 0;
    filter: blur(6px);
    clip-path: inset(0 0 0 100%);
    transform: translate3d(18px, 0, 0);
  }
  42% {
    opacity: 0.82;
    filter: blur(2.5px);
    clip-path: inset(0 0 0 32%);
    transform: translate3d(6px, 0, 0);
  }
  to {
    opacity: 1;
    filter: blur(0);
    clip-path: inset(0 0 0 0);
    transform: translate3d(0, 0, 0);
  }
}

@keyframes lyricWipeOut {
  from {
    opacity: 1;
    filter: blur(0);
    clip-path: inset(0 0 0 0);
    transform: translate3d(0, 0, 0);
  }
  38% {
    opacity: 0.58;
    filter: blur(2px);
    clip-path: inset(0 42% 0 0);
    transform: translate3d(-7px, 0, 0);
  }
  to {
    opacity: 0;
    filter: blur(6px);
    clip-path: inset(0 100% 0 0);
    transform: translate3d(-18px, 0, 0);
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
