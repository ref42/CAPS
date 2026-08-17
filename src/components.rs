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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiLanguage {
    En,
    Zh,
}

impl UiLanguage {
    pub fn from_code(code: &str) -> Self {
        if code == "zh" { Self::Zh } else { Self::En }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Zh => "zh",
        }
    }
}

fn tr(language: UiLanguage, en: &'static str, zh: &'static str) -> &'static str {
    match language {
        UiLanguage::En => en,
        UiLanguage::Zh => zh,
    }
}

#[component]
pub fn Tabs(
    language: UiLanguage,
    tab: String,
    onsearch: EventHandler<MouseEvent>,
    onqueue: EventHandler<MouseEvent>,
    onpet: EventHandler<MouseEvent>,
    onsettings: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        div { class: "tabs",
            button {
                class: if tab == "search" { "tab active" } else { "tab" },
                onclick: onsearch,
                "{tr(language, \"Search\", \"搜索\")}"
            }
            button {
                class: if tab == "queue" { "tab active" } else { "tab" },
                onclick: onqueue,
                "{tr(language, \"Queue\", \"队列\")}"
            }
            button {
                class: if tab == "pet" { "tab active" } else { "tab" },
                onclick: onpet,
                "{tr(language, \"Pet\", \"宠物\")}"
            }
            button {
                class: if tab == "settings" { "tab active" } else { "tab" },
                onclick: onsettings,
                "{tr(language, \"Settings\", \"设置\")}"
            }
        }
    }
}

#[component]
pub fn PetPanel(
    companion: String,
    coco_style: String,
    dodo_style: String,
    oncompanion: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "panel-section pet-panel",
            div { class: "pet-card-grid",
                button {
                    class: if companion == "coco" { "pet-card active" } else { "pet-card" },
                    onclick: move |_| oncompanion.call("coco".to_string()),
                    div { class: "pet-card-sprite", style: "{coco_style}",
                        div { class: "addon-pet-strip" }
                    }
                    strong { "Coco" }
                }
                button {
                    class: if companion == "dodo" { "pet-card active" } else { "pet-card" },
                    onclick: move |_| oncompanion.call("dodo".to_string()),
                    div { class: "pet-card-sprite", style: "{dodo_style}",
                        div { class: "addon-pet-strip" }
                        }
                    strong { "Dodo" }
                }
            }
        }
    }
}

#[component]
pub fn SearchPanel(
    language: UiLanguage,
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
    onlocal_music_folder: EventHandler<MouseEvent>,
    onload_local: EventHandler<MouseEvent>,
    onrandom_append: EventHandler<u32>,
    onrandom_replace: EventHandler<u32>,
    onrandom_count: EventHandler<u32>,
    onclear_results: EventHandler<()>,
    onadd: EventHandler<Track>,
) -> Element {
    let search_from_key = query.trim().to_string();
    let import_from_key = video_url.trim().to_string();
    let import_from_button = import_from_key.clone();
    let video_source_label = match source {
        SearchSource::Bilibili => "Bilibili",
        SearchSource::Youtube => "YouTube",
        _ => "",
    };
    let video_placeholder = match source {
        SearchSource::Bilibili => tr(
            language,
            "Paste Bilibili video URL",
            "粘贴 Bilibili 视频链接",
        ),
        SearchSource::Youtube => tr(language, "Paste YouTube video URL", "粘贴 YouTube 视频链接"),
        _ => "",
    };
    let search_placeholder = tr(language, "Song, artist, album", "歌曲、歌手、专辑");
    let folder_label = tr(language, "Folder", "文件夹");
    let local_placeholder = tr(language, "path/to/your/audios", "你的音频文件夹");
    let empty_status = if status.trim().is_empty() {
        tr(language, "No results.", "暂无结果。")
    } else {
        status.as_str()
    };
    rsx! {
        div { class: "panel-section",
            div { class: "source-switch",
                button {
                    class: if source == SearchSource::Netease { "source-option active" } else { "source-option" },
                    onclick: move |_| onsource.call(SearchSource::Netease),
                    "Online"
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
                    "{tr(language, \"Local\", \"本地\")}"
                }
            }

            if source == SearchSource::Netease {
                div { class: "source-mode netease-mode",
                    div { class: "search-row",
                        div { class: "search-field",
                            input {
                                value: "{query}",
                                placeholder: "{search_placeholder}",
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
                    div { class: "random-control",
                        input {
                            class: "random-count-input",
                            r#type: "number",
                            min: "1",
                            max: "999",
                            value: "{random_count}",
                            oninput: move |event| {
                                if let Ok(value) = event.value().parse::<u32>() {
                                    onrandom_count.call(value.clamp(1, 999));
                                }
                            },
                        }
                        button {
                            class: "random-add",
                            onclick: move |_| onrandom_append.call(random_count),
                            "{tr(language, \"Append\", \"追加\")}"
                        }
                        button {
                            class: "random-add random-replace",
                            onclick: move |_| onrandom_replace.call(random_count),
                            "{tr(language, \"Replace\", \"替换\")}"
                        }
                        button {
                            class: "random-clear",
                            title: "{tr(language, \"Clear search results\", \"清空搜索结果\")}",
                            onclick: move |_| onclear_results.call(()),
                            "{tr(language, \"Clear\", \"清空\")}"
                        }
                    }
                }
            }

            if source == SearchSource::Bilibili || source == SearchSource::Youtube {
                div { class: "source-mode video-mode",
                    div { class: "search-row",
                        div { class: "search-field no-trailing-icon",
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
                        }
                        button {
                            class: "source-action video-import",
                            onclick: move |_| onimport_video.call((source, import_from_button.clone())),
                            "{tr(language, \"Extract\", \"提取\")}"
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
                        span { "{folder_label}" }
                        button {
                            class: "local-folder-picker",
                            r#type: "button",
                            title: "{local_music_folder}",
                            onfocus,
                            onblur,
                            onclick: onlocal_music_folder,
                            if local_music_folder.trim().is_empty() {
                                "{local_placeholder}"
                            } else {
                                "{local_music_folder}"
                            }
                        }
                        button {
                            r#type: "button",
                            onclick: onload_local,
                            "{tr(language, \"Load\", \"加载\")}"
                        }
                    }
                    div { class: "import-readout",
                        span { "{tr(language, \"Local\", \"本地\")}" }
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
                        div { class: "empty", "{empty_status}" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn QueuePanel(
    language: UiLanguage,
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
                span { "{queue_len} {tr(language, \"tracks\", \"首\")}" }
                button { onclick: onclear, "{tr(language, \"Clear\", \"清空\")}" }
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
                    div { class: "empty", "{tr(language, \"Showing first\", \"仅显示前\")} {QUEUE_RENDER_LIMIT}. {hidden_count} {tr(language, \"more tracks stay in the queue.\", \"首仍在队列中。\")}" }
                }
                if queue_len == 0 {
                    div { class: "empty", "{tr(language, \"Queue is empty.\", \"队列为空。\")}" }
                }
            }
        }
    }
}

#[component]
pub fn SettingsPanel(
    language: UiLanguage,
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
    onlanguage: EventHandler<String>,
    onnormal: EventHandler<MouseEvent>,
    onsilent: EventHandler<MouseEvent>,
    onquiet: EventHandler<MouseEvent>,
    oncheck_update: EventHandler<MouseEvent>,
    oninstall_update: EventHandler<MouseEvent>,
) -> Element {
    let opacity_progress = ((opacity.saturating_sub(10) as f64 / 90.0) * 100.0).clamp(0.0, 100.0);
    let volume_progress = (volume as f64).clamp(0.0, 100.0);
    let island_progress =
        ((island_size.saturating_sub(85) as f64 / 65.0) * 100.0).clamp(0.0, 100.0);
    let show_update_status = !update_status.starts_with("Installed CAPS");
    rsx! {
        div { class: "panel-section settings",
            label { class: "setting",
                span { "{tr(language, \"Opacity\", \"透明度\")}" }
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
                span { "{tr(language, \"Volume\", \"音量\")}" }
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
                span { "{tr(language, \"Island size\", \"岛屿大小\")}" }
                div { class: "setting-control",
                    input {
                        r#type: "range",
                        min: "85",
                        max: "150",
                        value: "{island_size}",
                        style: "--setting-progress: {island_progress:.2}%;",
                        onfocus: move |event| onslider_focus.call(event),
                        onblur: move |event| onslider_blur.call(event),
                        onmousedown: move |event| onslider_down.call(event),
                        onmouseup: move |event| onslider_up.call(event),
                        oninput: move |event| {
                            if let Ok(value) = event.value().parse::<u32>() {
                                onisland_size.call(value.clamp(85, 150));
                            }
                        },
                    }
                    output { "{island_size}%" }
                }
            }
            label { class: "setting",
                span { "{tr(language, \"Mode\", \"模式\")}" }
                div { class: "mode-actions",
                    button {
                        class: if music_mode == MusicMode::Normal { "mode-button normal-button active" } else { "mode-button normal-button" },
                        onclick: onnormal,
                        "{tr(language, \"Normal\", \"普通\")}"
                    }
                    button {
                        class: if music_mode == MusicMode::Silent { "mode-button silent-button active" } else { "mode-button silent-button" },
                        onclick: onsilent,
                        "{tr(language, \"Silent\", \"静音\")}"
                    }
                    button {
                        class: if music_mode == MusicMode::Quiet { "mode-button quiet-button active" } else { "mode-button quiet-button" },
                        onclick: onquiet,
                        "{tr(language, \"Quiet\", \"安静\")}"
                    }
                }
            }
            label { class: "setting",
                span { "{tr(language, \"Language\", \"语言\")}" }
                div { class: "mode-actions language-actions",
                    button {
                        class: if language == UiLanguage::En { "mode-button language-button active" } else { "mode-button language-button" },
                        onclick: move |_| onlanguage.call("en".to_string()),
                        "EN"
                    }
                    button {
                        class: if language == UiLanguage::Zh { "mode-button language-button active" } else { "mode-button language-button" },
                        onclick: move |_| onlanguage.call("zh".to_string()),
                        "中文"
                    }
                }
            }
            label { class: "setting update-setting",
                span { "{tr(language, \"Updates\", \"更新\")}" }
                div { class: "update-inline",
                    if show_update_status {
                        div { class: "update-copy", "{update_status}" }
                    }
                    if let Some(progress) = update_progress {
                        i { style: "--update-progress: {progress:.2}%;" }
                    }
                }
                div { class: "setting-actions update-actions",
                    button {
                        disabled: update_busy,
                        onclick: oncheck_update,
                        "{tr(language, \"Check update\", \"检查更新\")}"
                    }
                    if update_available {
                        button {
                            disabled: update_busy,
                            onclick: oninstall_update,
                            "{tr(language, \"Update\", \"更新\")}"
                        }
                    }
                }
            }
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
    expanded_title_text: String,
    expanded_title_class: &'static str,
    expanded_title_style: String,
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
    onsplit_press_start: EventHandler<MouseEvent>,
    onsplit_press_end: EventHandler<MouseEvent>,
    onprev: EventHandler<MouseEvent>,
    onplaypause: EventHandler<MouseEvent>,
    onnext: EventHandler<MouseEvent>,
    onstop: EventHandler<MouseEvent>,
    onseek: EventHandler<f64>,
) -> Element {
    rsx! {
        section {
            class: "{island_class}",
            onmousedown: move |event| {
                ondrag.call(event.clone());
                onsplit_press_start.call(event);
            },
            onmouseup: move |event| onsplit_press_end.call(event),
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
                    } else {
                        div { class: "music-copy expanded-music-copy",
                            div { class: "expanded-title-viewport", title: "{expanded_title_text}",
                                strong {
                                    class: "{expanded_title_class}",
                                    style: "{expanded_title_style}",
                                    key: "expanded-title-{transition_key}-{expanded_title_text}",
                                    "{expanded_title_text}"
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

#[component]
pub fn AddonIsland(
    companion_style: String,
    companion_name: &'static str,
    separated: bool,
    splitting: bool,
    onhover: EventHandler<MouseEvent>,
) -> Element {
    let class = match (separated, splitting) {
        (true, _) => "addon-island separated",
        (false, true) => "addon-island splitting",
        _ => "addon-island",
    }
    .to_string();
    rsx! {
        aside { class: "{class}", title: "{companion_name}", onmouseenter: move |event| onhover.call(event),
            div { class: "addon-pet", style: "{companion_style}", "aria-label": "{companion_name}",
                div { class: "addon-pet-strip" }
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
    let display_name = search_track_name(&track);
    let duration = track
        .duration
        .map(format_track_duration)
        .unwrap_or_default();
    rsx! {
        button { class: if active { "song active" } else { "song" }, onclick,
            span { class: "song-cover", style: "{cover_style}" }
            span { class: "song-copy",
                strong { "{display_name}" }
                small { "{detail}" }
            }
            if !duration.is_empty() {
                span { class: "track-duration", "{duration}" }
            }
            span { class: "song-action", "{action}" }
        }
    }
}

fn search_track_name(track: &Track) -> String {
    let suffix = match track.source.as_str() {
        crate::track::SOURCE_NETEASE | crate::track::SOURCE_SHITEASE => "netease",
        crate::track::SOURCE_QQMUSIC => "qqmusic",
        crate::track::SOURCE_KUGOU => "kugou",
        _ => return track.name.clone(),
    };
    format!("{} - {suffix}", track.name)
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
            button { class: "remove-song", onclick: move |event| {
                event.stop_propagation();
                onremove.call(event);
            }, title: "Remove", "×" }
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
