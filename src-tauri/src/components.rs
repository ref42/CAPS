use crate::track::Track;
use dioxus::prelude::*;

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
    upload: String,
    download: String,
    total_upload: String,
    total_download: String,
    month_total: String,
    status: String,
) -> Element {
    rsx! {
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
            div { class: "status-text", "{status}" }
        }
    }
}

#[component]
pub fn Island(
    island_class: &'static str,
    core_class: &'static str,
    cover_class: &'static str,
    cover_style: String,
    has_music: bool,
    is_expanded: bool,
    primary_class: &'static str,
    visible_primary_text: String,
    outgoing_primary_text: Option<String>,
    transition_key: u64,
    download: String,
    upload: String,
    spectrum: [f32; 5],
    progress: f64,
    is_playing: bool,
    ondrag: EventHandler<MouseEvent>,
    onprev: EventHandler<MouseEvent>,
    onplaypause: EventHandler<MouseEvent>,
    onnext: EventHandler<MouseEvent>,
    onstop: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        section {
            class: "{island_class}",
            onmousedown: ondrag,
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
            div { class: "spectrum",
                for value in spectrum {
                    i { style: "transform: scaleY({value});" }
                }
            }
            if is_expanded && has_music {
                div { class: "playback-progress",
                    span { style: "width: {progress}%;" }
                }
            }
        }
    }
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
