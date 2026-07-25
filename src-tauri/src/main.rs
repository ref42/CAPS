#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod album_color;
mod audio;
mod audio_spectrum;
mod bilibili;
mod components;
mod download;
mod formatting;
mod icon;
mod local_music;
mod lyrics;
mod mode;
mod shitease;
mod storage;
mod track;
mod updater;
mod windowing;
mod youtube;

use actions::{
    RandomQueueMode, VideoImportSource, append_unique_tracks, spawn_import_video_url,
    spawn_load_local_queue, spawn_play, spawn_random_queue, spawn_search,
};
use audio::{AudioCommand, AudioPlayer};
use components::{
    Island, QUEUE_RENDER_LIMIT, QueuePanel, SearchPanel, SearchSource, SettingsPanel, StatsPanel,
    Tabs,
};
use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use formatting::{format_bytes, format_rate};
use lyrics::{LyricLine, current_lyric_line};
use mode::MusicMode;
use std::sync::Arc;
use storage::AppSettings;
use sysinfo::{MINIMUM_CPU_UPDATE_INTERVAL, Networks, System};
use track::Track;
use windowing::{
    COLLAPSED_H, COLLAPSED_W, EXPANDED_H, EXPANDED_W, ISLAND_BLEED, MUSIC_COLLAPSED_W,
    place_top_center, set_island_window,
};

#[cfg(target_os = "windows")]
use dioxus::desktop::tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};

#[derive(Clone)]
struct LyricTransition {
    current: String,
    outgoing: Option<String>,
    id: u64,
}

const DEFAULT_SPECTRUM_FROM: &str = "rgb(255, 196, 224)";
const DEFAULT_SPECTRUM_TO: &str = "rgb(255, 105, 180)";
const WINDOW_COLLAPSE_DELAY_MS: u64 = 280;

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

    let mut config = Config::new().with_window(window);
    if let Some(icon) = icon::app_icon() {
        config = config.with_icon(icon);
    }

    config
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
    let mut window_expanded = use_signal(|| false);
    let mut transition_ticket = use_signal(|| 0u64);
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
    let mut volume = use_signal({
        let saved_settings = saved_settings.clone();
        move || saved_settings.volume
    });
    let mut island_size = use_signal({
        let saved_settings = saved_settings.clone();
        move || saved_settings.island_size
    });
    let mut random_count = use_signal({
        let saved_settings = saved_settings.clone();
        move || saved_settings.random_count
    });
    let mut local_music_folder = use_signal({
        let saved_settings = saved_settings.clone();
        move || saved_settings.local_music_folder.clone()
    });
    let mut search_source = use_signal(|| SearchSource::Netease);
    let mut query = use_signal(String::new);
    let mut video_url = use_signal(String::new);
    let results = use_signal(Vec::<Track>::new);
    let mut queue = use_signal({
        let saved_state = saved_state.clone();
        move || saved_state.queue.clone()
    });
    let mut current_index = use_signal(move || saved_state.current_index);
    let mut current_track = use_signal(|| None::<Track>);
    let mut status =
        use_signal(|| "Search music, add songs, then play from the island.".to_string());
    let mut music_mode = use_signal(|| MusicMode::Silent);
    let mut audio_state = use_signal(|| player.get_state());
    let mut spectrum = use_signal(audio_spectrum::get_audio_spectrum);
    let mut spectrum_colors = use_signal(|| {
        (
            DEFAULT_SPECTRUM_FROM.to_string(),
            DEFAULT_SPECTRUM_TO.to_string(),
        )
    });
    let mut upload = use_signal(|| "0 B/s".to_string());
    let mut download = use_signal(|| "0 B/s".to_string());
    let mut cpu_usage = use_signal(|| "0%".to_string());
    let mut memory_usage = use_signal(|| "0%".to_string());
    let mut cpu_progress = use_signal(|| 0.0_f64);
    let mut memory_progress = use_signal(|| 0.0_f64);
    let mut upload_progress = use_signal(|| 0.0_f64);
    let mut download_progress = use_signal(|| 0.0_f64);
    let mut activity_energy = use_signal(|| 0.0_f64);
    let mut paused_since = use_signal(|| None::<std::time::Instant>);
    let mut total_upload = use_signal(|| 0_u64);
    let mut total_download = use_signal(|| 0_u64);
    let mut lyrics = use_signal(Vec::<LyricLine>::new);
    let mut pending_update = use_signal(|| None::<updater::ReleaseUpdate>);
    let mut update_busy = use_signal(|| false);
    let mut update_progress = use_signal(|| None::<f64>);
    let mut update_status =
        use_signal(|| format!("Installed CAPS {}.", updater::current_version()));

    {
        let player = player.clone();
        use_effect(move || {
            player.send(AudioCommand::SetVolume(volume() as f32 / 100.0));
        });
    }

    use_effect(move || {
        storage::save_state_parts(
            AppSettings {
                opacity: opacity(),
                volume: volume(),
                island_size: island_size(),
                random_count: random_count(),
                active_tab: active_tab.read().clone(),
                local_music_folder: local_music_folder.read().clone(),
            },
            &queue.read(),
            *current_index.read(),
        );
    });

    let player_for_state = player.clone();
    use_effect(move || {
        let player = player_for_state.clone();
        spawn(async move {
            let mut smoothed_energy = 0.0_f64;
            loop {
                let next_state = player.get_state();
                let next_spectrum = audio_spectrum::get_audio_spectrum();
                let raw_energy = if next_state.is_playing {
                    activity_energy_from_spectrum(&next_spectrum)
                } else {
                    0.0
                };
                let blend = if raw_energy > smoothed_energy {
                    0.34
                } else {
                    0.16
                };
                smoothed_energy += (raw_energy - smoothed_energy) * blend;
                if !next_state.is_playing && smoothed_energy < 0.01 {
                    smoothed_energy = 0.0;
                }
                if next_state.is_playing || next_state.is_finished || next_state.title.is_empty() {
                    paused_since.set(None);
                } else if paused_since.read().is_none() {
                    paused_since.set(Some(std::time::Instant::now()));
                }
                audio_state.set(next_state);
                spectrum.set(next_spectrum);
                activity_energy.set(smoothed_energy);
                tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            }
        });
    });

    use_effect(move || {
        spawn(async move {
            let mut networks = Networks::new_with_refreshed_list();
            let mut system = System::new_all();
            system.refresh_cpu_usage();
            tokio::time::sleep(MINIMUM_CPU_UPDATE_INTERVAL).await;
            let mut last_rx = 0_u64;
            let mut last_tx = 0_u64;
            let mut rx_peak = 1.0_f64;
            let mut tx_peak = 1.0_f64;
            loop {
                networks.refresh(true);
                system.refresh_cpu_usage();
                system.refresh_memory();
                let mut rx = 0_u64;
                let mut tx = 0_u64;
                for (_, data) in networks.iter() {
                    rx += data.total_received();
                    tx += data.total_transmitted();
                }
                let rx_delta = rx.saturating_sub(last_rx);
                let tx_delta = tx.saturating_sub(last_tx);
                if last_rx != 0 || last_tx != 0 {
                    download.set(format_rate(rx_delta));
                    upload.set(format_rate(tx_delta));
                    rx_peak = (rx_peak * 0.94).max(rx_delta as f64).max(1.0);
                    tx_peak = (tx_peak * 0.94).max(tx_delta as f64).max(1.0);
                    download_progress.set(rate_progress(rx_delta, rx_peak));
                    upload_progress.set(rate_progress(tx_delta, tx_peak));
                }
                let next_cpu = system.global_cpu_usage() as f64;
                let next_memory = memory_percent_value(
                    system
                        .total_memory()
                        .saturating_sub(system.available_memory()),
                    system.total_memory(),
                );
                cpu_usage.set(format_percent(next_cpu));
                memory_usage.set(format_percent(next_memory));
                cpu_progress.set(next_cpu.clamp(0.0, 100.0));
                memory_progress.set(next_memory);
                total_download.set(rx);
                total_upload.set(tx);
                last_rx = rx;
                last_tx = tx;
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });
    });

    let last_cover_for_color = use_hook(|| std::cell::RefCell::new(String::new()));
    use_effect(move || {
        let cover_url = current_index
            .read()
            .and_then(|index| queue.read().get(index).map(|track| track.cover.clone()))
            .unwrap_or_default();
        if *last_cover_for_color.borrow() == cover_url {
            return;
        }
        *last_cover_for_color.borrow_mut() = cover_url.clone();
        if cover_url.is_empty() {
            spectrum_colors.set((
                DEFAULT_SPECTRUM_FROM.to_string(),
                DEFAULT_SPECTRUM_TO.to_string(),
            ));
            return;
        }
        spawn(async move {
            let colors = album_color::spectrum_colors(cover_url)
                .await
                .unwrap_or_else(|| {
                    (
                        DEFAULT_SPECTRUM_FROM.to_string(),
                        DEFAULT_SPECTRUM_TO.to_string(),
                    )
                });
            spectrum_colors.set(colors);
        });
    });

    let mut expand_window = move || {
        transition_ticket.set(transition_ticket().wrapping_add(1));
        window_expanded.set(true);
        expanded.set(true);
    };

    let mut collapse_window = move || {
        if !*input_focused.read() {
            expanded.set(false);
            schedule_window_collapse(
                window_expanded,
                pointer_inside,
                input_focused,
                transition_ticket,
            );
        }
    };

    let player_for_random_append = player.clone();
    let load_random_append = move |count: u32| {
        spawn_random_queue(
            count,
            RandomQueueMode::Append,
            queue,
            current_index,
            current_track,
            music_mode,
            player_for_random_append.clone(),
            status,
            lyrics,
        )
    };
    let player_for_random_replace = player.clone();
    let load_random_replace = move |count: u32| {
        spawn_random_queue(
            count,
            RandomQueueMode::Replace,
            queue,
            current_index,
            current_track,
            music_mode,
            player_for_random_replace.clone(),
            status,
            lyrics,
        )
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
            music_mode.set(MusicMode::Normal);
            spawn_play(
                track,
                player_for_next.clone(),
                current_index,
                current_track,
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
            music_mode.set(MusicMode::Normal);
            spawn_play(
                track,
                player_for_prev.clone(),
                current_index,
                current_track,
                status,
                lyrics,
            );
        }
    };

    let player_for_finish = player.clone();
    use_effect(move || {
        let state = audio_state.read().clone();
        if !state.is_finished {
            return;
        }
        let Some(index) = *current_index.read() else {
            return;
        };
        let len = queue.read().len();
        if len == 0 {
            current_index.set(None);
            current_track.set(None);
            music_mode.set(MusicMode::Silent);
            lyrics.set(Vec::new());
            player_for_finish.send(AudioCommand::Stop);
            status.set("Queue finished.".to_string());
            return;
        }
        let finished_track_matches_state = queue
            .read()
            .get(index)
            .is_some_and(|track| track.name == state.title && track.artist == state.detail);
        if !finished_track_matches_state {
            return;
        }
        if index + 1 >= len {
            current_index.set(None);
            current_track.set(None);
            music_mode.set(MusicMode::Silent);
            lyrics.set(Vec::new());
            player_for_finish.send(AudioCommand::Stop);
            status.set("Queue finished.".to_string());
            return;
        }
        let next = index + 1;
        current_index.set(Some(next));
        if let Some(track) = queue.read().get(next).cloned() {
            music_mode.set(MusicMode::Normal);
            spawn_play(
                track,
                player_for_finish.clone(),
                current_index,
                current_track,
                status,
                lyrics,
            );
        }
    });

    let state = audio_state.read().clone();
    let indexed_track = current_index
        .read()
        .and_then(|index| queue.read().get(index).cloned());
    let active_track = current_track.read().clone().or(indexed_track);
    let is_finished_last = state.is_finished
        && current_index
            .read()
            .is_some_and(|index| index + 1 >= queue.read().len());
    let paused_over_idle_timeout = paused_since
        .read()
        .as_ref()
        .is_some_and(|since| since.elapsed() >= std::time::Duration::from_secs(30));
    let audio_has_track = !state.title.is_empty() && !state.is_finished;
    let has_music = (active_track.is_some() || audio_has_track)
        && !is_finished_last
        && !paused_over_idle_timeout
        && music_mode() == MusicMode::Normal;
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
    let lyric_units = text_visual_units(&primary_text);
    let lyric_scroll_class = if has_music && lyric_units > 36.0 {
        "lyric-wrap lyric-dense"
    } else if has_music && lyric_units > 18.0 {
        "lyric-wrap"
    } else {
        ""
    };
    let lyric_scroll_style = if has_music {
        let target_units = if lyric_units > 36.0 {
            42.0
        } else if lyric_units > 18.0 {
            28.0
        } else {
            14.2
        };
        let fit_floor = if lyric_units > 36.0 { 0.46 } else { 0.56 };
        let fit_ratio = if lyric_units > f64::EPSILON {
            (target_units / lyric_units).clamp(fit_floor, 1.0)
        } else {
            1.0
        };
        let lyric_font = 15.2 * fit_ratio;
        let expanded_lyric_font = 16.8 * fit_ratio;
        let plain_font = 14.4 * fit_ratio;
        let expanded_plain_font = 15.2 * fit_ratio;
        format!(
            "--lyric-font-size: {lyric_font:.2}px; --expanded-lyric-font-size: {expanded_lyric_font:.2}px; --plain-font-size: {plain_font:.2}px; --expanded-plain-font-size: {expanded_plain_font:.2}px;"
        )
    } else {
        String::new()
    };
    let cover_style = active_track
        .as_ref()
        .filter(|track| !track.cover.is_empty())
        .map(|track| format!("background-image: url('{}');", track.cover))
        .unwrap_or_default();
    let active_spectrum_colors = {
        let colors = spectrum_colors.read();
        (colors.0.clone(), colors.1.clone())
    };
    let spectrum_style = {
        let colors = &active_spectrum_colors;
        format!(
            "--spectrum-from: {}; --spectrum-to: {};",
            colors.0, colors.1
        )
    };
    let progress = if state.duration > 0.0 {
        (state.position / state.duration * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let progress_style = {
        let colors = &active_spectrum_colors;
        format!(
            "--progress: {progress:.2}%; --spectrum-from: {}; --spectrum-to: {};",
            colors.0, colors.1
        )
    };
    let tab = active_tab.read().clone();
    let (queue_len_for_panel, visible_queue_for_panel) = if tab == "queue" {
        let queue_ref = queue.read();
        (
            queue_ref.len(),
            queue_ref
                .iter()
                .take(QUEUE_RENDER_LIMIT)
                .cloned()
                .enumerate()
                .collect::<Vec<_>>(),
        )
    } else {
        (0, Vec::new())
    };
    let is_expanded = *expanded.read();
    let opacity_css = (*opacity.read() as f64 / 100.0).clamp(0.1, 1.0);
    let island_scale = (*island_size.read() as f64 / 100.0).clamp(0.85, 1.35);
    let collapsed_width = collapsed_width_for_text(&primary_text, has_music);
    let stage_width = collapsed_width + ISLAND_BLEED * 2.0;
    let stage_height = COLLAPSED_H + ISLAND_BLEED * 2.0;
    let expanded_stage_width = EXPANDED_W + ISLAND_BLEED * 2.0;
    let expanded_stage_height = EXPANDED_H + ISLAND_BLEED * 2.0;
    let island_alpha = (opacity_css * 0.92).clamp(0.08, 0.92);
    let panel_alpha = (opacity_css * 0.86).clamp(0.08, 0.86);
    let soft_alpha = (opacity_css * 0.16).clamp(0.02, 0.16);
    let softer_alpha = (opacity_css * 0.08).clamp(0.01, 0.08);
    let hover_alpha = (opacity_css * 0.15).clamp(0.02, 0.15);
    let active_alpha = (opacity_css * 0.18).clamp(0.02, 0.18);
    let green_alpha = (opacity_css * 0.2).clamp(0.03, 0.2);
    let red_alpha = (opacity_css * 0.22).clamp(0.03, 0.22);
    let stage_style = format!(
        "--island-bg-alpha: {island_alpha:.3}; --panel-bg-alpha: {panel_alpha:.3}; --soft-alpha: {soft_alpha:.3}; --softer-alpha: {softer_alpha:.3}; --hover-alpha: {hover_alpha:.3}; --active-alpha: {active_alpha:.3}; --green-alpha: {green_alpha:.3}; --red-alpha: {red_alpha:.3}; --island-scale: {island_scale:.2}; --collapsed-width: {collapsed_width:.0}px; --stage-width: {stage_width:.0}px; --stage-height: {stage_height:.0}px; --expanded-stage-width: {expanded_stage_width:.0}px; --expanded-stage-height: {expanded_stage_height:.0}px; --island-bleed: {ISLAND_BLEED:.0}px;"
    );
    let stage_class = match (*window_expanded.read(), is_expanded) {
        (true, true) => "stage window-expanded visual-expanded",
        (true, false) => "stage window-expanded",
        (false, _) => "stage",
    };
    let island_class = if has_music {
        "island"
    } else {
        "island idle-island"
    };
    let is_loading = status.read().starts_with("Loading")
        || status.read().starts_with("Searching")
        || status.read().starts_with("Importing")
        || status.read().starts_with("Downloading");
    let (activity_class, activity_title) = if is_loading {
        ("activity-dot loading", "Loading")
    } else if state.is_playing {
        ("activity-dot live", "Playing")
    } else if has_music {
        ("activity-dot paused", "Paused")
    } else {
        ("activity-dot idle", "Idle")
    };
    let activity_style = if state.is_playing {
        let energy = activity_energy().clamp(0.0, 1.0);
        let activity_scale = 0.92 + energy * 0.3;
        let activity_opacity = 0.58 + energy * 0.4;
        format!("--activity-scale: {activity_scale:.3}; --activity-opacity: {activity_opacity:.3};")
    } else {
        String::new()
    };
    let core_class = if has_music { "core" } else { "core idle-core" };
    let cover_class = if state.is_playing {
        "cover playing"
    } else {
        "cover"
    };
    let audio_ready_for_toggle = !state.title.is_empty() && !state.is_finished;
    let playpause_track = active_track.clone();
    let has_active_music = active_track.is_some() || audio_has_track;
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
            let window_expanded_key = window_expanded();
            let width_key = if window_expanded_key {
                EXPANDED_W.round() as i32
            } else {
                collapsed_width.round() as i32
            };
            let size_key = island_size();
            let next = (window_expanded_key, width_key, size_key);
            if *cache.borrow() != Some(next) {
                *cache.borrow_mut() = Some(next);
                set_island_window(
                    &desktop,
                    window_expanded_key,
                    size_key as f64 / 100.0,
                    collapsed_width,
                );
            }
        });
    }

    rsx! {
        style { "{APP_CSS}" }
        main {
            class: "{stage_class}",
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
                move |_| {
                    let _ = storage::clean_song_cache();
                    desktop.close()
                }
            },
            Island {
                island_class,
                activity_class,
                activity_title,
                activity_style,
                core_class,
                cover_class,
                cover_style,
                has_music,
                is_expanded,
                primary_class,
                visible_primary_text,
                outgoing_primary_text,
                transition_key,
                lyric_scroll_class,
                lyric_scroll_style,
                cpu: cpu_usage.read().clone(),
                memory: memory_usage.read().clone(),
                download: download.read().clone(),
                upload: upload.read().clone(),
                spectrum: *spectrum.read(),
                spectrum_style,
                progress,
                progress_style,
                duration: state.duration,
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
                    move |_| {
                        if audio_ready_for_toggle {
                            music_mode.set(MusicMode::Normal);
                            player.send(AudioCommand::PlayPause);
                        } else if let Some(track) = playpause_track.clone() {
                            music_mode.set(MusicMode::Normal);
                            spawn_play(
                                track,
                                player.clone(),
                                current_index,
                                current_track,
                                status,
                                lyrics,
                            );
                        } else {
                            status.set("Queue is empty.".to_string());
                        }
                    }
                },
                onnext: play_next,
                onstop: {
                    let player = player.clone();
                    move |_| {
                        current_index.set(None);
                        current_track.set(None);
                        music_mode.set(MusicMode::Silent);
                        lyrics.set(Vec::new());
                        player.send(AudioCommand::Stop);
                        status.set("Stopped.".to_string());
                    }
                },
                onseek: {
                    let player = player.clone();
                    move |position| player.send(AudioCommand::Seek(position))
                },
            }

            div { class: "panel-shell",
            section { class: "panel",
                Tabs {
                    tab: tab.clone(),
                    onsearch: move |_| active_tab.set("search".to_string()),
                    onqueue: move |_| active_tab.set("queue".to_string()),
                    onstats: move |_| active_tab.set("stats".to_string()),
                    onsettings: move |_| active_tab.set("settings".to_string()),
                }

                if tab == "search" {
                    SearchPanel {
                        source: search_source(),
                        query: query.read().clone(),
                        video_url: video_url.read().clone(),
                        local_music_folder: local_music_folder.read().clone(),
                        results: results.read().clone(),
                        random_count: random_count(),
                        status: status.read().clone(),
                        onsource: move |source| search_source.set(source),
                        onfocus: move |_| input_focused.set(true),
                        onblur: {
                            move |_| {
                                input_focused.set(false);
                                if !*pointer_inside.read() {
                                    expanded.set(false);
                                    schedule_window_collapse(
                                        window_expanded,
                                        pointer_inside,
                                        input_focused,
                                        transition_ticket,
                                    );
                                }
                            }
                        },
                        onquery: move |value| query.set(value),
                        onvideo_url: move |value| video_url.set(value),
                        onsearch: move |text: String| spawn_search(text, results, status),
                        onimport_video: move |(source, text): (SearchSource, String)| {
                            let import_source = match source {
                                SearchSource::Bilibili => VideoImportSource::Bilibili,
                                SearchSource::Youtube => VideoImportSource::Youtube,
                                _ => return,
                            };
                            spawn_import_video_url(import_source, text, queue, status)
                        },
                        onlocal_music_folder: move |value| local_music_folder.set(value),
                        onload_local: move |_| {
                            spawn_load_local_queue(local_music_folder.read().clone(), queue, status)
                        },
                        onrandom_append: move |count| load_random_append(count),
                        onrandom_replace: move |count| load_random_replace(count),
                        onrandom_count: move |value| random_count.set(value),
                        onadd: move |track| {
                            let (added, total) = {
                                let mut next = queue.write();
                                let added = append_unique_tracks(&mut next, [track]);
                                (added, next.len())
                            };
                            if added == 0 {
                                status.set("Track is already in the queue.".to_string());
                            } else {
                                status.set(format!("Queued {total} tracks."));
                            }
                        },
                    }
                } else if tab == "queue" {
                    QueuePanel {
                        queue_len: queue_len_for_panel,
                        visible_tracks: visible_queue_for_panel,
                        current_index: *current_index.read(),
                        onclear: {
                            let player = player.clone();
                            move |_| {
                                queue.set(Vec::new());
                                current_index.set(None);
                                current_track.set(None);
                                music_mode.set(MusicMode::Silent);
                                lyrics.set(Vec::new());
                                player.send(AudioCommand::Stop);
                                status.set("Queue cleared.".to_string());
                            }
                        },
                        onplay: {
                            let player = player.clone();
                            move |index| {
                                if let Some(track) = queue.read().get(index).cloned() {
                                    current_index.set(Some(index));
                                    music_mode.set(MusicMode::Normal);
                                    spawn_play(
                                        track,
                                        player.clone(),
                                        current_index,
                                        current_track,
                                        status,
                                        lyrics,
                                    );
                                }
                            }
                        },
                        onremove: {
                            let player = player.clone();
                            move |index| {
                                let mut list = queue.read().clone();
                                if index < list.len() {
                                    list.remove(index);
                                    queue.set(list);
                                    let current = *current_index.read();
                                    match current {
                                        Some(i) if i == index => {
                                            current_index.set(None);
                                            current_track.set(None);
                                            music_mode.set(MusicMode::Silent);
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
                } else if tab == "stats" {
                    StatsPanel {
                        cpu: cpu_usage.read().clone(),
                        memory: memory_usage.read().clone(),
                        upload: upload.read().clone(),
                        download: download.read().clone(),
                        cpu_progress: cpu_progress(),
                        memory_progress: memory_progress(),
                        upload_progress: upload_progress(),
                        download_progress: download_progress(),
                        total_upload: format_bytes(*total_upload.read()),
                        total_download: format_bytes(*total_download.read()),
                        month_total: format_bytes(total_upload() + total_download()),
                        status: status.read().clone(),
                    }
                } else {
                    SettingsPanel {
                        opacity: opacity(),
                        volume: volume(),
                        island_size: island_size(),
                        music_mode: music_mode(),
                        update_status: update_status.read().clone(),
                        update_progress: *update_progress.read(),
                        update_available: pending_update.read().is_some(),
                        update_busy: update_busy(),
                        onslider_focus: move |_| input_focused.set(true),
                        onslider_blur: {
                            move |_| {
                                input_focused.set(false);
                                if !*pointer_inside.read() {
                                    expanded.set(false);
                                    schedule_window_collapse(
                                        window_expanded,
                                        pointer_inside,
                                        input_focused,
                                        transition_ticket,
                                    );
                                }
                            }
                        },
                        onslider_down: move |_| input_focused.set(true),
                        onslider_up: {
                            move |_| {
                                input_focused.set(false);
                                if !*pointer_inside.read() {
                                    expanded.set(false);
                                    schedule_window_collapse(
                                        window_expanded,
                                        pointer_inside,
                                        input_focused,
                                        transition_ticket,
                                    );
                                }
                            }
                        },
                        onopacity: move |value| opacity.set(value),
                        onvolume: {
                            let player = player.clone();
                            move |value| {
                                volume.set(value);
                                player.send(AudioCommand::SetVolume(value as f32 / 100.0));
                            }
                        },
                        onisland_size: {
                            let desktop = desktop.clone();
                            move |value| {
                                island_size.set(value);
                                set_island_window(
                                    &desktop,
                                    expanded(),
                                    value as f64 / 100.0,
                                    collapsed_width,
                                );
                            }
                        },
                        onnormal: move |_| {
                            if has_active_music {
                                music_mode.set(MusicMode::Normal);
                                status.set("Normal mode.".to_string());
                            } else {
                                status.set("No active music.".to_string());
                            }
                        },
                        onsilent: {
                            let player = player.clone();
                            move |_| {
                                current_index.set(None);
                                current_track.set(None);
                                music_mode.set(MusicMode::Silent);
                                lyrics.set(Vec::new());
                                player.send(AudioCommand::Stop);
                                status.set("Silent.".to_string());
                            }
                        },
                        onquiet: move |_| {
                            if has_active_music {
                                music_mode.set(MusicMode::Quiet);
                                status.set("Quiet mode: music keeps playing.".to_string());
                            } else {
                                status.set("No active music.".to_string());
                            }
                        },
                        onclean_cache: {
                            let player = player.clone();
                            move |_| {
                                current_index.set(None);
                                current_track.set(None);
                                music_mode.set(MusicMode::Silent);
                                lyrics.set(Vec::new());
                                player.send(AudioCommand::Stop);
                                status.set("Cleaning downloaded audio cache...".to_string());
                                spawn(async move {
                                    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
                                    match storage::clean_song_cache() {
                                        Ok(()) => status.set("Downloaded audio cache cleaned.".to_string()),
                                        Err(err) => status.set(err),
                                    }
                                });
                            }
                        },
                        oncheck_update: move |_| {
                            if update_busy() {
                                return;
                            }
                            pending_update.set(None);
                            update_progress.set(None);
                            update_busy.set(true);
                            update_status.set("Checking updates...".to_string());
                            spawn(async move {
                                match updater::check_latest_release().await {
                                    Ok(updater::UpdateStatus::Current {
                                        current,
                                        latest,
                                        url: _,
                                    }) => {
                                        let message = if current == latest {
                                            "Already latest.".to_string()
                                        } else {
                                            format!("Already latest: CAPS {latest}.")
                                        };
                                        update_status.set(message);
                                    }
                                    Ok(updater::UpdateStatus::Available(update)) => {
                                        let size = if update.asset_size > 0 {
                                            format!(" ({})", format_bytes(update.asset_size))
                                        } else {
                                            String::new()
                                        };
                                        let message =
                                            format!("Ready to update: CAPS {}{size}.", update.latest);
                                        pending_update.set(Some(update));
                                        update_status.set(message);
                                    }
                                    Err(err) => {
                                        update_status.set(err);
                                    }
                                }
                                update_busy.set(false);
                            });
                        },
                        oninstall_update: {
                            let desktop_for_install = desktop.clone();
                            move |_| {
                                if update_busy() {
                                    return;
                                }
                                let Some(update) = pending_update.read().clone() else {
                                    update_status.set("Check for an update first.".to_string());
                                    return;
                                };
                                update_busy.set(true);
                                update_progress.set(Some(0.0));
                                update_status.set(format!("Downloading CAPS {}...", update.latest));
                                let desktop = desktop_for_install.clone();
                                spawn(async move {
                                    let latest = update.latest.clone();
                                    let result = updater::download_and_install_update(
                                        update,
                                        |downloaded, total| {
                                            if let Some(total) = total.filter(|value| *value > 0) {
                                                let progress = (downloaded as f64 / total as f64 * 100.0)
                                                    .clamp(0.0, 100.0);
                                                update_progress.set(Some(progress));
                                                update_status.set(format!(
                                                    "Downloading CAPS {latest}: {} / {} ({progress:.0}%).",
                                                    format_bytes(downloaded),
                                                    format_bytes(total)
                                                ));
                                            } else {
                                                update_status.set(format!(
                                                    "Downloading CAPS {latest}: {}.",
                                                    format_bytes(downloaded)
                                                ));
                                            }
                                        },
                                    )
                                    .await;
                                    match result {
                                        Ok(()) => {
                                            update_progress.set(Some(100.0));
                                            update_status.set(
                                                "Installing update. CAPS will restart.".to_string(),
                                            );
                                            tokio::time::sleep(std::time::Duration::from_millis(180)).await;
                                            desktop.close();
                                        }
                                        Err(err) => {
                                            update_progress.set(None);
                                            update_status.set(err);
                                            update_busy.set(false);
                                        }
                                    }
                                });
                            }
                        },
                    }
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

fn schedule_window_collapse(
    mut window_expanded: Signal<bool>,
    pointer_inside: Signal<bool>,
    input_focused: Signal<bool>,
    mut transition_ticket: Signal<u64>,
) {
    let ticket = transition_ticket().wrapping_add(1);
    transition_ticket.set(ticket);
    spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(WINDOW_COLLAPSE_DELAY_MS)).await;
        if transition_ticket() == ticket && !pointer_inside() && !input_focused() {
            window_expanded.set(false);
        }
    });
}

fn text_visual_units(text: &str) -> f64 {
    text.chars()
        .map(|ch| {
            if ch.is_ascii_whitespace() {
                0.28
            } else if ch.is_ascii_alphanumeric() {
                0.56
            } else if ch.is_ascii_punctuation() {
                0.34
            } else if ch.is_whitespace() {
                0.36
            } else {
                1.0
            }
        })
        .sum()
}

fn activity_energy_from_spectrum(spectrum: &[f32]) -> f64 {
    let weights = [1.2, 1.16, 1.05, 0.94, 0.76, 0.62, 0.48];
    let mut weighted_energy = 0.0_f64;
    let mut total_weight = 0.0_f64;

    for (index, value) in spectrum.iter().enumerate() {
        let weight = weights.get(index).copied().unwrap_or(0.5);
        let normalized = ((*value as f64 - 0.28) / 0.94).clamp(0.0, 1.0);
        weighted_energy += normalized.powf(1.18) * weight;
        total_weight += weight;
    }

    if total_weight <= f64::EPSILON {
        0.0
    } else {
        (weighted_energy / total_weight).clamp(0.0, 1.0)
    }
}

fn format_percent(value: f64) -> String {
    let value = value.clamp(0.0, 100.0);
    if value >= 99.95 {
        "100%".to_string()
    } else {
        format!("{value:.1}%")
    }
}

fn memory_percent_value(used: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    (used as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
}

fn rate_progress(rate: u64, peak: f64) -> f64 {
    if peak <= f64::EPSILON {
        return 0.0;
    }
    (rate as f64 / peak * 100.0).clamp(0.0, 100.0)
}

const APP_CSS: &str = include_str!("app.css");
