#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod audio;
mod audio_spectrum;
mod components;
mod formatting;
mod lyrics;
mod netease;
mod storage;
mod track;
mod windowing;

use actions::{spawn_play, spawn_random_queue, spawn_search};
use audio::{AudioCommand, AudioPlayer};
use components::{Island, QueueTrackRow, StatsPanel, Tabs, TrackRow};
use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use formatting::{format_bytes, format_rate};
use lyrics::{LyricLine, current_lyric_line};
use std::sync::Arc;
use storage::{AppSettings, AppState};
use sysinfo::Networks;
use track::Track;
use windowing::{
    COLLAPSED_H, COLLAPSED_W, EXPANDED_W, ISLAND_BLEED, MUSIC_COLLAPSED_W, place_top_center,
    set_island_window,
};

#[cfg(target_os = "windows")]
use dioxus::desktop::tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};

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
    let saved_state = use_hook(storage::load_state);
    let saved_settings = saved_state.settings.clone();
    let player = use_hook(|| Arc::new(AudioPlayer::spawn()));
    let mut expanded = use_signal(|| false);
    let mut pointer_inside = use_signal(|| false);
    let mut input_focused = use_signal(|| false);
    let mut active_tab = use_signal({
        let saved_settings = saved_settings.clone();
        move || saved_settings.active_tab.clone()
    });
    let mut opacity = use_signal({
        let saved_settings = saved_settings.clone();
        move || saved_settings.opacity
    });
    let mut spring_style = use_signal({
        let saved_settings = saved_settings.clone();
        move || saved_settings.spring_style.clone()
    });
    let mut volume = use_signal({
        let saved_settings = saved_settings.clone();
        move || saved_settings.volume
    });
    let mut island_size = use_signal({
        let saved_settings = saved_settings.clone();
        move || saved_settings.island_size
    });
    let mut random_count = use_signal(move || saved_settings.random_count);
    let mut query = use_signal(String::new);
    let results = use_signal(Vec::<Track>::new);
    let mut queue = use_signal({
        let saved_state = saved_state.clone();
        move || saved_state.queue.clone()
    });
    let mut current_index = use_signal(move || saved_state.current_index);
    let mut status =
        use_signal(|| "Search NetEase, add songs, then play from the island.".to_string());
    let mut audio_state = use_signal(|| player.get_state());
    let mut spectrum = use_signal(audio_spectrum::get_audio_spectrum);
    let mut upload = use_signal(|| "0 B/s".to_string());
    let mut download = use_signal(|| "0 B/s".to_string());
    let mut total_upload = use_signal(|| 0_u64);
    let mut total_download = use_signal(|| 0_u64);
    let mut lyrics = use_signal(Vec::<LyricLine>::new);

    {
        let player = player.clone();
        use_effect(move || {
            player.send(AudioCommand::SetVolume(volume() as f32 / 100.0));
        });
    }

    use_effect(move || {
        storage::save_state(&AppState {
            settings: AppSettings {
                opacity: opacity(),
                volume: volume(),
                island_size: island_size(),
                random_count: random_count(),
                spring_style: spring_style.read().clone(),
                active_tab: active_tab.read().clone(),
            },
            queue: queue.read().clone(),
            current_index: *current_index.read(),
        });
    });

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
                networks.refresh(true);
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

    let load_random = move |count: u32| spawn_random_queue(count, queue, current_index, status);

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
            spawn_play(
                track,
                player_for_next.clone(),
                current_index,
                status,
                lyrics,
            );
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
            spawn_play(
                track,
                player_for_prev.clone(),
                current_index,
                status,
                lyrics,
            );
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
    let island_class = if has_music {
        "island"
    } else {
        "island idle-island"
    };
    let core_class = if has_music { "core" } else { "core idle-core" };
    let cover_class = if state.is_playing {
        "cover playing"
    } else {
        "cover"
    };
    let status_text = status.read().clone();
    let activity = if status_text.starts_with("Searching") {
        "searching"
    } else if status_text.starts_with("Loading") {
        "working"
    } else {
        ""
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
            Island {
                island_class,
                core_class,
                cover_class,
                cover_style,
                has_music,
                is_expanded,
                primary_class,
                visible_primary_text,
                outgoing_primary_text,
                transition_key,
                download: download.read().clone(),
                upload: upload.read().clone(),
                spectrum: *spectrum.read(),
                activity: activity.to_string(),
                progress,
                is_playing: state.is_playing,
                ondrag: {
                    let desktop = desktop.clone();
                    move |event: MouseEvent| {
                        if event.modifiers().shift()
                            && event
                                .trigger_button()
                                .is_some_and(|button| button == MouseButton::Primary)
                        {
                            desktop.drag();
                        }
                    }
                },
                onprev: play_prev,
                onplaypause: {
                    let player = player.clone();
                    move |_| player.send(AudioCommand::PlayPause)
                },
                onnext: play_next,
                onstop: {
                    let player = player.clone();
                    move |_| {
                        current_index.set(None);
                        lyrics.set(Vec::new());
                        player.send(AudioCommand::Stop);
                        status.set("Stopped.".to_string());
                    }
                }
            }

            section { class: "panel",
                Tabs {
                    tab: tab.clone(),
                    onsearch: move |_| active_tab.set("search".to_string()),
                    onqueue: move |_| active_tab.set("queue".to_string()),
                    onstats: move |_| active_tab.set("stats".to_string()),
                    onsettings: move |_| active_tab.set("settings".to_string()),
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
                                            set_island_window(
                                                &desktop,
                                                false,
                                                island_size() as f64 / 100.0,
                                                collapsed_width,
                                            );
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
                                                spawn_play(track, player.clone(), current_index, status, lyrics);
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
                    StatsPanel {
                        upload: upload.read().clone(),
                        download: download.read().clone(),
                        total_upload: format_bytes(*total_upload.read()),
                        total_download: format_bytes(*total_download.read()),
                        month_total: format_bytes(total_upload() + total_download()),
                        status: status.read().clone(),
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
                                                set_island_window(
                                                    &desktop,
                                                    expanded(),
                                                    value as f64 / 100.0,
                                                    collapsed_width,
                                                );
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

fn collapsed_width_for_text(_text: &str, has_music: bool) -> f64 {
    if has_music {
        MUSIC_COLLAPSED_W
    } else {
        COLLAPSED_W
    }
}

const APP_CSS: &str = include_str!("app.css");
