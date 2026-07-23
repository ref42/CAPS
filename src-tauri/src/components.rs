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
    spectrum: [f32; 11],
    activity: String,
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
            if activity.is_empty() {
                DottedSpectrum { spectrum }
            } else {
                LoadingOrb { activity }
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
fn DottedSpectrum(spectrum: [f32; 11]) -> Element {
    rsx! {
        div { class: "spectrum peak-spectrum",
            for (lane_index, value) in spectrum.into_iter().enumerate() {
                {
                    let hue = 44.0 + lane_index as f32 * 15.0;
                    let active_steps = ((value * 5.4).round() as i32).clamp(1, 7);
                    rsx! {
                        div {
                            class: "spectrum-lane",
                            key: "{lane_index}",
                            style: "--lane-hue: {hue:.1}; --lane-value: {value:.3};",
                            for dot_index in 0..7 {
                                {
                                    let level = 7 - dot_index;
                                    let active = level <= active_steps;
                                    let head = level == active_steps;
                                    let distance = (active_steps - level).abs() as f32;
                                    let alpha = if active {
                                        (0.92 - distance * 0.095).clamp(0.34, 0.96)
                                    } else {
                                        0.12
                                    };
                                    let scale = if head {
                                        (0.95 + value * 0.54).clamp(1.0, 1.62)
                                    } else if active {
                                        (0.72 + value * 0.22 - distance * 0.035).clamp(0.58, 1.08)
                                    } else {
                                        0.48
                                    };
                                    let delay = (lane_index as i32 * 14) - (dot_index as i32 * 10);
                                    let dot_class = if head {
                                        "spectrum-dot peak-dot"
                                    } else if active {
                                        "spectrum-dot active-dot"
                                    } else {
                                        "spectrum-dot"
                                    };
                                    rsx! {
                                        b {
                                            class: "{dot_class}",
                                            style: "--dot-scale: {scale:.3}; --dot-alpha: {alpha:.3}; --dot-delay: {delay}ms;"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn LoadingOrb(activity: String) -> Element {
    let orb_class = if activity == "searching" {
        "spectrum loading-orb searching-orb"
    } else {
        "spectrum loading-orb working-orb"
    };
    let dots = [
        (-8, -9, 0.78, 0.38),
        (-2, -11, 1.02, 0.62),
        (6, -10, 0.86, 0.5),
        (10, -4, 1.18, 0.72),
        (9, 3, 0.74, 0.42),
        (4, 9, 1.08, 0.68),
        (-4, 10, 0.82, 0.5),
        (-10, 5, 1.0, 0.6),
        (-11, -2, 0.72, 0.4),
        (-5, -3, 0.58, 0.34),
        (1, -1, 0.84, 0.58),
        (6, 3, 0.64, 0.4),
    ];
    rsx! {
        div { class: "{orb_class}", aria_label: "Loading",
            for (index, (x, y, scale, alpha)) in dots.into_iter().enumerate() {
                span {
                    key: "{index}",
                    style: "--orb-x: {x}px; --orb-y: {y}px; --orb-scale: {scale}; --orb-alpha: {alpha}; --orb-delay: {-((index as i32) * 84)}ms;"
                }
            }
            i {}
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
