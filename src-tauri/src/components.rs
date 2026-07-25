use crate::audio_spectrum::SPECTRUM_BANDS;
use crate::mode::MusicMode;
use crate::track::Track;
use dioxus::prelude::*;

pub const QUEUE_RENDER_LIMIT: usize = 300;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SearchSource {
    Netease,
    Bilibili,
    Youtube,
    Local,
}

#[component]
pub fn Tabs(
    tab: String,
    onsearch: EventHandler<MouseEvent>,
    onqueue: EventHandler<MouseEvent>,
    onstats: EventHandler<MouseEvent>,
    onsettings: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        div { class: "tabs",
            button {
                class: if tab == "search" { "tab active" } else { "tab" },
                onclick: onsearch,
                "Search"
            }
            button {
                class: if tab == "queue" { "tab active" } else { "tab" },
                onclick: onqueue,
                "Queue"
            }
            button {
                class: if tab == "stats" { "tab active" } else { "tab" },
                onclick: onstats,
                "Stats"
            }
            button {
                class: if tab == "settings" { "tab active" } else { "tab" },
                onclick: onsettings,
                "Settings"
            }
        }
    }
}

#[component]
pub fn StatsPanel(
    cpu: String,
    memory: String,
    upload: String,
    download: String,
    cpu_progress: f64,
    memory_progress: f64,
    upload_progress: f64,
    download_progress: f64,
    total_upload: String,
    total_download: String,
    month_total: String,
    status: String,
) -> Element {
    rsx! {
        div { class: "panel-section stats",
            div { class: "live-grid",
                LiveStatTile { label: "CPU", value: cpu, progress: cpu_progress }
                LiveStatTile { label: "RAM", value: memory, progress: memory_progress }
                LiveStatTile { label: "Upload", value: upload, progress: upload_progress }
                LiveStatTile { label: "Download", value: download, progress: download_progress }
            }
            div { class: "total-list",
                div { class: "stat-line",
                    span { "Total up" }
                    strong { "{total_upload}" }
                }
                div { class: "stat-line",
                    span { "Total down" }
                    strong { "{total_download}" }
                }
                div { class: "stat-line",
                    span { "This month" }
                    strong { "{month_total}" }
                }
            }
            div { class: "status-text", "{status}" }
        }
    }
}

#[component]
fn LiveStatTile(label: &'static str, value: String, progress: f64) -> Element {
    let progress = progress.clamp(0.0, 100.0);
    rsx! {
        div { class: "live-stat", style: "--stat-progress: {progress:.2}%;",
            span { "{label}" }
            strong { "{value}" }
            i {}
        }
    }
}

#[component]
pub fn SearchPanel(
    source: SearchSource,
    query: String,
    video_url: String,
    local_music_folder: String,
    results: Vec<Track>,
    random_count: u32,
    status: String,
    onsource: EventHandler<SearchSource>,
    onfocus: EventHandler<FocusEvent>,
    onblur: EventHandler<FocusEvent>,
    onquery: EventHandler<String>,
    onvideo_url: EventHandler<String>,
    onsearch: EventHandler<String>,
    onimport_video: EventHandler<(SearchSource, String)>,
    onlocal_music_folder: EventHandler<String>,
    onload_local: EventHandler<MouseEvent>,
    onrandom_append: EventHandler<u32>,
    onrandom_replace: EventHandler<u32>,
    onrandom_count: EventHandler<u32>,
    onadd: EventHandler<Track>,
) -> Element {
    let search_from_key = query.trim().to_string();
    let search_from_button = search_from_key.clone();
    let import_from_key = video_url.trim().to_string();
    let import_from_button = import_from_key.clone();
    let video_source_label = match source {
        SearchSource::Bilibili => "Bilibili",
        SearchSource::Youtube => "YouTube",
        _ => "",
    };
    let video_placeholder = match source {
        SearchSource::Bilibili => "Paste Bilibili video URL",
        SearchSource::Youtube => "Paste YouTube video URL",
        _ => "",
    };
    let random_progress =
        ((random_count.saturating_sub(1) as f64 / 99.0) * 100.0).clamp(0.0, 100.0);
    rsx! {
        div { class: "panel-section",
            div { class: "source-switch",
                button {
                    class: if source == SearchSource::Netease { "source-option active" } else { "source-option" },
                    onclick: move |_| onsource.call(SearchSource::Netease),
                    "NetEase"
                }
                button {
                    class: if source == SearchSource::Bilibili { "source-option active" } else { "source-option" },
                    onclick: move |_| onsource.call(SearchSource::Bilibili),
                    "Bilibili"
                }
                button {
                    class: if source == SearchSource::Youtube { "source-option active" } else { "source-option" },
                    onclick: move |_| onsource.call(SearchSource::Youtube),
                    "YouTube"
                }
                button {
                    class: if source == SearchSource::Local { "source-option active" } else { "source-option" },
                    onclick: move |_| onsource.call(SearchSource::Local),
                    "Local"
                }
            }

            if source == SearchSource::Netease {
                div { class: "source-mode netease-mode",
                    div { class: "search-row",
                        div { class: "search-field",
                            input {
                                value: "{query}",
                                placeholder: "Song, artist, album",
                                onfocus,
                                onblur,
                                oninput: move |event| onquery.call(event.value()),
                                onkeydown: move |event| {
                                    if event.key() == Key::Enter && !event.is_composing() {
                                        onsearch.call(search_from_key.clone());
                                    }
                                }
                            }
                            span { class: "search-icon", "⌕" }
                        }
                        button {
                            class: "source-action",
                            onclick: move |_| onsearch.call(search_from_button.clone()),
                            "Search"
                        }
                    }
                    div { class: "random-control",
                        span { "Random {random_count}" }
                        input {
                            r#type: "range",
                            min: "1",
                            max: "100",
                            value: "{random_count}",
                            style: "--random-progress: {random_progress:.2}%;",
                            oninput: move |event| {
                                if let Ok(value) = event.value().parse::<u32>() {
                                    onrandom_count.call(value.clamp(1, 100));
                                }
                            },
                        }
                        button {
                            class: "random-add",
                            onclick: move |_| onrandom_append.call(random_count),
                            "Append"
                        }
                        button {
                            class: "random-add random-replace",
                            onclick: move |_| onrandom_replace.call(random_count),
                            "Replace"
                        }
                    }
                }
            }

            if source == SearchSource::Bilibili || source == SearchSource::Youtube {
                div { class: "source-mode video-mode",
                    div { class: "search-row",
                        div { class: "search-field",
                            input {
                                value: "{video_url}",
                                placeholder: "{video_placeholder}",
                                onfocus,
                                onblur,
                                oninput: move |event| onvideo_url.call(event.value()),
                                onkeydown: move |event| {
                                    if event.key() == Key::Enter && !event.is_composing() {
                                        onimport_video.call((source, import_from_key.clone()));
                                    }
                                }
                            }
                            span { class: "search-icon video-source-mark", "↧" }
                        }
                        button {
                            class: "source-action video-import",
                            onclick: move |_| onimport_video.call((source, import_from_button.clone())),
                            "Extract"
                        }
                    }
                    div { class: "import-readout",
                        span { "{video_source_label}" }
                        strong { "{status}" }
                    }
                }
            }

            if source == SearchSource::Local {
                div { class: "source-mode local-mode",
                    div { class: "local-loader",
                        span { "Folder" }
                        input {
                            r#type: "text",
                            value: "{local_music_folder}",
                            placeholder: "D:\\Music",
                            onfocus,
                            onblur,
                            oninput: move |event| onlocal_music_folder.call(event.value()),
                        }
                        button {
                            onclick: onload_local,
                            "Load"
                        }
                    }
                    div { class: "import-readout",
                        span { "Local" }
                        strong { "{status}" }
                    }
                }
            }

            if source == SearchSource::Netease {
                div { class: "song-list",
                    for track in results.iter().cloned() {
                        TrackRow {
                            track: track.clone(),
                            action: "+",
                            active: false,
                            onclick: move |_| onadd.call(track.clone()),
                        }
                    }
                    if results.is_empty() {
                        div { class: "empty", "{status}" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn QueuePanel(
    queue_len: usize,
    visible_tracks: Vec<(usize, Track)>,
    current_index: Option<usize>,
    onclear: EventHandler<MouseEvent>,
    onplay: EventHandler<usize>,
    onremove: EventHandler<usize>,
) -> Element {
    let hidden_count = queue_len.saturating_sub(visible_tracks.len());
    rsx! {
        div { class: "panel-section",
            div { class: "queue-toolbar",
                span { "{queue_len} tracks" }
                button { onclick: onclear, "Clear" }
            }
            div { class: "song-list",
                for (index, track) in visible_tracks {
                    QueueTrackRow {
                        track,
                        active: current_index.is_some_and(|i| i == index),
                        onplay: move |_| onplay.call(index),
                        onremove: move |_| onremove.call(index),
                    }
                }
                if hidden_count > 0 {
                    div { class: "empty", "Showing first {QUEUE_RENDER_LIMIT}. {hidden_count} more tracks stay in the queue." }
                }
                if queue_len == 0 {
                    div { class: "empty", "Queue is empty." }
                }
            }
        }
    }
}

#[component]
pub fn SettingsPanel(
    opacity: u32,
    volume: u32,
    island_size: u32,
    music_mode: MusicMode,
    update_status: String,
    update_progress: Option<f64>,
    update_available: bool,
    update_busy: bool,
    onslider_focus: EventHandler<FocusEvent>,
    onslider_blur: EventHandler<FocusEvent>,
    onslider_down: EventHandler<MouseEvent>,
    onslider_up: EventHandler<MouseEvent>,
    onopacity: EventHandler<u32>,
    onvolume: EventHandler<u32>,
    onisland_size: EventHandler<u32>,
    onnormal: EventHandler<MouseEvent>,
    onsilent: EventHandler<MouseEvent>,
    onquiet: EventHandler<MouseEvent>,
    onclean_cache: EventHandler<MouseEvent>,
    oncheck_update: EventHandler<MouseEvent>,
    oninstall_update: EventHandler<MouseEvent>,
) -> Element {
    let opacity_progress = ((opacity.saturating_sub(10) as f64 / 90.0) * 100.0).clamp(0.0, 100.0);
    let volume_progress = (volume as f64).clamp(0.0, 100.0);
    let island_progress =
        ((island_size.saturating_sub(85) as f64 / 50.0) * 100.0).clamp(0.0, 100.0);
    rsx! {
        div { class: "panel-section settings",
            label { class: "setting",
                span { "Opacity" }
                div { class: "setting-control",
                    input {
                        r#type: "range",
                        min: "10",
                        max: "100",
                        value: "{opacity}",
                        style: "--setting-progress: {opacity_progress:.2}%;",
                        onfocus: move |event| onslider_focus.call(event),
                        onblur: move |event| onslider_blur.call(event),
                        onmousedown: move |event| onslider_down.call(event),
                        onmouseup: move |event| onslider_up.call(event),
                        oninput: move |event| {
                            if let Ok(value) = event.value().parse::<u32>() {
                                onopacity.call(value.clamp(10, 100));
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
                        style: "--setting-progress: {volume_progress:.2}%;",
                        onfocus: move |event| onslider_focus.call(event),
                        onblur: move |event| onslider_blur.call(event),
                        onmousedown: move |event| onslider_down.call(event),
                        onmouseup: move |event| onslider_up.call(event),
                        oninput: move |event| {
                            if let Ok(value) = event.value().parse::<u32>() {
                                onvolume.call(value.clamp(0, 100));
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
                        style: "--setting-progress: {island_progress:.2}%;",
                        onfocus: move |event| onslider_focus.call(event),
                        onblur: move |event| onslider_blur.call(event),
                        onmousedown: move |event| onslider_down.call(event),
                        onmouseup: move |event| onslider_up.call(event),
                        oninput: move |event| {
                            if let Ok(value) = event.value().parse::<u32>() {
                                onisland_size.call(value.clamp(85, 135));
                            }
                        },
                    }
                    output { "{island_size}%" }
                }
            }
            label { class: "setting",
                span { "Mode" }
                div { class: "mode-actions",
                    button {
                        class: if music_mode == MusicMode::Normal { "mode-button normal-button active" } else { "mode-button normal-button" },
                        onclick: onnormal,
                        "Normal"
                    }
                    button {
                        class: if music_mode == MusicMode::Silent { "mode-button silent-button active" } else { "mode-button silent-button" },
                        onclick: onsilent,
                        "Silent"
                    }
                    button {
                        class: if music_mode == MusicMode::Quiet { "mode-button quiet-button active" } else { "mode-button quiet-button" },
                        onclick: onquiet,
                        "Quiet"
                    }
                }
            }
            label { class: "setting",
                span { "Disk cache" }
                div { class: "cache-actions",
                    button {
                        onclick: onclean_cache,
                        "Clean cache"
                    }
                }
            }
            label { class: "setting",
                span { "Updates" }
                div { class: "cache-actions update-actions",
                    button {
                        disabled: update_busy,
                        onclick: oncheck_update,
                        "Check update"
                    }
                    if update_available {
                        button {
                            disabled: update_busy,
                            onclick: oninstall_update,
                            "Update"
                        }
                    }
                }
            }
            div { class: "update-readout",
                div { class: "update-copy", "{update_status}" }
                if let Some(progress) = update_progress {
                    i { style: "--update-progress: {progress:.2}%;" }
                }
            }
            div { class: "status-text", "Right-click the island to exit CAPS." }
        }
    }
}

#[component]
pub fn Island(
    island_class: &'static str,
    activity_class: &'static str,
    activity_title: &'static str,
    activity_style: String,
    core_class: &'static str,
    cover_class: &'static str,
    cover_style: String,
    has_music: bool,
    is_expanded: bool,
    primary_class: &'static str,
    visible_primary_text: String,
    outgoing_primary_text: Option<String>,
    transition_key: u64,
    lyric_scroll_class: &'static str,
    lyric_scroll_style: String,
    cpu: String,
    memory: String,
    download: String,
    upload: String,
    spectrum: [f32; SPECTRUM_BANDS],
    spectrum_style: String,
    progress: f64,
    progress_style: String,
    duration: f64,
    is_playing: bool,
    ondrag: EventHandler<MouseEvent>,
    onprev: EventHandler<MouseEvent>,
    onplaypause: EventHandler<MouseEvent>,
    onnext: EventHandler<MouseEvent>,
    onstop: EventHandler<MouseEvent>,
    onseek: EventHandler<f64>,
) -> Element {
    rsx! {
        section {
            class: "{island_class}",
            onmousedown: ondrag,
            div { class: "{activity_class}", title: "{activity_title}", style: "{activity_style}" }
            div { class: "{core_class}",
                if has_music {
                    div { class: "{cover_class}",
                        div { class: "cover-art", style: "{cover_style}" }
                    }
                    if !is_expanded {
                        div { class: "music-copy",
                            div { class: "lyric-viewport",
                                if let Some(outgoing_text) = outgoing_primary_text {
                                    ParticleLyricText {
                                        text: outgoing_text,
                                        primary_class,
                                        outgoing: true,
                                        key: "out-{transition_key}",
                                    }
                                }
                                div {
                                    class: "lyric-layer lyric-in {lyric_scroll_class}",
                                    style: "{lyric_scroll_style}",
                                    key: "in-{transition_key}-{visible_primary_text}",
                                    ParticleLyricText {
                                        text: visible_primary_text,
                                        primary_class,
                                        outgoing: false,
                                    }
                                }
                            }
                        }
                    }
                } else {
                    div { class: "speed-copy idle-speeds",
                        div {
                            class: "speed-stat system-stat",
                            title: "CPU usage",
                            span { "▦" }
                            strong { "{cpu}" }
                        }
                        div {
                            class: "speed-stat system-stat",
                            title: "RAM usage",
                            span { "▤" }
                            strong { "{memory}" }
                        }
                        div {
                            class: "speed-stat download-stat",
                            title: "Download speed",
                            span { class: "speed-arrow", "↓" }
                            strong { "{download}" }
                        }
                        div {
                            class: "speed-stat upload-stat",
                            title: "Upload speed",
                            span { class: "speed-arrow", "↑" }
                            strong { "{upload}" }
                        }
                    }
                }
            }
            if is_expanded && has_music {
                div { class: "mini-controls",
                    button { onclick: onprev, title: "Previous", "⏮" }
                    button {
                        onclick: onplaypause,
                        title: "Play/Pause",
                        if is_playing {
                            "Ⅱ"
                        } else {
                            "▶"
                        }
                    }
                    button { onclick: onnext, title: "Next", "⏭" }
                    button { onclick: onstop, title: "Stop", "■" }
                }
            }
            div { class: "spectrum", style: "{spectrum_style}",
                for value in spectrum {
                    i { style: "transform: scaleY({value});" }
                }
            }
            if is_expanded && has_music {
                div { class: "playback-progress",
                    input {
                        r#type: "range",
                        min: "0",
                        max: "100",
                        step: "0.1",
                        value: "{progress}",
                        style: "{progress_style}",
                        oninput: move |event| {
                            if duration > 0.0 {
                                if let Ok(value) = event.value().parse::<f64>() {
                                    onseek.call((value.clamp(0.0, 100.0) / 100.0) * duration);
                                }
                            }
                        },
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
struct LyricParticle {
    text: String,
    style: String,
}

#[component]
fn ParticleLyricText(text: String, primary_class: &'static str, outgoing: bool) -> Element {
    let particles = lyric_particles(&text);
    let class = if outgoing {
        format!("{primary_class} lyric-layer lyric-out lyric-particles")
    } else {
        format!("{primary_class} lyric-particles")
    };
    rsx! {
        strong { class: "{class}",
            for (index, particle) in particles.iter().enumerate() {
                span {
                    class: "lyric-particle",
                    style: "{particle.style}",
                    key: "{index}-{particle.text}",
                    "{particle.text}"
                }
            }
        }
    }
}

fn lyric_particles(text: &str) -> Vec<LyricParticle> {
    text.chars()
        .enumerate()
        .map(|(index, ch)| {
            let text = if ch.is_whitespace() {
                "\u{00a0}".to_string()
            } else {
                ch.to_string()
            };
            let delay = (index as f64 * 9.0).min(160.0);
            LyricParticle {
                text,
                style: format!("--particle-delay: {delay:.1}ms;"),
            }
        })
        .collect()
}

#[component]
pub fn TrackRow(
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
    let detail = track_detail(&track);
    let duration = track
        .duration
        .map(format_track_duration)
        .unwrap_or_default();
    rsx! {
        button { class: if active { "song active" } else { "song" }, onclick,
            span { class: "song-cover", style: "{cover_style}" }
            span { class: "song-copy",
                strong { "{track.name}" }
                small { "{detail}" }
            }
            if !duration.is_empty() {
                span { class: "track-duration", "{duration}" }
            }
            span { class: "song-action", "{action}" }
        }
    }
}

#[component]
pub fn QueueTrackRow(
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
    let detail = track_detail(&track);
    let duration = track
        .duration
        .map(format_track_duration)
        .unwrap_or_default();
    rsx! {
        div { class: if active { "song queue-song active" } else { "song queue-song" },
            button { class: "queue-main", onclick: onplay,
                span { class: "song-cover", style: "{cover_style}" }
                span { class: "song-copy",
                    strong { "{track.name}" }
                    small { "{detail}" }
                }
            }
            if !duration.is_empty() {
                span { class: "track-duration queue-duration", "{duration}" }
            }
            button { class: "remove-song", onclick: onremove, title: "Remove", "×" }
        }
    }
}

fn track_detail(track: &Track) -> String {
    if track.source == crate::track::SOURCE_LOCAL {
        track.artist.clone()
    } else {
        track.artist.clone()
    }
}

fn format_track_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}
