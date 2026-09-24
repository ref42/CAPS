use crate::{
    actions,
    audio::{AudioCommand, AudioPlayer, AudioState},
    audio_spectrum,
    components::HoverLine,
    lyrics::{self, LyricLine},
    mode::{MusicMode, PlayOrder},
    motion::Motion,
    storage,
    track::Track,
    updater,
};
use gpui_kit::{
    component::{
        input::{InputEvent, InputState},
        slider::{SliderEvent, SliderState},
    },
    *,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, PartialEq)]
pub enum Source {
    Online,
    Bilibili,
    Youtube,
    Local,
}

pub enum Message {
    Status(u64, String),
    Search(u64, Result<Vec<Track>, String>),
    Queue(Result<Vec<Track>, String>, bool),
    Imported(Result<(Track, String), String>),
    Folder(Option<String>),
    Loaded(u64, Track, Result<String, String>),
    Lyrics(u64, Vec<LyricLine>),
    Cover(u64, Option<Arc<Image>>, Option<(u32, u32)>),
    Thumbnail(String, Option<Arc<Image>>),
    Spin(u64, Arc<RenderImage>),
    Stats(Stats),
    Update(Result<updater::UpdateStatus, String>),
    UpdateProgress(u64, Option<u64>),
    Installed(Result<(), String>),
}

#[derive(Default)]
pub struct Stats {
    pub cpu: f32,
    pub memory: f32,
    pub upload: u64,
    pub download: u64,
}

/// An in-progress shift-drag of the whole window.
///
/// The window is moved by the tick, not by the shell's modal move loop: that
/// loop runs inside GPUI's mouse-down dispatch, so the capsule's frame loop
/// stops for the duration of the drag and never restarts.
#[derive(Clone, Copy)]
pub struct Drag {
    cursor: (f32, f32),
    origin: (f32, f32),
    requested: (f32, f32),
}

/// Minimum gap between repaints. The tick samples state faster than this so
/// hover and drag stay responsive, but painting stays at display rate.
const PAINT_INTERVAL: Duration = Duration::from_millis(15);
const TICK_INTERVAL: Duration = Duration::from_millis(8);
/// How many shuffle jumps "previous" can walk back through.
const SHUFFLE_HISTORY: usize = 64;

/// How many cover thumbnails stay in memory. The queue and the search results
/// share this cache and a row asks for its cover every frame it paints, so a
/// full cache has to drop a cover that is not on screen; see
/// `Caps::cache_thumbnail`.
const THUMBNAIL_CACHE: usize = 128;

/// A cached cover, and when a row last asked for it.
struct Thumbnail {
    image: Arc<Image>,
    used: u64,
}

/// The cover that has gone longest without being asked for.
///
/// A row in view asks for its cover on every frame it paints, so this is what
/// keeps the covers someone can actually see in the cache: whatever it picks has
/// been off screen the longest. Picking a hash map's first key instead is
/// arbitrary, and an evicted cover is re-requested by the next frame that needs
/// it.
fn least_recently_used<'a>(
    entries: impl Iterator<Item = (&'a String, u64)>,
) -> Option<&'a String> {
    entries.min_by_key(|(_, used)| *used).map(|(url, _)| url)
}

/// Which of the two font choices the settings page is currently showing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FontSlot {
    /// Latin and interface text.
    Latin,
    /// Han text: titles, artists and lyrics from Chinese providers.
    Han,
}

/// Faces offered for Latin text: the UI families a Windows desktop is likely to
/// have, in the order a designer would reach for them. Anything the machine is
/// missing is dropped, and the current choice is always kept even if it is not
/// on this list, so a hand-edited state file is never silently overruled.
const LATIN_CANDIDATES: [&str; 20] = [
    "Segoe UI Variable Text",
    "Segoe UI Variable Display",
    "Segoe UI",
    "Arial",
    "Helvetica",
    "Verdana",
    "Tahoma",
    "Calibri",
    "Candara",
    "Corbel",
    "Trebuchet MS",
    "Georgia",
    "Cambria",
    "Times New Roman",
    "Consolas",
    "Cascadia Mono",
    "Inter",
    "Noto Sans",
    "MiSans",
    "HarmonyOS Sans",
];

/// Faces offered for Han text. Only faces that actually carry Han glyphs, and
/// never a weight-named cut ("Microsoft YaHei UI Light", "MiSans Semibold"): the
/// capsule asks for several weights, so a single-cut family would render all of
/// them in one weight.
const HAN_CANDIDATES: [&str; 10] = [
    "Microsoft YaHei UI",
    "Microsoft YaHei",
    "MiSans",
    "Noto Sans SC",
    "Source Han Sans SC",
    "PingFang SC",
    "HarmonyOS Sans SC",
    "DengXian",
    "SimHei",
    "SimSun",
];

/// Weight and optical-size words that make a family a single cut rather than a
/// family with weights. Only used to order the list, never to hide a face.
const WEIGHT_WORDS: [&str; 12] = [
    "thin",
    "extralight",
    "ultralight",
    "light",
    "demilight",
    "normal",
    "medium",
    "demibold",
    "semibold",
    "bold",
    "heavy",
    "black",
];

/// Whether a family name is a single weight cut of another family.
fn is_weight_cut(family: &str) -> bool {
    let lower = family.to_ascii_lowercase();
    family
        .split_whitespace()
        .last()
        .map(|last| WEIGHT_WORDS.contains(&last.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
        || WEIGHT_WORDS
            .iter()
            .any(|word| lower.ends_with(&format!("-{word}")))
}

/// The family to draw Han with, then the stand-ins to try after it. Whatever is
/// named here is only consulted for characters the Latin family cannot draw.
fn han_family_chain(picked: Option<&str>) -> Vec<String> {
    let built_in = crate::components::HAN_FALLBACKS
        .iter()
        .filter(|name| Some(**name) != picked);
    match picked {
        Some(picked) => std::iter::once(picked.to_string())
            .chain(built_in.map(|name| (*name).to_string()))
            .collect(),
        None => built_in.map(|name| (*name).to_string()).collect(),
    }
}

/// Whether a family name looks like a face that carries Han glyphs. Windows
/// reports these names in English, so the Chinese names people actually search
/// for are matched too; the check only decides ordering, never availability.
fn looks_han(family: &str) -> bool {
    const MARKERS: [&str; 26] = [
        "yahei", "simsun", "simhei", "nsimsun", "dengxian", "fangsong", "kaiti", "songti", "song",
        "hei", "ming", "kai", "fang", "misans", "noto sans sc", "noto sans tc", "noto sans cjk",
        "source han", "pingfang", "harmonyos", "思源", "黑体", "宋体", "楷体", "雅黑", "等线",
    ];
    let lower = family.to_ascii_lowercase();
    MARKERS
        .iter()
        .any(|marker| lower.contains(marker) || family.contains(marker))
}

/// Chinese names people search for, with the Latin text Windows uses in the
/// family name. Searching "雅黑" has to find "Microsoft YaHei".
const SEARCH_ALIASES: [(&str, &str); 8] = [
    ("雅黑", "yahei"),
    ("黑体", "hei"),
    ("宋体", "sun"),
    ("楷体", "kai"),
    ("仿宋", "fang"),
    ("等线", "dengxian"),
    ("思源", "source han"),
    ("苹方", "pingfang"),
];

/// Whether `family` satisfies the search box.
fn font_matches(family: &str, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    let name = family.to_ascii_lowercase();
    let needle = query.to_ascii_lowercase();
    if name.contains(&needle) {
        return true;
    }
    SEARCH_ALIASES.iter().any(|(chinese, latin)| {
        (chinese.contains(query) || query.contains(*chinese)) && name.contains(latin)
    })
}

/// Every family the machine has, ordered so the likely choices come first: the
/// recommended faces for this slot, then anything that looks like it covers the
/// right script, then the rest alphabetically. Nothing is hidden: `settings.md
/// > Best practices` asks to minimize *settings*, not the choices within one.
fn font_choices(installed: &[String], slot: FontSlot, current: Option<&str>) -> Vec<String> {
    let recommended: &[&str] = match slot {
        FontSlot::Latin => &LATIN_CANDIDATES,
        FontSlot::Han => &HAN_CANDIDATES,
    };
    let mut ordered: Vec<String> = Vec::with_capacity(installed.len());
    let push = |name: &String, ordered: &mut Vec<String>| {
        if !ordered.iter().any(|have| have == name) {
            ordered.push(name.to_owned());
        }
    };
    for name in recommended {
        if let Some(found) = installed.iter().find(|have| have == name) {
            push(found, &mut ordered);
        }
    }
    for name in installed {
        // Single-cut faces ("MiSans Semibold") are kept in the list but not
        // promoted: the capsule asks for several weights, so a one-cut family is
        // rarely what someone wants as a whole-script choice.
        if looks_han(name) && !is_weight_cut(name) {
            push(name, &mut ordered);
        }
    }
    let mut rest: Vec<&String> = installed
        .iter()
        .filter(|name| !ordered.iter().any(|have| have == *name))
        .collect();
    rest.sort_by_key(|name| (is_weight_cut(name), name.to_ascii_lowercase()));
    for name in rest {
        push(name, &mut ordered);
    }
    // A family the user picked earlier stays available even if the machine no
    // longer reports it, so their choice is never invisible or unselectable.
    if let Some(current) = current
        && !ordered.iter().any(|have| have == current)
    {
        ordered.insert(0, current.to_string());
    }
    ordered
}

pub struct Caps {
    pub font_family: SharedString,
    /// The platform's own UI family, kept so "System default" can be restored.
    pub system_font: SharedString,
    pub latin_fonts: Vec<String>,
    pub han_fonts: Vec<String>,
    /// Indices into the open slot's list that satisfy the search box.
    pub font_matches: Vec<usize>,
    pub font_search: Entity<InputState>,
    /// Resolved Han fallback list, ready for the text system.
    pub han_fallbacks: FontFallbacks,
    pub font_picker: Option<FontSlot>,
    pub font_scroll: UniformListScrollHandle,
    pub settings: storage::AppSettings,
    pub queue: Vec<Track>,
    pub results: Vec<Track>,
    pub current_index: Option<usize>,
    pub current_track: Option<Track>,
    pub player: Arc<AudioPlayer>,
    pub audio: AudioState,
    pub mode: MusicMode,
    /// What plays when the current track ends; chosen in the queue header.
    pub play_order: PlayOrder,
    /// Track indices left behind by shuffle jumps, so "previous" goes back to
    /// what was actually playing rather than to the neighbouring row.
    shuffle_history: Vec<usize>,
    pub status: String,
    pub source: Source,
    pub query: Entity<InputState>,
    pub video: Entity<InputState>,
    pub count: Entity<InputState>,
    pub opacity_slider: Entity<SliderState>,
    pub volume_slider: Entity<SliderState>,
    pub size_slider: Entity<SliderState>,
    pub seek_slider: Entity<SliderState>,
    pub expanded: bool,
    pub expand_motion: Motion,
    pub width_motion: Motion,
    pub hover: bool,
    pub input_focused: bool,
    pub slider_dragging: bool,
    pub dialog_open: bool,
    pub drag: Option<Drag>,
    pub reduce_motion: bool,
    pub leave_at: Option<Instant>,
    /// When the desktop behind the capsule was last sampled. The palette follows
    /// it, so the text stays the opposite of whatever the capsule floats over.
    pub backdrop_at: Option<Instant>,
    pub stats: Stats,
    pub spectrum: [f32; audio_spectrum::SPECTRUM_BANDS],
    pub colors: (u32, u32),
    pub cover: Option<Arc<Image>>,
    pub spinning_cover: Option<Arc<RenderImage>>,
    pub cover_turn: f32,
    last_frame: Instant,
    retired_covers: Vec<Arc<RenderImage>>,
    thumbnails: HashMap<String, Thumbnail>,
    requested_covers: HashSet<String>,
    /// Counts up as covers are asked for, which is what orders the cache.
    thumb_clock: u64,
    pub lyric: String,
    pub outgoing_lyric: String,
    pub lyric_changed: Instant,
    pub pending_update: Option<updater::ReleaseUpdate>,
    pub update_busy: bool,
    pub update_status: String,
    pub update_progress: Option<f32>,
    pub list_scroll: UniformListScrollHandle,
    /// The queue's own handle. Appending a batch of random tracks has to be
    /// visible where it happened — at the end of the list — and the results
    /// list keeps its own position.
    pub queue_scroll: UniformListScrollHandle,
    /// The line the pointer is on, and when it got there. A line of text that
    /// does not fit scrolls while the pointer rests on it.
    pub hover_line: Option<HoverLine>,
    pub marquee_start: Instant,
    lyrics: Vec<LyricLine>,
    play_generation: u64,
    search_generation: u64,
    loading: bool,
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
    persist_at: Option<Instant>,
    last_size: Option<(i32, i32)>,
    last_shape: Option<(i32, i32, i32, i32, i32, i32)>,
    last_scale: f32,
    last_notify: Instant,
    paint_due: bool,
    _subscriptions: Vec<Subscription>,
}

impl Caps {
    pub fn thumbnail(&mut self, url: &str) -> Option<Arc<Image>> {
        self.thumb_clock = self.thumb_clock.wrapping_add(1);
        let clock = self.thumb_clock;
        if let Some(cached) = self.thumbnails.get_mut(url) {
            // Being asked for is what keeps a cover: a row in view asks on every
            // frame it paints, so what gets dropped later is what has been off
            // screen the longest.
            cached.used = clock;
            return Some(cached.image.to_owned());
        }
        if !url.is_empty() && self.requested_covers.insert(url.to_owned()) {
            let url = url.to_owned();
            let tx = self.tx.to_owned();
            tokio::spawn(async move {
                let cover = crate::artwork::load(&url).await;
                let _ = tx.send(Message::Thumbnail(url, cover));
            });
        }
        None
    }
    /// Puts a cover in the cache, dropping whatever has gone longest without
    /// being asked for.
    fn cache_thumbnail(&mut self, url: String, image: Arc<Image>, cx: &mut Context<Self>) {
        if self.thumbnails.len() >= THUMBNAIL_CACHE {
            // The entry to drop is the one nobody is looking at. Picking an
            // arbitrary one — a hash map's first key is arbitrary — dropped
            // covers that were on screen, and because the request was re-armed
            // the row fetched them again: the album art in the queue appeared to
            // blink, once per refetch, for as long as the cache stayed full.
            let oldest = least_recently_used(
                self.thumbnails
                    .iter()
                    .map(|(url, cached)| (url, cached.used)),
            )
            .map(|url| url.to_owned());
            if let Some(key) = oldest {
                if let Some(old) = self.thumbnails.remove(&key) {
                    old.image.remove_asset(cx);
                }
                self.requested_covers.remove(&key);
            }
        }
        self.thumb_clock = self.thumb_clock.wrapping_add(1);
        let used = self.thumb_clock;
        self.thumbnails.insert(url, Thumbnail { image, used });
    }
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let saved = storage::load_state();
        let language = crate::locale::Language::from_code(&saved.settings.language);
        let query =
            cx.new(|cx| InputState::new(window, cx).placeholder(language.search_placeholder()));
        let video =
            cx.new(|cx| InputState::new(window, cx).placeholder(language.video_placeholder()));
        let count = cx.new(|cx| {
            InputState::new(window, cx).default_value(saved.settings.random_count.to_string())
        });
        let opacity_slider = cx.new(|_| {
            SliderState::new()
                // Down to nothing: the capsule's text is plain white whatever the
                // surface is doing, so a fully faded capsule is still readable.
                .min(0.)
                .max(100.)
                .default_value(saved.settings.opacity as f32)
        });
        let volume_slider = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .default_value(saved.settings.volume as f32)
        });
        let size_slider = cx.new(|_| {
            SliderState::new()
                .min(85.)
                .max(150.)
                .default_value(saved.settings.capsule_size as f32)
        });
        let seek_slider = cx.new(|_| SliderState::new().min(0.).max(100.).step(0.1));
        let font_search = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(language.font_search_placeholder())
                .clean_on_escape()
        });
        let (tx, rx) = mpsc::channel();
        start_stats(tx.to_owned());
        let player = Arc::new(AudioPlayer::spawn());
        player.send(AudioCommand::SetVolume(saved.settings.volume as f32 / 100.));
        let mut subscriptions = Vec::new();
        for input in [&query, &video, &count, &font_search] {
            subscriptions.push(cx.subscribe_in(input, window, |this, _, event, _, cx| {
                match event {
                    InputEvent::Focus => {
                        this.input_focused = true;
                        this.leave_at = None;
                    }
                    InputEvent::Blur => {
                        this.input_focused = false;
                        this.leave_at = Some(Instant::now());
                    }
                    _ => {}
                }
                cx.notify();
            }));
        }
        subscriptions.push(cx.subscribe_in(&query, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.search(cx);
            }
        }));
        subscriptions.push(cx.subscribe_in(&video, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.import(cx);
            }
        }));
        // The font search filters as you type, and Enter takes the first match so
        // the list can be driven from the keyboard alone.
        subscriptions.push(cx.subscribe_in(&font_search, window, |this, _, event, _, cx| {
            match event {
                InputEvent::Change => {
                    this.refilter_fonts(cx);
                }
                InputEvent::PressEnter { .. } => {
                    let slot = this.font_picker;
                    if let Some(slot) = slot
                        && let Some(index) = this.font_matches.first().copied()
                    {
                        this.set_font(slot, Some(index), cx);
                    }
                }
                _ => {}
            }
        }));
        for (kind, slider) in [
            (0, &opacity_slider),
            (1, &volume_slider),
            (2, &size_slider),
            (3, &seek_slider),
        ] {
            subscriptions.push(
                cx.subscribe_in(slider, window, move |this, _, event, _, cx| {
                    let value = match event {
                        SliderEvent::Change(v) => {
                            this.slider_dragging = true;
                            v.start()
                        }
                        SliderEvent::Release(v) => {
                            this.slider_dragging = false;
                            this.leave_at = Some(Instant::now());
                            v.start()
                        }
                    };
                    match kind {
                        0 => {
                            this.settings.opacity = value.clamp(0., 100.).round() as u32;
                        }
                        1 => {
                            this.settings.volume = value.round() as u32;
                            this.player.send(AudioCommand::SetVolume(value / 100.));
                        }
                        2 => this.settings.capsule_size = value.round() as u32,
                        _ => this.player.send(AudioCommand::Seek(
                            value as f64 * this.audio.duration / 100.,
                        )),
                    }
                    if kind != 3 {
                        this.persist();
                    }
                    cx.notify();
                }),
            );
        }
        cx.spawn_in(window, async move |view, cx| {
            // Drive state from a timer instead of chaining next-frame callbacks.
            // A dropped frame request can then never wedge the capsule: hover
            // polling, motion, queued playback messages and the resize all keep
            // running, and painting is requested on demand.
            let mut misses = 0u32;
            loop {
                tokio::time::sleep(TICK_INTERVAL).await;
                match view.update_in(cx, |this, window, cx| this.tick(window, cx)) {
                    Ok(()) => misses = 0,
                    // The app is briefly borrowed by a frame on the main thread.
                    // Give up only once the window is genuinely gone.
                    Err(_) => {
                        misses += 1;
                        if misses > 250 {
                            break;
                        }
                    }
                }
            }
        })
        .detach();
        let now = Instant::now();
        let reduce_motion = cx.reduce_motion();
        let scale = saved.settings.capsule_size as f32 / 100.;
        // Reduced motion keeps the same states but drops the travel between
        // them, which is the substitution accessibility.md asks for.
        let (forward, reverse) = if reduce_motion { (0, 0) } else { (260, 150) };
        let system_font = cx
            .global::<gpui_kit::component::Theme>()
            .font_family
            .to_owned();
        // The picker lists what this machine can actually draw, so a family the
        // user chose on another machine cannot be offered here as a dead entry.
        let installed = cx.text_system().all_font_names();
        let latin_fonts = font_choices(&installed, FontSlot::Latin, saved.settings.latin_font.as_deref());
        let han_fonts = font_choices(&installed, FontSlot::Han, saved.settings.han_font.as_deref());
        // Read before `saved.settings` is moved into the struct below.
        let play_order = PlayOrder::from_key(&saved.settings.play_order);
        Self {
            font_family: saved
                .settings
                .latin_font
                .as_deref()
                .map(SharedString::from)
                .unwrap_or_else(|| system_font.to_owned()),
            system_font,
            latin_fonts,
            han_fonts,
            font_matches: Vec::new(),
            font_search,
            han_fallbacks: FontFallbacks::from_fonts(han_family_chain(
                saved.settings.han_font.as_deref(),
            )),
            font_picker: None,
            font_scroll: UniformListScrollHandle::new(),
            settings: saved.settings,
            queue: saved.queue,
            results: Vec::new(),
            current_index: saved.current_index,
            current_track: None,
            player,
            audio: AudioState::default(),
            mode: MusicMode::Silent,
            play_order,
            shuffle_history: Vec::new(),
            status: language.welcome().into(),
            source: Source::Online,
            query,
            video,
            count,
            opacity_slider,
            volume_slider,
            size_slider,
            seek_slider,
            expanded: false,
            expand_motion: Motion::with_durations(0., forward, reverse),
            width_motion: Motion::with_durations(crate::windowing::COLLAPSED_W, forward, reverse),
            hover: false,
            input_focused: false,
            slider_dragging: false,
            dialog_open: false,
            drag: None,
            reduce_motion,
            leave_at: None,
            backdrop_at: None,
            stats: Stats::default(),
            spectrum: [0.08; audio_spectrum::SPECTRUM_BANDS],
            colors: (0x7df2ca, 0x34c759),
            cover: None,
            spinning_cover: None,
            cover_turn: 0.,
            last_frame: now,
            retired_covers: Vec::new(),
            thumbnails: HashMap::new(),
            requested_covers: HashSet::new(),
            thumb_clock: 0,
            lyric: String::new(),
            outgoing_lyric: String::new(),
            lyric_changed: now,
            pending_update: None,
            update_busy: false,
            update_status: format!("CAPS {}", updater::current_version()),
            update_progress: None,
            list_scroll: UniformListScrollHandle::new(),
            queue_scroll: UniformListScrollHandle::new(),
            hover_line: None,
            marquee_start: Instant::now(),
            lyrics: Vec::new(),
            play_generation: 0,
            search_generation: 0,
            loading: false,
            tx,
            rx,
            persist_at: None,
            last_size: None,
            last_shape: None,
            last_scale: scale,
            last_notify: now,
            paint_due: true,
            _subscriptions: subscriptions,
        }
    }

    pub fn tr(&self, en: &'static str, zh: &'static str) -> &'static str {
        if self.settings.language == "zh" {
            zh
        } else {
            en
        }
    }
    /// Localized text that has to be built at runtime. Both sentences are
    /// spelled out in full rather than assembled from fragments, so the two
    /// languages are free to order their words differently.
    pub fn say(&self, en: String, zh: String) -> String {
        if self.settings.language == "zh" { zh } else { en }
    }

    /// The family Latin runs are drawn in: the user's pick, or the platform's
    /// own UI face.
    pub fn latin_family(&self) -> SharedString {
        self.settings
            .latin_font
            .as_deref()
            .map(SharedString::from)
            .unwrap_or_else(|| self.system_font.to_owned())
    }

    /// The Han family, in preference order: the user's pick first, then the
    /// platform companion and the usual stand-ins. Whatever is named here is
    /// only consulted for characters the Latin family cannot draw.
    pub fn han_families(&self) -> Vec<String> {
        han_family_chain(self.settings.han_font.as_deref())
    }

    /// The same list, ready for the text system. Kept resolved so painting a
    /// frame does not rebuild it.
    fn resolve_han_fallbacks(&mut self) {
        self.han_fallbacks =
            FontFallbacks::from_fonts(han_family_chain(self.settings.han_font.as_deref()));
    }

    /// The families offered for a slot.
    pub fn font_choices(&self, slot: FontSlot) -> &[String] {
        match slot {
            FontSlot::Latin => &self.latin_fonts,
            FontSlot::Han => &self.han_fonts,
        }
    }

    /// The family at `index` for `slot`, if it is one of the offered ones.
    pub fn font_at(&self, slot: FontSlot, index: usize) -> Option<&str> {
        self.font_choices(slot).get(index).map(String::as_str)
    }

    /// The families that satisfy the current search text, as indices into the
    /// open slot's list.
    pub fn refilter_fonts(&mut self, cx: &mut Context<Self>) {
        let Some(slot) = self.font_picker else {
            self.font_matches.clear();
            return;
        };
        let query = self.font_search.read(cx).value();
        self.font_matches = self
            .font_choices(slot)
            .iter()
            .enumerate()
            .filter(|(_, name)| font_matches(name, &query))
            .map(|(index, _)| index)
            .collect();
        cx.notify();
    }

    /// Whether a family is a real, installed one right now. Used to mark a
    /// stored choice that has since been uninstalled.
    pub fn font_installed(&self, family: &str, slot: FontSlot) -> bool {
        let list = match slot {
            FontSlot::Latin => &self.latin_fonts,
            FontSlot::Han => &self.han_fonts,
        };
        list.iter().any(|name| name == family)
    }

    /// Choose a family for `slot`, or `None` for the platform default.
    pub fn set_font(&mut self, slot: FontSlot, index: Option<usize>, cx: &mut Context<Self>) {
        let stored = index
            .and_then(|index| self.font_at(slot, index))
            .map(str::to_owned);
        match slot {
            FontSlot::Latin => {
                self.settings.latin_font = stored;
                self.font_family = self.latin_family();
            }
            FontSlot::Han => {
                self.settings.han_font = stored;
                self.resolve_han_fallbacks();
            }
        }
        self.font_picker = None;
        self.persist();
        cx.notify();
    }
    pub fn set_language(&mut self, code: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.language == code {
            return;
        }
        self.settings.language = code.into();
        let language = crate::locale::Language::from_code(code);
        self.query.update(cx, |input, cx| {
            input.set_placeholder(language.search_placeholder(), window, cx);
        });
        self.video.update(cx, |input, cx| {
            input.set_placeholder(language.video_placeholder(), window, cx);
        });
        gpui_kit::component::set_locale(code);
        self.status = language.welcome().into();
        self.update_status = format!("CAPS {}", updater::current_version());
        self.persist();
        cx.notify();
    }
    pub fn persist(&mut self) {
        self.persist_at = Some(Instant::now());
    }
    fn save(&self) {
        storage::save_state_parts(self.settings.to_owned(), &self.queue, self.current_index);
    }
    pub fn has_music(&self) -> bool {
        self.current_track.is_some() && self.mode == MusicMode::Normal
    }

    /// Whether the pointer is over the capsule. Returns whether that changed.
    pub fn set_hover(&mut self, hovered: bool) -> bool {
        if self.hover == hovered {
            return false;
        }
        self.hover = hovered;
        if hovered {
            self.expanded = true;
            self.leave_at = None;
        } else {
            self.leave_at = Some(Instant::now());
        }
        true
    }

    /// Start moving the window with the pointer. Called on shift + left click.
    pub fn begin_drag(&mut self, window: &Window) {
        let (Some(cursor), Some(origin)) = (
            crate::windowing::cursor_position(),
            crate::windowing::window_origin(window),
        ) else {
            return;
        };
        self.drag = Some(Drag {
            cursor,
            origin,
            requested: origin,
        });
        self.leave_at = None;
    }

    /// Stop moving the window and remember where it landed.
    pub fn end_drag(&mut self, window: &Window) {
        if self.drag.take().is_none() {
            return;
        }
        if let Some(origin) = crate::windowing::window_origin(window) {
            let dpi = window.scale_factor().max(0.1);
            self.settings.window_position = Some((origin.0 / dpi, origin.1 / dpi));
        }
        self.leave_at = Some(Instant::now());
        self.persist();
    }

    /// While a drag is live, follow the pointer. The new position is always
    /// derived from the position and cursor captured at mouse-down, so a slow
    /// frame or a missed move can never accumulate drift.
    fn follow_drag(&mut self, window: &Window, cx: &App) {
        let Some(mut drag) = self.drag else {
            return;
        };
        if !crate::windowing::left_button_down() {
            self.end_drag(window);
            return;
        }
        let Some(cursor) = crate::windowing::cursor_position() else {
            return;
        };
        let target = (
            drag.origin.0 + cursor.0 - drag.cursor.0,
            drag.origin.1 + cursor.1 - drag.cursor.1,
        );
        // A drag follows the pointer, and the pointer can be taken to the very
        // edge of the desktop: without this the capsule could be parked off the
        // screen, where it could never be grabbed again. The clamp is against the
        // display the pointer is on, so dragging across to another monitor still
        // works.
        let target = match crate::windowing::window_size(window) {
            Some(size) => crate::windowing::clamp_to_monitor(target, size, cursor),
            None => target,
        };
        if (target.0 - drag.requested.0).abs() < 1. && (target.1 - drag.requested.1).abs() < 1. {
            return;
        }
        drag.requested = target;
        self.drag = Some(drag);
        crate::windowing::move_window(window, cx, target);
    }

    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        let mut dirty = false;
        for image in self.retired_covers.drain(..) {
            let _ = window.drop_image(image);
        }
        if self.audio.is_playing && !self.reduce_motion {
            self.cover_turn = (self.cover_turn
                + now.duration_since(self.last_frame).as_secs_f32() / crate::artwork::SPIN_SECONDS)
                % 1.;
            dirty = true;
        }
        self.last_frame = now;
        while let Ok(message) = self.rx.try_recv() {
            self.receive(message, cx);
            dirty = true;
        }
        // The window has no native title bar and is shaped by hit testing rather
        // than by a region, so the pointer is sampled here every tick. WM_MOUSELEAVE
        // is unreliable for a resized popup, and this also keeps working while a
        // drag has captured the mouse.
        let expansion = self.expand_motion.value(now);
        let width = self.width_motion.value(now);
        self.publish_geometry(window, expansion, width);
        // Keep the whole window on the display it is on. A resize is applied a
        // frame after it is asked for, so the window can briefly be smaller than
        // the paint it holds — and a window that has just grown at the edge of
        // the screen, because the size control moved or because the panel opened
        // low on the screen, would otherwise hang its far side off the display.
        // Clamping against the real rectangle rather than the requested one also
        // accounts for the frame around the window that GPUI adds.
        if let Some((left, top, right, bottom)) = crate::windowing::window_rect(window) {
            let target = crate::windowing::clamp_to_monitor(
                (left, top),
                (right - left, bottom - top),
                (left, top),
            );
            if (target.0 - left).abs() > 0.5 || (target.1 - top).abs() > 0.5 {
                crate::windowing::move_window(window, cx, target);
            }
        }
        // What the capsule is floating over decides whether its text is white or
        // near-black. A few times a second is plenty — a desktop changes when
        // the user changes it, not per frame — and never while the capsule is
        // moving, so opening and folding the panel is not interrupted by a
        // screen read.
        let animating = self.expand_motion.animating(now) || self.width_motion.animating(now);
        if !animating
            && self
                .backdrop_at
                .is_none_or(|at| now.duration_since(at) > Duration::from_millis(500))
        {
            self.backdrop_at = Some(now);
            if let Some(samples) = crate::windowing::sample_backdrop(window) {
                if let Some(light) =
                    crate::components::set_backdrop(&samples, self.settings.opacity as f32 / 100.)
                {
                    crate::components::sync_theme(light, window, cx);
                    dirty = true;
                }
            }
        }
        if let Some(inside) = crate::windowing::pointer_inside(window) {
            dirty |= self.set_hover(inside);
        }
        if self.drag.is_some() {
            self.follow_drag(window, cx);
            dirty = true;
        }
        if crate::motion::should_collapse(
            self.hover,
            self.drag.is_some() || self.slider_dragging,
            self.dialog_open,
            self.leave_at,
            now,
        ) {
            self.expanded = false;
            self.leave_at = None;
            dirty = true;
            if self.input_focused {
                // A focused search field must not pin the panel open forever.
                window.blur(cx);
                self.input_focused = false;
            }
        }
        dirty |= self
            .expand_motion
            .target(if self.expanded { 1. } else { 0. }, now);
        dirty |= self.width_motion.target(
            if self.expanded || self.has_music() {
                crate::windowing::EXPANDED_W
            } else {
                crate::windowing::COLLAPSED_W
            },
            now,
        );
        // A motion that is mid-flight needs a frame even though its target has
        // not changed since the last tick.
        dirty |= self.expand_motion.animating(now) || self.width_motion.animating(now);
        self.audio = self.player.get_state();
        if self.audio.is_finished && !self.loading && self.current_track.is_some() {
            match self.play_order {
                // Repeat-one reloads the track: a streamed URL is not worth
                // rewinding, and a local file is cheap to open again.
                PlayOrder::RepeatOne => {
                    if let Some(index) = self.current_index {
                        self.play(index);
                    }
                }
                _ => self.step(1),
            }
        }
        if self.audio.is_playing {
            dirty = true;
        }
        // A line of text that is scrolling needs frames of its own, and only the
        // line under the pointer can start one.
        if self.marquee_running() {
            dirty = true;
        }
        let spectrum = audio_spectrum::get_audio_spectrum();
        for (value, target) in self.spectrum.iter_mut().zip(spectrum) {
            let goal = target.max(0.06);
            if (*value - goal).abs() > 0.002 {
                dirty = true;
            }
            *value += (goal - *value) * 0.22;
        }
        let text =
            lyrics::current_lyric_line(&self.lyrics, self.audio.position).unwrap_or_else(|| {
                self.current_track
                    .as_ref()
                    .map(|t| t.name.to_owned())
                    .unwrap_or_default()
            });
        if self.lyric != text {
            self.outgoing_lyric = std::mem::replace(&mut self.lyric, text);
            self.lyric_changed = now;
        }
        if self.lyric_changed.elapsed() < Duration::from_millis(500) {
            dirty = true;
        }
        if !self.slider_dragging {
            let progress = if self.audio.duration > 0. {
                (self.audio.position / self.audio.duration * 100.) as f32
            } else {
                0.
            };
            self.seek_slider
                .update(cx, |slider, cx| slider.set_value(progress, window, cx));
        }
        if self
            .persist_at
            .is_some_and(|t| now.duration_since(t) > Duration::from_millis(350))
        {
            self.save();
            self.persist_at = None;
        }
        dirty |= self.resize_to_content(window, cx, expansion, width);
        self.reshape(window, now, expansion, width);
        if dirty {
            self.paint_due = true;
        }
        // Painting is coalesced to display rate even though state is sampled
        // faster, so an animating capsule cannot outrun the compositor.
        if self.paint_due && now.duration_since(self.last_notify) >= PAINT_INTERVAL {
            self.paint_due = false;
            self.last_notify = now;
            cx.notify();
        }
    }

    /// The painted capsules in the window's own device pixels. Hit testing reads
    /// exactly these numbers, so click-through always matches the last paint.
    fn painted_geometry(&self, window: &Window, expansion: f32, width: f32) -> crate::windowing::Geometry {
        let unit = self.settings.capsule_size as f32 / 100. * window.scale_factor();
        crate::windowing::Geometry {
            unit,
            bleed: crate::windowing::BLEED * unit,
            width: width * unit,
            header: crate::windowing::header_height(expansion) * unit,
            panel: crate::windowing::panel_height(expansion) * unit,
            panel_top: crate::windowing::panel_top(expansion) * unit,
            panel_radius: crate::windowing::PANEL_RADIUS * unit,
        }
    }

    fn publish_geometry(&self, window: &Window, expansion: f32, width: f32) {
        crate::windowing::publish_geometry(self.painted_geometry(window, expansion, width));
    }

    /// Hand the painted shape to the system, but only when it actually moved.
    /// The region has to follow the expansion animation, or the panel would be
    /// cut off while it opens.
    fn reshape(&mut self, window: &Window, now: Instant, expansion: f32, width: f32) {
        let animating = self.expand_motion.animating(now) || self.width_motion.animating(now);
        // A little room while the capsule moves, added evenly on all four sides,
        // so the mask never trails the paint it is clipping.
        let slack = if animating {
            crate::windowing::REGION_LEAD
        } else {
            0.
        };
        let geometry = self.painted_geometry(window, expansion, width);
        let key = (
            geometry.width.round() as i32,
            geometry.header.round() as i32,
            geometry.panel.round() as i32,
            geometry.panel_top.round() as i32,
            (geometry.unit * 1000.).round() as i32,
            (slack * 100.).round() as i32,
        );
        if self.last_shape == Some(key) {
            return;
        }
        self.last_shape = Some(key);
        crate::windowing::apply_region(window, geometry, slack);
    }

    /// Resize the popup so it always has room for the painted capsules. Returns
    /// whether the window changed.
    fn resize_to_content(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        expansion: f32,
        width: f32,
    ) -> bool {
        let scale = self.settings.capsule_size as f32 / 100.;
        let wanted = size(
            px(crate::windowing::window_width(width) * scale),
            px(crate::windowing::window_height(expansion) * scale),
        );
        let key = (
            wanted.width.as_f32().ceil() as i32,
            wanted.height.as_f32().ceil() as i32,
        );
        let changed = self.last_size != Some(key);
        if changed {
            window.resize(wanted);
            self.last_size = Some(key);
        }
        // Changing the capsule size keeps the capsule's centre pinned, instead of
        // sliding it across the desktop as the transparent margin grows. Keeping
        // the window itself on the display is the tick's job, where its real
        // rectangle is known.
        let delta = scale - self.last_scale;
        if delta != 0. {
            self.last_scale = scale;
            if let Some(origin) = crate::windowing::window_origin(window) {
                let dpi = window.scale_factor();
                let shift = (
                    -(crate::windowing::BLEED + width * 0.5) * delta * dpi,
                    -crate::windowing::BLEED * delta * dpi,
                );
                crate::windowing::move_window(window, cx, (origin.0 + shift.0, origin.1 + shift.1));
            }
        }
        changed
    }

    fn receive(&mut self, message: Message, cx: &mut Context<Self>) {
        match message {
            Message::Status(id, text) if id == self.play_generation => self.status = text,
            Message::Search(id, result) if id == self.search_generation => match result {
                Ok(tracks) => {
                    let count = tracks.len();
                    self.status = self.say(
                        format!("Found {count} tracks."),
                        format!("找到 {count} 首歌曲。"),
                    );
                    self.results = tracks;
                }
                Err(err) => {
                    self.status = err;
                    self.results.clear();
                }
            },
            Message::Queue(result, replace) => match result {
                Ok(tracks) => {
                    if replace {
                        self.stop();
                        self.queue.clear();
                    }
                    let added = actions::append_unique_tracks(&mut self.queue, tracks);
                    let total = self.queue.len();
                    // New tracks go on the end, so that is where the list is
                    // left: a batch that arrives out of sight reads as if it
                    // never arrived.
                    if added > 0 {
                        self.queue_scroll.scroll_to_bottom();
                    }
                    self.status = self.say(
                        format!("Added {added} tracks. Queue has {total}."),
                        format!("已添加 {added} 首，队列共 {total} 首。"),
                    );
                    self.persist();
                }
                Err(err) => self.status = err,
            },
            Message::Imported(result) => match result {
                Ok((track, detail)) => {
                    let added = actions::append_unique_tracks(&mut self.queue, [track]);
                    if added > 0 {
                        self.queue_scroll.scroll_to_bottom();
                    }
                    self.status = self.say(
                        format!("Added {added}. {detail}"),
                        format!("已添加 {added} 首。{detail}"),
                    );
                    self.persist();
                }
                Err(err) => self.status = err,
            },
            Message::Folder(folder) => {
                self.dialog_open = false;
                if let Some(folder) = folder {
                    self.settings.local_music_folder = folder;
                    self.persist();
                    self.load_local();
                }
            }
            Message::Loaded(id, track, result) if id == self.play_generation => {
                self.loading = false;
                match result {
                    Ok(path) => {
                        self.player.send(AudioCommand::LoadFile {
                            path,
                            title: track.name.to_owned(),
                            detail: track.artist.to_owned(),
                            duration: track.duration.map(|d| d as f64),
                        });
                        let name = track.name.to_owned();
                        self.status = self
                            .say(format!("Playing {name}."), format!("正在播放 {name}。"));
                        self.current_track = Some(track);
                    }
                    Err(err) => {
                        self.stop();
                        self.status = err;
                    }
                }
            }
            Message::Lyrics(id, lines) if id == self.play_generation => self.lyrics = lines,
            Message::Cover(id, cover, colors) if id == self.play_generation => {
                if let Some(cover) = cover {
                    if let Some(track) = self.current_track.to_owned() {
                        self.cache_thumbnail(track.cover.to_owned(), cover.to_owned(), cx);
                    }
                    self.cover = Some(cover);
                }
                if let Some(colors) = colors {
                    self.colors = colors;
                }
            }
            Message::Thumbnail(url, Some(cover)) => self.cache_thumbnail(url, cover, cx),
            Message::Spin(id, image) if id == self.play_generation => {
                if let Some(old) = self.spinning_cover.replace(image) {
                    self.retired_covers.push(old);
                }
            }
            Message::Stats(stats) => self.stats = stats,
            Message::Update(result) => {
                self.update_busy = false;
                match result {
                    Ok(updater::UpdateStatus::Available(update)) => {
                        let latest = update.latest.to_owned();
                        self.update_status = self.say(
                            format!("CAPS {latest} available"),
                            format!("CAPS {latest} 可更新"),
                        );
                        self.pending_update = Some(update);
                    }
                    Ok(_) => {
                        self.update_status = self.tr("Already up to date.", "已是最新版本。").into()
                    }
                    Err(err) => self.update_status = err,
                }
            }
            Message::UpdateProgress(bytes, total) => {
                self.update_progress = total.filter(|t| *t > 0).map(|t| bytes as f32 / t as f32);
                let size = crate::formatting::format_bytes(bytes);
                self.update_status = self.say(
                    format!("Downloading {size}"),
                    format!("正在下载 {size}"),
                );
            }
            Message::Installed(result) => {
                self.update_busy = false;
                match result {
                    Ok(()) => cx.quit(),
                    Err(err) => self.update_status = err,
                }
            }
            _ => {}
        }
    }

    pub fn search(&mut self, cx: &mut Context<Self>) {
        self.search_generation += 1;
        let id = self.search_generation;
        let text = self.query.read(cx).value().to_string();
        let tx = self.tx.to_owned();
        self.status = self.tr("Searching online…", "正在搜索…").into();
        tokio::spawn(async move {
            let _ = tx.send(Message::Search(id, actions::search(text).await));
        });
    }
    pub fn random(&mut self, replace: bool, cx: &mut Context<Self>) {
        let Ok(count) = self.count.read(cx).value().parse::<u32>() else {
            self.status = self
                .tr("Enter a count from 1 to 999.", "请输入 1 到 999。")
                .into();
            return;
        };
        let count = count.clamp(1, 999);
        self.settings.random_count = count;
        self.persist();
        let tx = self.tx.to_owned();
        self.status = self.say(
            format!("Loading {count} random tracks…"),
            format!("正在加载 {count} 首随机歌曲…"),
        );
        tokio::spawn(async move {
            let _ = tx.send(Message::Queue(actions::random(count).await, replace));
        });
    }
    pub fn import(&mut self, cx: &mut Context<Self>) {
        let source = match self.source {
            Source::Bilibili => actions::VideoImportSource::Bilibili,
            Source::Youtube => actions::VideoImportSource::Youtube,
            _ => return,
        };
        let url = self.video.read(cx).value().trim().to_string();
        let tx = self.tx.to_owned();
        self.status = self.say(
            format!("Importing {}…", source.label()),
            format!("正在导入 {}…", source.label()),
        );
        tokio::spawn(async move {
            let _ = tx.send(Message::Imported(actions::import_video(source, url).await));
        });
    }
    pub fn pick_folder(&mut self) {
        if self.dialog_open {
            return;
        }
        self.dialog_open = true;
        let folder = self.settings.local_music_folder.to_owned();
        let tx = self.tx.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut dialog = rfd::FileDialog::new().set_title("Choose music folder");
            if !folder.is_empty() {
                dialog = dialog.set_directory(folder);
            }
            let _ = tx.send(Message::Folder(
                dialog
                    .pick_folder()
                    .map(|p| p.to_string_lossy().into_owned()),
            ));
        });
    }
    pub fn load_local(&mut self) {
        let folder = self.settings.local_music_folder.to_owned();
        let tx = self.tx.to_owned();
        self.status = self.tr("Loading local audio…", "正在加载本地音乐…").into();
        tokio::task::spawn_blocking(move || {
            if let Err(err) = crate::local_music::load_all_batched(&folder, 80, |batch, _| {
                tx.send(Message::Queue(Ok(batch), false)).is_ok()
            }) {
                let _ = tx.send(Message::Queue(Err(err), false));
            }
        });
    }
    pub fn play(&mut self, index: usize) {
        let Some(track) = self.queue.get(index).cloned() else {
            return;
        };
        self.play_generation += 1;
        let id = self.play_generation;
        self.current_index = Some(index);
        self.current_track = Some(track.to_owned());
        self.mode = MusicMode::Normal;
        self.lyrics.clear();
        self.cover = None;
        self.cover_turn = 0.;
        if let Some(old) = self.spinning_cover.take() {
            self.retired_covers.push(old);
        }
        self.colors = (0x7df2ca, 0x34c759);
        self.loading = true;
        self.player.send(AudioCommand::Stop);
        let name = track.name.to_owned();
        self.status = self.say(format!("Loading {name}…"), format!("正在加载 {name}…"));
        self.persist();
        let tx = self.tx.to_owned();
        let progress_tx = tx.to_owned();
        let audio_track = track.to_owned();
        tokio::spawn(async move {
            let progress = actions::Progress::new(move |text| {
                let _ = progress_tx.send(Message::Status(id, text));
            });
            let result = actions::load_track_path(&audio_track, progress).await;
            let _ = tx.send(Message::Loaded(id, audio_track, result));
        });
        let tx = self.tx.to_owned();
        let lyric_track = track.to_owned();
        tokio::spawn(async move {
            let _ = tx.send(Message::Lyrics(
                id,
                actions::load_track_lyrics(&lyric_track).await,
            ));
        });
        let tx = self.tx.to_owned();
        let color_tx = tx.to_owned();
        let cover_url = track.cover.to_owned();
        tokio::spawn(async move {
            let colors = crate::album_color::spectrum_colors(cover_url)
                .await
                .map(|(a, b)| (parse_rgb(&a), parse_rgb(&b)));
            let _ = color_tx.send(Message::Cover(id, None, colors));
        });
        let cached = self
            .thumbnails
            .get(&track.cover)
            .map(|cached| cached.image.to_owned());
        let spin = !self.reduce_motion;
        tokio::spawn(async move {
            let cover = match cached {
                Some(cover) => Some(cover),
                None => crate::artwork::load(&track.cover).await,
            };
            let _ = tx.send(Message::Cover(id, cover.to_owned(), None));
            // Reduced motion keeps the still artwork instead of paying for a
            // pre-rendered rotation nobody will see.
            if spin
                && let Some(cover) = cover
                && let Ok(Some(spin)) =
                    tokio::task::spawn_blocking(move || crate::artwork::spin_frames(&cover)).await
            {
                let _ = tx.send(Message::Spin(id, spin));
            }
        });
    }
    pub fn stop(&mut self) {
        if let Some(old) = self.spinning_cover.take() {
            self.retired_covers.push(old);
        }
        self.play_generation += 1;
        self.player.send(AudioCommand::Stop);
        self.current_track = None;
        self.current_index = None;
        self.lyrics.clear();
        self.loading = false;
        self.mode = MusicMode::Silent;
        self.cover = None;
        self.persist();
    }
    pub fn step(&mut self, direction: isize) {
        if self.queue.is_empty() {
            self.stop();
            return;
        }
        let len = self.queue.len();
        let current = self.current_index.unwrap_or(0);
        // Shuffle jumps somewhere else in the queue and remembers where it came
        // from; every other order walks the rows, which is what makes the queue
        // panel readable as a running order.
        if self.play_order == PlayOrder::Shuffle && len > 1 {
            if direction > 0 {
                self.shuffle_history.push(current);
                if self.shuffle_history.len() > SHUFFLE_HISTORY {
                    self.shuffle_history.remove(0);
                }
                let next = self.random_other_than(current);
                self.play(next);
                return;
            }
            if let Some(previous) = self.shuffle_history.pop() {
                self.play(previous);
                return;
            }
        }
        let index = current as isize + direction;
        let next = if self.play_order == PlayOrder::Sequential {
            if index < 0 || index as usize >= len {
                // The end of the queue in play-in-order means the music stops
                // rather than jumping back to the top.
                self.stop();
                return;
            }
            index as usize
        } else {
            index.rem_euclid(len as isize) as usize
        };
        self.play(next);
    }

    /// A queue position other than `current`, picked from the clock.
    ///
    /// There is no sequence to reproduce here, so the pick is seeded per call
    /// instead of keeping a generator: every track but the current one stays
    /// reachable, which is what a two-track queue needs to alternate.
    fn random_other_than(&self, current: usize) -> usize {
        let len = self.queue.len();
        if len < 2 {
            return current;
        }
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos() as usize)
            .unwrap_or(0);
        (current + 1 + seed % (len - 1)) % len
    }

    /// Move to the next play order and remember it. Chosen in the queue header.
    pub fn cycle_play_order(&mut self) {
        self.play_order = self.play_order.next();
        self.settings.play_order = self.play_order.key().to_string();
        self.persist();
    }
    pub fn remove(&mut self, index: usize) {
        if index >= self.queue.len() {
            return;
        }
        self.queue.remove(index);
        match self.current_index {
            Some(i) if i == index => self.stop(),
            Some(i) if i > index => self.current_index = Some(i - 1),
            _ => {}
        }
        self.persist();
    }
    /// Records which line of text the pointer is on, so it can scroll if the box
    /// is too narrow for it. Returns whether that changed, which is the only time
    /// a repaint is needed for it.
    pub fn set_line_hover(&mut self, line: HoverLine, entered: bool) -> bool {
        if entered {
            if self.hover_line == Some(line) {
                return false;
            }
            self.hover_line = Some(line);
            // Start from the beginning again: moving along a queue should show
            // each line's opening words, not wherever the last one had got to.
            self.marquee_start = Instant::now();
            true
        } else if self.hover_line == Some(line) {
            self.hover_line = None;
            true
        } else {
            false
        }
    }

    /// How far a line has scrolled this frame, in points. Zero unless the
    /// pointer is on it and the last paint found it too wide for its box.
    pub fn marquee_offset(&self, line: HoverLine, slot: usize, s: f32) -> f32 {
        if self.hover_line != Some(line) {
            return 0.;
        }
        crate::components::marquee_phase(
            Instant::now().duration_since(self.marquee_start),
            crate::components::line_travel(slot),
            crate::components::MARQUEE_SPEED * s,
        )
    }

    /// Whether a line the pointer is on is scrolling. The tick keeps frames
    /// coming while this holds, which is the only reason a still pointer costs
    /// anything at all.
    pub fn marquee_running(&self) -> bool {
        match self.hover_line {
            Some(HoverLine::Capsule) => {
                crate::components::line_overflows(crate::components::TITLE_SLOT)
                    || crate::components::line_overflows(crate::components::ARTIST_SLOT)
            }
            Some(HoverLine::Row(..)) => {
                crate::components::line_overflows(crate::components::NAME_SLOT)
                    || crate::components::line_overflows(crate::components::ROW_SLOT)
            }
            None => false,
        }
    }

    pub fn check_update(&mut self) {
        if self.update_busy {
            return;
        }
        self.update_busy = true;
        self.pending_update = None;
        self.update_progress = None;
        self.update_status = self.tr("Checking updates…", "正在检查更新…").into();
        let tx = self.tx.to_owned();
        tokio::spawn(async move {
            let _ = tx.send(Message::Update(updater::check_latest_release().await));
        });
    }
    pub fn install_update(&mut self) {
        if self.update_busy {
            return;
        }
        let Some(update) = self.pending_update.to_owned() else {
            return;
        };
        self.update_busy = true;
        let tx = self.tx.to_owned();
        let progress = tx.to_owned();
        tokio::spawn(async move {
            let result = updater::download_and_install_update(update, move |bytes, total| {
                let _ = progress.send(Message::UpdateProgress(bytes, total));
            })
            .await;
            let _ = tx.send(Message::Installed(result));
        });
    }
}

impl Drop for Caps {
    fn drop(&mut self) {
        self.save();
        self.player.send(AudioCommand::Stop);
    }
}

fn start_stats(tx: mpsc::Sender<Message>) {
    std::thread::spawn(move || {
        let mut system = sysinfo::System::new();
        let mut networks = sysinfo::Networks::new_with_refreshed_list();
        let mut last = Instant::now();
        loop {
            system.refresh_cpu_usage();
            system.refresh_memory();
            networks.refresh(true);
            let seconds = last.elapsed().as_secs_f64().max(0.1);
            last = Instant::now();
            let upload = networks.values().map(|n| n.transmitted()).sum::<u64>();
            let download = networks.values().map(|n| n.received()).sum::<u64>();
            if tx
                .send(Message::Stats(Stats {
                    cpu: system.global_cpu_usage(),
                    memory: system.used_memory() as f32 / system.total_memory().max(1) as f32
                        * 100.,
                    upload: (upload as f64 / seconds) as u64,
                    download: (download as f64 / seconds) as u64,
                }))
                .is_err()
            {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

fn parse_rgb(text: &str) -> u32 {
    let parts: Vec<u32> = text
        .trim_start_matches("rgb(")
        .trim_end_matches(')')
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    if parts.len() == 3 {
        (parts[0] << 16) | (parts[1] << 8) | parts[2]
    } else {
        0x7df2ca
    }
}

#[cfg(test)]
mod tests {
    // The toolkit's glob would shadow the built-in `#[test]`, so the helpers
    // under test are named explicitly.
    use super::{FontSlot, font_choices, font_matches, least_recently_used};

    #[test]
    fn the_thumbnail_cache_drops_the_cover_asked_for_longest_ago() {
        let cached = [
            ("in view".to_string(), 9u64),
            ("off screen".to_string(), 2),
            ("just scrolled past".to_string(), 5),
        ];
        let dropped = least_recently_used(cached.iter().map(|(url, used)| (url, *used)));
        assert_eq!(dropped.map(String::as_str), Some("off screen"));
    }

    #[test]
    fn an_empty_thumbnail_cache_drops_nothing() {
        let cached: [(String, u64); 0] = [];
        assert!(least_recently_used(cached.iter().map(|(url, used)| (url, *used))).is_none());
    }

    fn installed() -> Vec<String> {
        [
            "Segoe UI",
            "Arial",
            "Georgia",
            "Microsoft YaHei UI",
            "MiSans",
            "MiSans Semibold",
            "SimSun",
            "Unrelated Face",
        ]
        .iter()
        .map(|name| (*name).to_string())
        .collect()
    }

    fn rank(choices: &[String], name: &str) -> usize {
        choices
            .iter()
            .position(|have| have == name)
            .unwrap_or(usize::MAX)
    }

    #[test]
    fn search_ignores_case_and_blank_queries() {
        assert!(font_matches("Microsoft YaHei UI", "yahei"));
        assert!(font_matches("Georgia", "GEO"));
        assert!(!font_matches("Georgia", "arial"));
        assert!(font_matches("Georgia", "   "));
    }

    #[test]
    fn search_understands_the_chinese_names_people_type() {
        assert!(font_matches("Microsoft YaHei UI", "雅黑"));
        assert!(font_matches("SimSun", "宋体"));
        assert!(!font_matches("Georgia", "雅黑"));
    }

    #[test]
    fn every_installed_family_is_offered() {
        let installed = installed();
        let latin = font_choices(&installed, FontSlot::Latin, None);
        assert_eq!(latin.len(), installed.len(), "nothing may be hidden");
        assert_eq!(latin.first().map(String::as_str), Some("Segoe UI"));
        assert!(latin.iter().any(|name| name == "Unrelated Face"));
    }

    #[test]
    fn han_capable_families_are_promoted_above_single_weight_cuts() {
        let latin = font_choices(&installed(), FontSlot::Latin, None);
        assert!(rank(&latin, "Microsoft YaHei UI") < rank(&latin, "Unrelated Face"));
        assert!(rank(&latin, "MiSans") < rank(&latin, "MiSans Semibold"));
    }

    #[test]
    fn a_family_that_vanished_stays_selectable() {
        let choices = font_choices(&installed(), FontSlot::Latin, Some("Removed Face"));
        assert_eq!(choices.first().map(String::as_str), Some("Removed Face"));
    }
}

