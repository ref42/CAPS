use crate::audio_spectrum::SPECTRUM_BANDS;
use crate::track::Track;
use dioxus::prelude::*;

pub const QUEUE_RENDER_LIMIT: usize = 300;

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
    query: String,
    local_music_folder: String,
    results: Vec<Track>,
    random_count: u32,
    status: String,
    onfocus: EventHandler<FocusEvent>,
    onblur: EventHandler<FocusEvent>,
    onquery: EventHandler<String>,
    onsearch: EventHandler<String>,
    onlocal_music_folder: EventHandler<String>,
    onload_local: EventHandler<MouseEvent>,
    onrandom: EventHandler<u32>,
    onrandom_count: EventHandler<u32>,
    onadd: EventHandler<Track>,
) -> Element {
    let search_from_key = query.trim().to_string();
    let random_progress =
        ((random_count.saturating_sub(1) as f64 / 99.0) * 100.0).clamp(0.0, 100.0);
    rsx! {
        div { class: "panel-section",
            div { class: "search-row",
                div { class: "search-field",
                    input {
                        placeholder: "Search NetEase",
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
            }
            div { class: "local-loader",
                span { "Local folder" }
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
                    onclick: move |_| onrandom.call(random_count),
                    "Add"
                }
            }
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
    spring_style: String,
    onopacity: EventHandler<u32>,
    onvolume: EventHandler<u32>,
    onisland_size: EventHandler<u32>,
    onspring_style: EventHandler<String>,
) -> Element {
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
                        oninput: move |event| {
                            if let Ok(value) = event.value().parse::<u32>() {
                                onisland_size.call(value.clamp(85, 135));
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
                        class: if spring_style == "smooth" { "active" } else { "" },
                        onclick: move |_| onspring_style.call("smooth".to_string()),
                        "Smooth"
                    }
                    button {
                        class: if spring_style == "bouncy" { "active" } else { "" },
                        onclick: move |_| onspring_style.call("bouncy".to_string()),
                        "Bouncy"
                    }
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
    rsx! {
        button { class: if active { "song active" } else { "song" }, onclick,
            span { class: "song-cover", style: "{cover_style}" }
            span { class: "song-copy",
                strong { "{track.name}" }
                small { "{detail}" }
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
    rsx! {
        div { class: if active { "song queue-song active" } else { "song queue-song" },
            button { class: "queue-main", onclick: onplay,
                span { class: "song-cover", style: "{cover_style}" }
                span { class: "song-copy",
                    strong { "{track.name}" }
                    small { "{detail}" }
                }
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
