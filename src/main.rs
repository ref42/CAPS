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
mod qqmusic;
mod shitease;
mod storage;
mod track;
mod updater;
mod windowing;
mod youtube;

use actions::{
    RandomQueueMode, VideoImportSource, append_unique_tracks, spawn_import_video_url,
    spawn_load_local_queue, spawn_play, spawn_prefetch_next, spawn_random_queue, spawn_search,
};
use audio::{AudioCommand, AudioPlayer};
use components::{
    AddonIsland, Island, PetPanel, QUEUE_RENDER_LIMIT, QueuePanel, SearchPanel, SearchSource,
    SettingsPanel, Tabs, UiLanguage,
};
use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use formatting::{format_bytes, format_rate};
use lyrics::{LyricLine, current_lyric_line};
use mode::MusicMode;
use std::path::Path;
use std::sync::Arc;
use storage::AppSettings;
use sysinfo::{MINIMUM_CPU_UPDATE_INTERVAL, Networks, System};
use track::Track;
use windowing::{
    ADDON_COLLAPSED_W, ADDON_EXPANDED_W, ADDON_GAP, COLLAPSED_H, COLLAPSED_W, EXPANDED_H,
    EXPANDED_W, ISLAND_BLEED, MUSIC_COLLAPSED_W, place_top_center, set_island_window,
};

#[cfg(target_os = "windows")]
use dioxus::desktop::tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};

#[derive(Clone)]
struct LyricTransition {
    current: String,
    outgoing: Option<String>,
    id: u64,
}

const DEFAULT_SPECTRUM_FROM: &str = "rgb(125, 242, 202)";
const DEFAULT_SPECTRUM_TO: &str = "rgb(52, 199, 89)";
const WINDOW_COLLAPSE_DELAY_MS: u64 = 280;
const SPLIT_HOLD_MS: u64 = 470;
const SPLIT_SETTLE_MS: u64 = 370;

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
    let mut separated_islands = use_signal(|| false);
    let mut splitting_islands = use_signal(|| false);
    let mut separating_islands = use_signal(|| false);
    let mut merging_islands = use_signal(|| false);
    let mut split_press_ticket = use_signal(|| 0u64);
    let mut split_motion_ticket = use_signal(|| 0u64);
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
    let mut companion = use_signal({
        let saved_settings = saved_settings.clone();
        move || saved_settings.companion.clone()
    });
    let mut language_code = use_signal({
        let saved_settings = saved_settings.clone();
        move || saved_settings.language.clone()
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
    let mut bilibili_video_url = use_signal(String::new);
    let mut youtube_video_url = use_signal(String::new);
    let mut results = use_signal(Vec::<Track>::new);
    let mut queue = use_signal({
        let saved_state = saved_state.clone();
        move || saved_state.queue.clone()
    });
    let mut current_index = use_signal(move || saved_state.current_index);
    let mut current_track = use_signal(|| None::<Track>);
    let initial_language = UiLanguage::from_code(&saved_settings.language);
    let mut status = use_signal(move || default_status(initial_language).to_string());
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
    let mut spectrum_color_request = use_signal(|| 0_u64);
    let mut pending_update = use_signal(|| None::<updater::ReleaseUpdate>);
    let mut update_busy = use_signal(|| false);
    let mut update_progress = use_signal(|| None::<f64>);
    let mut update_status =
        use_signal(|| format!("Installed CAPS {}.", updater::current_version()));
    let coco_sprite_src = use_hook(coco_sprite_data_uri);
    let dodo_sprite_src = use_hook(dodo_sprite_data_uri);

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
                companion: companion.read().clone(),
                language: language_code.read().clone(),
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
        let cover_url = current_track
            .read()
            .as_ref()
            .map(|track| track.cover.clone())
            .or_else(|| {
                current_index
                    .read()
                    .and_then(|index| queue.read().get(index).map(|track| track.cover.clone()))
            })
            .unwrap_or_default();
        if *last_cover_for_color.borrow() == cover_url {
            return;
        }
        *last_cover_for_color.borrow_mut() = cover_url.clone();
        let request = spectrum_color_request().wrapping_add(1);
        spectrum_color_request.set(request);
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
            if spectrum_color_request() == request {
                spectrum_colors.set(colors);
            }
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
            let language = UiLanguage::from_code(&language_code.read());
            status.set(localized_status(language, "queue_empty").to_string());
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
            spawn_prefetch_next(queue, next);
        }
    };

    let player_for_prev = player.clone();
    let play_prev = move |_| {
        let len = queue.read().len();
        if len == 0 {
            let language = UiLanguage::from_code(&language_code.read());
            status.set(localized_status(language, "queue_empty").to_string());
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
            spawn_prefetch_next(queue, prev);
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
            spawn_prefetch_next(queue, next);
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
    let lyric_scroll_class = if has_music && lyric_units > 30.0 {
        "lyric-wrap lyric-dense"
    } else if has_music && lyric_units > 14.0 {
        "lyric-wrap"
    } else {
        ""
    };
    let lyric_scroll_style = if has_music {
        let target_units = if lyric_units > 48.0 {
            34.0
        } else if lyric_units > 30.0 {
            30.0
        } else if lyric_units > 14.0 {
            22.0
        } else {
            14.2
        };
        let fit_floor = if lyric_units > 48.0 {
            0.38
        } else if lyric_units > 30.0 {
            0.44
        } else if lyric_units > 14.0 {
            0.58
        } else {
            0.76
        };
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
    let title_units = text_visual_units(&current_title);
    let expanded_title_class = if has_music && title_units > 18.0 {
        "plain-title expanded-title-text title-marquee"
    } else {
        "plain-title expanded-title-text"
    };
    let expanded_title_style = if has_music && title_units > 18.0 {
        let overflow_px = ((title_units - 18.0) * 8.2).clamp(26.0, 220.0);
        let duration = (title_units * 0.42).clamp(7.0, 15.0);
        format!("--title-distance: -{overflow_px:.1}px; --title-duration: {duration:.1}s;")
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
    let companion_value = companion.read().clone();
    let language = UiLanguage::from_code(&language_code.read());
    let companion_is_dodo = companion_value == "dodo";
    let companion_sprite_src = if companion_is_dodo {
        dodo_sprite_src.clone()
    } else {
        coco_sprite_src.clone()
    };
    let coco_style = format!("--companion-image: url(\"{coco_sprite_src}\");");
    let dodo_style = format!("--companion-image: url(\"{dodo_sprite_src}\");");
    let companion_style = format!("--companion-image: url(\"{companion_sprite_src}\");");
    let companion_name = if companion_is_dodo { "Dodo" } else { "Coco" };
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
    let is_separated = separated_islands();
    let is_splitting = splitting_islands();
    let is_separating = separating_islands();
    let is_merging = merging_islands();
    let separation_visible = is_separated || is_splitting || is_separating || is_merging;
    let opacity_css = (*opacity.read() as f64 / 100.0).clamp(0.1, 1.0);
    let island_scale = (*island_size.read() as f64 / 100.0).clamp(0.85, 1.50);
    let collapsed_width = collapsed_width_for_text(&primary_text, has_music);
    let addon_width = if is_expanded {
        ADDON_EXPANDED_W
    } else {
        ADDON_COLLAPSED_W
    };
    let collapsed_addon_width = ADDON_COLLAPSED_W;
    let expanded_addon_width = ADDON_EXPANDED_W;
    let separation_extra_width = if separation_visible {
        ADDON_GAP + addon_width
    } else {
        0.0
    };
    let main_island_width = if is_expanded {
        EXPANDED_W
    } else {
        collapsed_width
    };
    let cluster_width = main_island_width + separation_extra_width;
    let stage_width = collapsed_width + separation_extra_width + ISLAND_BLEED * 2.0;
    let stage_height = COLLAPSED_H + ISLAND_BLEED * 2.0;
    let expanded_stage_width = EXPANDED_W + separation_extra_width + ISLAND_BLEED * 2.0;
    let expanded_stage_height = EXPANDED_H + ISLAND_BLEED * 2.0;
    let island_alpha = (opacity_css * 0.46).clamp(0.04, 0.46);
    let panel_alpha = (opacity_css * 0.52).clamp(0.06, 0.52);
    let soft_alpha = (opacity_css * 0.11).clamp(0.015, 0.11);
    let softer_alpha = (opacity_css * 0.065).clamp(0.01, 0.065);
    let hover_alpha = (opacity_css * 0.12).clamp(0.015, 0.12);
    let active_alpha = (opacity_css * 0.14).clamp(0.02, 0.14);
    let green_alpha = (opacity_css * 0.16).clamp(0.025, 0.16);
    let red_alpha = (opacity_css * 0.18).clamp(0.025, 0.18);
    let stage_style = format!(
        "--island-bg-alpha: {island_alpha:.3}; --panel-bg-alpha: {panel_alpha:.3}; --soft-alpha: {soft_alpha:.3}; --softer-alpha: {softer_alpha:.3}; --hover-alpha: {hover_alpha:.3}; --active-alpha: {active_alpha:.3}; --green-alpha: {green_alpha:.3}; --red-alpha: {red_alpha:.3}; --island-scale: {island_scale:.2}; --collapsed-width: {collapsed_width:.0}px; --stage-width: {stage_width:.0}px; --stage-height: {stage_height:.0}px; --expanded-stage-width: {expanded_stage_width:.0}px; --expanded-stage-height: {expanded_stage_height:.0}px; --island-bleed: {ISLAND_BLEED:.0}px; --addon-width: {addon_width:.0}px; --addon-collapsed-width: {collapsed_addon_width:.0}px; --addon-expanded-width: {expanded_addon_width:.0}px; --addon-gap: {ADDON_GAP:.0}px; --main-island-width: {main_island_width:.0}px; --cluster-width: {cluster_width:.0}px;"
    );
    let base_stage_class = match (*window_expanded.read(), is_expanded) {
        (true, true) => "stage window-expanded visual-expanded",
        (true, false) => "stage window-expanded",
        (false, _) => "stage",
    };
    let mut stage_class = base_stage_class.to_string();
    if is_separated {
        stage_class.push_str(" separated-islands");
    }
    if is_splitting {
        stage_class.push_str(" splitting-islands");
    }
    if is_separating {
        stage_class.push_str(" separating-islands");
    }
    if is_merging {
        stage_class.push_str(" merging-islands");
    }
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
    let window_size_cache = use_hook(|| std::cell::RefCell::new(None::<(bool, bool, i32, u32)>));

    {
        let desktop = desktop.clone();
        let cache = window_size_cache.clone();
        use_effect(move || {
            let window_expanded_key = window_expanded();
            let separated_key = separated_islands()
                || splitting_islands()
                || separating_islands()
                || merging_islands();
            let width_key = if window_expanded_key {
                EXPANDED_W.round() as i32
            } else {
                collapsed_width.round() as i32
            };
            let size_key = island_size();
            let next = (window_expanded_key, separated_key, width_key, size_key);
            if *cache.borrow() != Some(next) {
                *cache.borrow_mut() = Some(next);
                set_island_window(
                    &desktop,
                    window_expanded_key,
                    separated_key,
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
                splitting_islands.set(false);
                split_press_ticket.set(split_press_ticket().wrapping_add(1));
                collapse_window();
            },
            oncontextmenu: {
                let desktop = desktop.clone();
                move |_| {
                    let _ = storage::clean_song_cache();
                    desktop.close()
                }
            },
            div { class: "island-cluster",
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
                    expanded_title_text: current_title.clone(),
                    expanded_title_class,
                    expanded_title_style,
                    outgoing_primary_text,
                    transition_key,
                    lyric_scroll_class,
                    lyric_scroll_style,
                    cpu: cpu_usage.read().clone(),
                    memory: memory_usage.read().clone(),
                    download: download.read().clone(),
                    upload: upload.read().clone(),
                    spectrum: *spectrum.read(),
                    spectrum_style: spectrum_style.clone(),
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
                    onsplit_press_start: move |event: MouseEvent| {
                        if event.modifiers().shift()
                            || !event
                                .trigger_button()
                                .is_some_and(|button| button == MouseButton::Primary)
                        {
                            return;
                        }
                        let ticket = split_press_ticket().wrapping_add(1);
                        split_press_ticket.set(ticket);
                        split_motion_ticket.set(split_motion_ticket().wrapping_add(1));
                        separating_islands.set(false);
                        merging_islands.set(false);
                        splitting_islands.set(true);
                        window_expanded.set(true);
                        pointer_inside.set(true);
                        spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(SPLIT_HOLD_MS)).await;
                            if split_press_ticket() == ticket && splitting_islands() {
                                let motion_ticket = split_motion_ticket().wrapping_add(1);
                                split_motion_ticket.set(motion_ticket);
                                splitting_islands.set(false);
                                if separated_islands() {
                                    merging_islands.set(true);
                                    separating_islands.set(false);
                                    spawn(async move {
                                        tokio::time::sleep(std::time::Duration::from_millis(
                                            SPLIT_SETTLE_MS,
                                        ))
                                        .await;
                                        if split_motion_ticket() == motion_ticket {
                                            separated_islands.set(false);
                                            merging_islands.set(false);
                                        }
                                    });
                                } else {
                                    separated_islands.set(true);
                                    separating_islands.set(true);
                                    merging_islands.set(false);
                                    spawn(async move {
                                        tokio::time::sleep(std::time::Duration::from_millis(
                                            SPLIT_SETTLE_MS,
                                        ))
                                        .await;
                                        if split_motion_ticket() == motion_ticket {
                                            separating_islands.set(false);
                                        }
                                    });
                                }
                            }
                        });
                    },
                    onsplit_press_end: move |_| {
                        split_press_ticket.set(split_press_ticket().wrapping_add(1));
                        splitting_islands.set(false);
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
                                status.set(localized_status(language, "queue_empty").to_string());
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
                            status.set(localized_status(language, "stopped").to_string());
                        }
                    },
                    onseek: {
                        let player = player.clone();
                        move |position| player.send(AudioCommand::Seek(position))
                    },
                }
                div { class: "separation-neck" }
                AddonIsland {
                    companion_style: companion_style.clone(),
                    companion_name,
                    separated: is_separated,
                    splitting: is_splitting,
                    onhover: move |_| {
                        pointer_inside.set(true);
                        transition_ticket.set(transition_ticket().wrapping_add(1));
                        window_expanded.set(true);
                        expanded.set(true);
                    },
                }
            }

            div { class: "panel-shell",
            section { class: "panel",
                Tabs {
                    language,
                    tab: tab.clone(),
                    onsearch: move |_| active_tab.set("search".to_string()),
                    onqueue: move |_| active_tab.set("queue".to_string()),
                    onpet: move |_| active_tab.set("pet".to_string()),
                    onsettings: move |_| active_tab.set("settings".to_string()),
                }

                if tab == "search" {
                    SearchPanel {
                        language,
                        source: search_source(),
                        query: query.read().clone(),
                        video_url: match search_source() {
                            SearchSource::Bilibili => bilibili_video_url.read().clone(),
                            SearchSource::Youtube => youtube_video_url.read().clone(),
                            _ => String::new(),
                        },
                        local_music_folder: local_music_folder.read().clone(),
                        results: results.read().clone(),
                        random_count: random_count(),
                        status: status.read().clone(),
                        onsource: move |source| {
                            search_source.set(source);
                            results.set(Vec::new());
                            status.set(String::new());
                        },
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
                        onvideo_url: move |value| match search_source() {
                            SearchSource::Bilibili => bilibili_video_url.set(value),
                            SearchSource::Youtube => youtube_video_url.set(value),
                            _ => {}
                        },
                        onsearch: move |text: String| spawn_search(text, results, status),
                        onimport_video: move |(source, text): (SearchSource, String)| {
                            let import_source = match source {
                                SearchSource::Bilibili => VideoImportSource::Bilibili,
                                SearchSource::Youtube => VideoImportSource::Youtube,
                                _ => return,
                            };
                            spawn_import_video_url(import_source, text, queue, status)
                        },
                        onlocal_music_folder: move |_| {
                            let current = local_music_folder.read().clone();
                            spawn(async move {
                                match tokio::task::spawn_blocking(move || pick_folder(&current))
                                    .await
                                {
                                    Ok(Some(folder)) => local_music_folder.set(folder),
                                    Ok(None) => {}
                                    Err(err) => {
                                        status.set(format!(
                                            "{} {err}",
                                            localized_status(language, "folder_picker_failed")
                                        ));
                                    }
                                }
                            });
                        },
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
                                status.set(
                                    localized_status(language, "already_in_queue").to_string(),
                                );
                            } else {
                                status.set(format!(
                                    "{} {total} {}",
                                    localized_status(language, "queued_prefix"),
                                    localized_status(language, "tracks")
                                ));
                            }
                        },
                    }
                } else if tab == "queue" {
                    QueuePanel {
                        language,
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
                                status.set(localized_status(language, "queue_cleared").to_string());
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
                                    spawn_prefetch_next(queue, index);
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
                                            status.set(
                                                localized_status(language, "removed_current")
                                                    .to_string(),
                                            );
                                        }
                                        Some(i) if i > index => current_index.set(Some(i - 1)),
                                        _ => {}
                                    }
                                }
                            }
                        },
                    }
                } else if tab == "pet" {
                    PetPanel {
                        companion: companion_value.clone(),
                        coco_style: coco_style.clone(),
                        dodo_style: dodo_style.clone(),
                        oncompanion: move |value| companion.set(value),
                    }
                } else {
                    SettingsPanel {
                        language,
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
                                    separated_islands()
                                        || splitting_islands()
                                        || separating_islands()
                                        || merging_islands(),
                                    value as f64 / 100.0,
                                    collapsed_width,
                                );
                            }
                        },
                        onlanguage: move |value: String| {
                            language_code.set(UiLanguage::from_code(&value).code().to_string());
                        },
                        onnormal: move |_| {
                            if has_active_music {
                                music_mode.set(MusicMode::Normal);
                                status.set(localized_status(language, "normal").to_string());
                            } else {
                                status.set(localized_status(language, "no_active").to_string());
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
                                status.set(localized_status(language, "silent").to_string());
                            }
                        },
                        onquiet: move |_| {
                            if has_active_music {
                                music_mode.set(MusicMode::Quiet);
                                status.set(localized_status(language, "quiet").to_string());
                            } else {
                                status.set(localized_status(language, "no_active").to_string());
                            }
                        },
                        oncheck_update: move |_| {
                            if update_busy() {
                                return;
                            }
                            pending_update.set(None);
                            update_progress.set(None);
                            update_busy.set(true);
                            update_status.set(localized_status(language, "checking_updates").to_string());
                            spawn(async move {
                                match updater::check_latest_release().await {
                                    Ok(updater::UpdateStatus::Current {
                                        current,
                                        latest,
                                        url: _,
                                    }) => {
                                        let message = if current == latest {
                                            localized_status(language, "already_latest").to_string()
                                        } else {
                                            format!(
                                                "{} CAPS {latest}.",
                                                localized_status(language, "already_latest_prefix")
                                            )
                                        };
                                        update_status.set(message);
                                    }
                                    Ok(updater::UpdateStatus::Available(update)) => {
                                        let size = if update.asset_size > 0 {
                                            format!(" ({})", format_bytes(update.asset_size))
                                        } else {
                                            String::new()
                                        };
                                        let message = format!(
                                            "{} CAPS {}{size}.",
                                            localized_status(language, "ready_update"),
                                            update.latest
                                        );
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
                                    update_status.set(
                                        localized_status(language, "check_update_first").to_string(),
                                    );
                                    return;
                                };
                                update_busy.set(true);
                                update_progress.set(Some(0.0));
                                update_status.set(format!(
                                    "{} CAPS {}...",
                                    localized_status(language, "downloading"),
                                    update.latest
                                ));
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
                                                    "{} CAPS {latest}: {} / {} ({progress:.0}%).",
                                                    localized_status(language, "downloading"),
                                                    format_bytes(downloaded),
                                                    format_bytes(total)
                                                ));
                                            } else {
                                                update_status.set(format!(
                                                    "{} CAPS {latest}: {}.",
                                                    localized_status(language, "downloading"),
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
                                                localized_status(language, "installing_update")
                                                    .to_string(),
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

fn default_status(language: UiLanguage) -> &'static str {
    localized_status(language, "default")
}

fn localized_status(language: UiLanguage, key: &str) -> &'static str {
    match (language, key) {
        (UiLanguage::Zh, "default") => "搜索音乐，添加歌曲，然后从岛屿播放。",
        (UiLanguage::Zh, "queue_empty") => "队列为空。",
        (UiLanguage::Zh, "stopped") => "已停止。",
        (UiLanguage::Zh, "already_in_queue") => "这首歌已经在队列中。",
        (UiLanguage::Zh, "queued_prefix") => "已加入",
        (UiLanguage::Zh, "tracks") => "首歌曲。",
        (UiLanguage::Zh, "queue_cleared") => "队列已清空。",
        (UiLanguage::Zh, "removed_current") => "已移除当前歌曲。",
        (UiLanguage::Zh, "folder_picker_failed") => "文件夹选择失败：",
        (UiLanguage::Zh, "normal") => "普通模式。",
        (UiLanguage::Zh, "silent") => "静音。",
        (UiLanguage::Zh, "quiet") => "安静模式：音乐会继续播放。",
        (UiLanguage::Zh, "no_active") => "没有正在播放的音乐。",
        (UiLanguage::Zh, "checking_updates") => "正在检查更新...",
        (UiLanguage::Zh, "already_latest") => "已经是最新版本。",
        (UiLanguage::Zh, "already_latest_prefix") => "已经是最新版本：",
        (UiLanguage::Zh, "ready_update") => "可更新：",
        (UiLanguage::Zh, "check_update_first") => "请先检查更新。",
        (UiLanguage::Zh, "downloading") => "正在下载",
        (UiLanguage::Zh, "installing_update") => "正在安装更新。CAPS 将重启。",
        (_, "queue_empty") => "Queue is empty.",
        (_, "stopped") => "Stopped.",
        (_, "already_in_queue") => "Track is already in the queue.",
        (_, "queued_prefix") => "Queued",
        (_, "tracks") => "tracks.",
        (_, "queue_cleared") => "Queue cleared.",
        (_, "removed_current") => "Removed current track.",
        (_, "folder_picker_failed") => "Folder picker failed:",
        (_, "normal") => "Normal mode.",
        (_, "silent") => "Silent.",
        (_, "quiet") => "Quiet mode: music keeps playing.",
        (_, "no_active") => "No active music.",
        (_, "checking_updates") => "Checking updates...",
        (_, "already_latest") => "Already latest.",
        (_, "already_latest_prefix") => "Already latest:",
        (_, "ready_update") => "Ready to update:",
        (_, "check_update_first") => "Check for an update first.",
        (_, "downloading") => "Downloading",
        (_, "installing_update") => "Installing update. CAPS will restart.",
        _ => "Search music, add songs, then play from the island.",
    }
}

fn collapsed_width_for_text(_text: &str, has_music: bool) -> f64 {
    if has_music {
        MUSIC_COLLAPSED_W
    } else {
        COLLAPSED_W
    }
}

fn pick_folder(current: &str) -> Option<String> {
    let mut dialog = rfd::FileDialog::new();
    let current = current.trim();
    if !current.is_empty() {
        let path = Path::new(current);
        if path.exists() {
            dialog = dialog.set_directory(path);
        }
    }
    dialog.pick_folder().map(|path| path.display().to_string())
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
            } else if ch.is_ascii_uppercase() {
                0.68
            } else if ch.is_ascii_alphanumeric() {
                0.58
            } else if ch.is_ascii_punctuation() {
                0.34
            } else if ch.is_whitespace() {
                0.36
            } else if ch.is_ascii() {
                0.5
            } else {
                0.92
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

fn coco_sprite_data_uri() -> String {
    format!(
        "data:image/svg+xml;base64,{}",
        base64_encode(include_bytes!("../assets/coco.svg"))
    )
}

fn dodo_sprite_data_uri() -> String {
    format!(
        "data:image/svg+xml;base64,{}",
        base64_encode(include_bytes!("../assets/dodo.svg"))
    )
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let value = ((b0 as u32) << 16) | ((b1 as u32) << 8) | b2 as u32;

        encoded.push(TABLE[((value >> 18) & 0x3f) as usize] as char);
        encoded.push(TABLE[((value >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(TABLE[((value >> 6) & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(TABLE[(value & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
    }

    encoded
}

const APP_CSS: &str = include_str!("app.css");
