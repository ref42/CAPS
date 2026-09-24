//! Native GPUI layout and interaction; no DOM, CSS or embedded browser.
//!
//! Everything the capsule shows is drawn from the token block below. The palette
//! is deliberately small: one dark appearance, one accent, and the album's own
//! colours for the spectrum and the seek bar.
use crate::{
    app::{Caps, FontSlot, Source},
    audio::AudioCommand,
    formatting::format_rate,
    mode::{MusicMode, PlayOrder},
    motion,
    windowing::*,
};
use gpui_kit::base::{
    Scrollbar, ScrollbarMode, Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack,
};
use gpui_kit::component::{Sizable, Theme, ThemeMode, input::Input, slider::Slider};
use gpui_kit::{prelude::FluentBuilder, *};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------- palette ---
//
// Two appearances, chosen from what the capsule is floating over. The surface is
// the user's to fade — the opacity control reaches zero — so the text cannot
// take its contrast from that surface: it has to be the opposite of the pixels
// behind the window. The desktop is sampled around the capsule and composited
// with the faded surface, and the tones below are picked against *that* — white
// over a dark desktop, near-black over a light document. Hierarchy is carried by
// size and weight, because a half-tone is the first thing a variable background
// swallows.
const SURFACE: u32 = 0x0e0f12; // capsule body
const SURFACE_RAISED: u32 = 0x16181d; // panel: one step lighter, not glass on glass

/// Text and tints over a dark background.
const ON_DARK: Palette = Palette {
    text: 0xffffff,
    secondary: 0xffffff,
    tertiary: 0xffffff,
    accent: 0x7df2ca,
    hairline: 0xffffff12,
    well: 0xffffff0c,
    track: 0xffffff2e,
};

/// Text and tints over a light one.
const ON_LIGHT: Palette = Palette {
    text: 0x0d0f12,
    secondary: 0x0d0f12,
    tertiary: 0x0d0f12,
    accent: 0x0b7a58,
    hairline: 0x00000026,
    well: 0x00000014,
    track: 0x0000003d,
};

/// How much of the accent shows through a selection wash.
const WASH_ALPHA: u32 = 0x1c;

#[derive(Clone, Copy)]
struct Palette {
    text: u32,
    secondary: u32,
    tertiary: u32,
    accent: u32,
    hairline: u32,
    well: u32,
    track: u32,
}

/// Which of the two appearances is in force. Written by [`set_backdrop`] from
/// the app's tick, read by every colour accessor below.
static BACKDROP_IS_LIGHT: AtomicBool = AtomicBool::new(false);

/// How many samples in a row have asked for the other appearance. A backdrop
/// that is being dragged across the capsule changes its mind slowly.
static PENDING_SWITCH: AtomicU32 = AtomicU32::new(0);

fn palette() -> Palette {
    if BACKDROP_IS_LIGHT.load(Ordering::Relaxed) {
        ON_LIGHT
    } else {
        ON_DARK
    }
}

fn text() -> u32 {
    palette().text
}

fn text_secondary() -> u32 {
    palette().secondary
}

fn text_tertiary() -> u32 {
    palette().tertiary
}

fn accent() -> u32 {
    palette().accent
}

fn hairline() -> u32 {
    palette().hairline
}

fn well() -> u32 {
    palette().well
}

fn track_tint() -> u32 {
    palette().track
}

/// The accent as a translucent wash, so it follows whichever accent is in force.
fn selection() -> u32 {
    (accent() << 8) | WASH_ALPHA
}

/// Pick the appearance from what is behind the capsule; report whether it changed.
///
/// `samples` are the colours sampled around the capsule, each with the weight of
/// the area it stands for, and `alpha` is the user's opacity, because a surface
/// that is still mostly opaque keeps its own dark tone whatever is behind it —
/// every sample is composited the way the eye will see it before it is judged.
///
/// An capsule can easily sit across two windows, one white and one dark. The
/// larger area wins, but only by a clear margin, and only after two samples in a
/// row agree: a backdrop straddling the middle of the capsule must settle on one
/// appearance and keep it rather than strobing between the two.
///
/// Returns the appearance now in force when it changed, so the caller can bring
/// the component theme along with it.
pub fn set_backdrop(samples: &[([u8; 3], f32)], alpha: f32) -> Option<bool> {
    let surface = [
        ((SURFACE >> 16) & 0xff) as f32,
        ((SURFACE >> 8) & 0xff) as f32,
        (SURFACE & 0xff) as f32,
    ];
    let weights = [0.2126, 0.7152, 0.0722];
    let a = alpha.clamp(0., 1.);
    let (mut light, mut dark) = (0f32, 0f32);
    for (backdrop, weight) in samples {
        let mut luma = 0.;
        for i in 0..3 {
            let mixed = (a * surface[i] + (1. - a) * backdrop[i] as f32) / 255.;
            let linear = if mixed <= 0.03928 {
                mixed / 12.92
            } else {
                ((mixed + 0.055) / 1.055).powf(2.4)
            };
            luma += linear * weights[i];
        }
        // 0.19 is where white text and near-black text are equally readable, so
        // it is the only honest place to split the samples in two.
        if luma > 0.19 {
            light += weight;
        } else {
            dark += weight;
        }
    }
    let total = light + dark;
    if total <= 0. {
        return None;
    }
    let in_force = BACKDROP_IS_LIGHT.load(Ordering::Relaxed);
    let winner = if light / total > 0.6 {
        Some(true)
    } else if dark / total > 0.6 {
        Some(false)
    } else {
        None
    };
    let Some(wanted) = winner.filter(|wanted| *wanted != in_force) else {
        PENDING_SWITCH.store(0, Ordering::Relaxed);
        return None;
    };
    if PENDING_SWITCH.fetch_add(1, Ordering::Relaxed) + 1 < 2 {
        return None;
    }
    PENDING_SWITCH.store(0, Ordering::Relaxed);
    BACKDROP_IS_LIGHT.store(wanted, Ordering::Relaxed);
    Some(wanted)
}

/// Bring the component theme along with the palette.
///
/// The widgets this module does not paint itself — the search field's
/// placeholder, the scrollbar's thumb, a slider's fill — take their greys from
/// `gpui-component`'s theme, so the theme has to follow the same decision the
/// capsule's own colours do, or those few marks stay unreadable over exactly the
/// backdrops the rest of the capsule has adapted to.
pub fn sync_theme(light: bool, window: &mut Window, cx: &mut App) {
    // A light backdrop needs `ThemeMode::Light`: those widgets then draw their
    // dark greys, which is what the capsule's own text is doing.
    let wanted = if light {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    };
    if Theme::global(cx).is_dark() == light {
        Theme::change(wanted, Some(window), cx);
    }
}

// ------------------------------------------------------------------ type ---
//
// Logical points at 100% capsule size, scaled with the capsule. Nothing renders
// below `FLOOR`: typography.md › Ensuring legibility puts the desktop minimum
// at 10 pt, and this capsule is desktop-only.
const LYRIC: f32 = 17.;
const TITLE: f32 = 15.;
const BODY: f32 = 12.;
const LABEL: f32 = 11.;
const FLOOR: f32 = 10.;
/// The capsule has to hold the album art, a lyric line and the spectrum without
/// growing past the width the window was sized for.
const LYRIC_WIDTH: f32 = 268.;

const SEMIBOLD: FontWeight = FontWeight(600.);
const MEDIUM: FontWeight = FontWeight(500.);

/// A text size at the capsule's scale, held at the platform's minimum.
///
/// The capsule's size control scales everything, type included, and it goes down
/// to 85% — which took the 10 pt labels to 8.5 pt and the 11 pt ones to 9.4 pt,
/// both under the floor the HIG sets for desktop text. `accessibility.md ›
/// Vision`: "Use recommended defaults for custom type sizes. Each platform has
/// different default and minimum sizes for system-defined type styles to promote
/// readability. If you're using custom type styles, follow the recommended
/// defaults." `typography.md › Ensuring legibility`: "Use font sizes that most
/// people can read easily… Follow the recommended default and minimum text sizes
/// for each platform." At 100% and above this returns exactly `base * s`; it
/// only ever lifts the smallest sizes back to `FLOOR`.
fn type_size(base: f32, s: f32) -> f32 {
    (base * s).max(FLOOR)
}

/// Size of a drawn icon, before the capsule scale. Held level with `BODY`, so an
/// icon's ink weighs the same as the text beside it however it is drawn — then
/// half again as large, because the marks carry the transport row on their own
/// and read as timid at the text's own size.
const MARK_SIZE: f32 = BODY * 1.5;

/// The icons' buttons grow with them, so the padding around a mark keeps the
/// same proportion and the target gets easier to hit.
const ICON_BUTTON: f32 = 36.;
const ROW_BUTTON: f32 = 36.;

/// How long a scrolling line rests at each end, and how fast it travels, in
/// points per second.
const MARQUEE_HOLD: f32 = 1.2;
pub const MARQUEE_SPEED: f32 = 34.;

/// The lines that scroll while the pointer rests on them, one slot each in the
/// measurement tables below: a capsule shows its title and artist together, and
/// a track row shows its name and its metadata.
pub const TITLE_SLOT: usize = 0;
pub const ARTIST_SLOT: usize = 1;
pub const ROW_SLOT: usize = 2;
pub const NAME_SLOT: usize = 3;

/// Which part of the capsule the pointer is on, as far as those lines go.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HoverLine {
    /// The capsule's title and artist, which scroll together.
    Capsule,
    /// One track row's metadata, and which of the two lists it is in.
    Row(bool, usize),
}

/// Room left for a scrolling line, and the width that line wants, in points.
///
/// The line under the pointer publishes both as it paints, and the tick reads
/// them to keep frames coming while it moves. Lines nobody is pointing at never
/// write, so leaving them is what stops the scroll.
static LINE_ROOM: [AtomicU32; 4] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static LINE_WANT: [AtomicU32; 4] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];

fn line_room(slot: usize) -> f32 {
    f32::from_bits(LINE_ROOM[slot].load(Ordering::Relaxed))
}

fn line_want(slot: usize) -> f32 {
    f32::from_bits(LINE_WANT[slot].load(Ordering::Relaxed))
}

/// Whether the line in `slot` is wider than the box showing it.
pub fn line_overflows(slot: usize) -> bool {
    line_want(slot) > line_room(slot) + 1.
}

/// How far that line has to travel to reach its end.
pub fn line_travel(slot: usize) -> f32 {
    (line_want(slot) - line_room(slot)).max(0.)
}

/// How far a line has scrolled after `elapsed` of hovering.
///
/// The line rests at each end before moving on: one that snapped back the
/// moment it arrived would be harder to read than one that stops where it
/// finished.
pub fn marquee_phase(elapsed: Duration, travel: f32, speed: f32) -> f32 {
    if travel <= 0. || speed <= 0. {
        return 0.;
    }
    let slide = travel / speed;
    let cycle = MARQUEE_HOLD * 2. + slide;
    let t = elapsed.as_secs_f32() % cycle;
    if t < MARQUEE_HOLD {
        0.
    } else if t < MARQUEE_HOLD + slide {
        (t - MARQUEE_HOLD) * speed
    } else {
        travel
    }
}

/// How a scrolling line is set in type.
struct LineStyle {
    size: f32,
    color: u32,
    weight: FontWeight,
}

/// One line of text that scrolls while the pointer is on it, if it does not fit.
///
/// How wide the line wants to be cannot be asked of layout: the box clamps what
/// a child reports, so the width it wants came back equal to the room it had and
/// nothing ever scrolled. It is shaped through the text system instead, while
/// the pointer is on it, and the result is published for the tick.
///
/// One function covers all three lines — a row's metadata and the capsule's
/// title and artist — because they need exactly this and differ only in type.
fn scroll_line(
    slot: usize,
    text: String,
    style: LineStyle,
    offset: f32,
    hovered: bool,
) -> Div {
    let scrolling = hovered && line_overflows(slot);
    let measured = text.to_owned();
    let size = style.size;
    div()
        .w_full()
        .overflow_hidden()
        .child(
            canvas(
                move |bounds, window, _| {
                    if hovered {
                        LINE_ROOM[slot]
                            .store(f32::from(bounds.size.width).to_bits(), Ordering::Relaxed);
                        let text_style = window.text_style();
                        let run = TextRun {
                            len: measured.len(),
                            font: text_style.font(),
                            color: text_style.color,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let shaped = window.text_system().shape_line(
                            measured.to_owned().into(),
                            px(size),
                            &[run],
                            None,
                        );
                        LINE_WANT[slot]
                            .store(f32::from(shaped.width).to_bits(), Ordering::Relaxed);
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .child(
            div()
                .flex_shrink_0()
                .whitespace_nowrap()
                .relative()
                .left(px(if scrolling { -offset } else { 0. }))
                .text_size(px(size))
                .font_weight(style.weight)
                .text_color(rgb(style.color))
                // While it scrolls the line is cut by the box instead: an
                // ellipsis would ride along and hide the characters being
                // revealed.
                .when(!scrolling, |d| d.text_ellipsis())
                .child(text),
        )
}

/// Height of the rail the dot rides on, of the box that holds both, and the
/// diameter of the dot itself.
///
/// The rail is what the dot is centred on, so both numbers place it. The dot is
/// the 13 pt it used to be, at 0.8: the capsule is a stadium, so its lower rows
/// curve inward, and at full size the dot's outer edge met that curve at the
/// start of the bar and read as cut off at the bottom corner rather than round.
const SEEK_RAIL: f32 = 4.;
const SEEK_TRACK: f32 = 14.;
const SEEK_DOT: f32 = 10.4;

/// Width of an idle reading's value cell, in points at 100% capsule size.
///
/// A reading is an icon, a figure and its unit, and the cell holding the last
/// two keeps one width whatever the figures do: a cell that resized with them
/// pushed the icon sideways and changed the gap to the next reading, which made
/// the whole strip twitch as the numbers moved. The figure is set flush *left*
/// inside the cell, so it sits against its icon and only ever grows rightwards —
/// the slack a short figure leaves is trailing, where nobody sees it, instead of
/// opening a hole between the icon and the number it belongs to.
///
/// Four characters covers `100%`; nine covers the longest rate, `1023 KB/s` —
/// `format_rate` never emits a tenth, so a rate figure cannot reach the icon of
/// the next reading even in the worst case.
const READ_PERCENT_W: f32 = 30.;
const READ_RATE_W: f32 = 66.;
/// The icon inside a reading and the gap between it and the figure.
const READ_ICON: f32 = 15.;
const READ_TEXT_GAP: f32 = 3.;
/// The gap between readings, and the spectrum's own column.
///
/// The readings and the spectrum now carry their own widths, so these two
/// numbers, the header's 12 pt padding and `COLLAPSED_W` are all that is left
/// for the air between them: 400 − 24 padding − 2 × (15 + 3 + 30) percent − 2 ×
/// (15 + 3 + 66) rate − 48 spectrum leaves 64 pt, exactly four 16 pt gaps.
/// Hovering widens the capsule to 460 and the row spreads that extra 60 pt
/// evenly between the same five groups.
const READ_GAP: f32 = 16.;
const READ_SPECTRUM_W: f32 = 48.;

/// Tabular figures, for a number that changes while you look at it.
///
/// No HIG page covers this — it is studio practice, and it is the difference
/// between digits that keep their places and digits that shuffle in a
/// proportional face. Fonts without the feature simply ignore it.
fn tabular_figures() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".to_string(), 1)]))
}

/// The seek bar's own band at the bottom of the capsule.
///
/// The capsule opens to 86 pt, and the artwork is a 52 pt circle centred in it,
/// which leaves the bottom 17 pt free: the bar sits in that band, one dot
/// diameter lower than it used to, and that is also what lifts the dot clear of
/// the artist line above it instead of crowding its descenders.
const SEEK_BOTTOM: f32 = 2.;
/// Height of the bar's own box. The rail and the thumb are drawn inside it.
const SEEK_HEIGHT: f32 = 12.;
/// How far the bar's ends sit inside the capsule.
///
/// The capsule is a stadium, so its lower rows curve inward: at the bar's lowest
/// row the painted edge is 30 pt in from the corner. This clears that with room
/// for the thumb, which is why the bar no longer has to stop where the artwork
/// ends and can read as one line across the capsule.
const SEEK_EDGE: f32 = 36.;

/// Width of the track rows' duration column. Wide enough for `1:00:00`, so the
/// numbers stay in one column whatever the queue holds.
const DURATION_W: f32 = 38.;

/// Track length as `m:ss`, or `h:mm:ss` for anything over an hour.
fn duration_label(seconds: Option<u64>) -> String {
    let Some(total) = seconds else {
        return String::new();
    };
    let (hours, minutes, rest) = (total / 3600, (total / 60) % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{rest:02}")
    } else {
        format!("{minutes}:{rest:02}")
    }
}

/// Han coverage for the Latin UI face.
///
/// The theme names one Latin family (Segoe UI on Windows), which has no Han
/// glyphs at all. Without a declared fallback DirectWrite picks one on its own
/// and nothing guarantees it matches: the substituted face may only carry a
/// regular weight, so a semibold run gets synthesised bold, and its metrics need
/// not line up with the Latin beside it — which is why mixed lines like
/// "泽国同学 · 八千里路明月夜 · 3:20" looked uneven. Naming the chain keeps the
/// pairing Windows itself uses, and `generate_font_fallbacks` skips any family
/// the machine does not have, so the list is safe to over-specify.
///
/// `typography.md › Conveying hierarchy` asks to "minimize the number of
/// typefaces"; this is two — the system Latin face and its Han companion — which
/// is the same pairing the platform's own interface uses for the two scripts.
/// Settings can override either half.
pub const HAN_FALLBACKS: [&str; 6] = [
    "Microsoft YaHei UI",
    "Microsoft YaHei",
    "Noto Sans SC",
    "MiSans",
    "SimHei",
    "DengXian",
];

/// The capsule's text style: the chosen Latin family, with Han resolved through
/// the user's Han family and then [`HAN_FALLBACKS`] rather than DirectWrite's
/// own choice.
fn capsule_font(family: SharedString, han: FontFallbacks) -> Font {
    Font {
        family,
        features: FontFeatures::default(),
        weight: FontWeight::default(),
        style: FontStyle::default(),
        fallbacks: Some(han),
    }
}

fn row() -> Div {
    div().flex().items_center()
}
fn column() -> Div {
    div().flex().flex_col()
}

/// Inset segmented control. The 12 pt outer radius minus its 3 pt padding is
/// what the 9 pt segments and 9 pt rows below are measured against.
fn segmented(s: f32) -> Div {
    row()
        .gap(px(4. * s))
        .p(px(3. * s))
        .h(px(32. * s))
        .rounded(px(12. * s))
        .bg(rgba(well()))
}

/// A text button in a segmented control or a row of actions.
fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    active: bool,
    s: f32,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(26. * s))
        .px(px(9. * s))
        .rounded(px(9. * s))
        .cursor_pointer()
        .text_size(px(BODY * s))
        .font_weight(MEDIUM)
        .text_color(if active { rgb(accent()) } else { rgb(text_secondary()) })
        .bg(if active {
            rgba(selection())
        } else {
            rgba(0xffffff00)
        })
        .hover(|style| {
            style
                .bg(if active {
                    rgba(selection())
                } else {
                    rgba(0xffffff12)
                })
                .text_color(rgb(text()))
        })
        .active(|style| style.bg(rgba(0xffffff20)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(label.into())
}

/// An icon-only control. Every one carries an accessible label, which is what
/// `buttons.md › Content` asks for in place of a visible one.
fn icon_button(
    id: impl Into<ElementId>,
    glyph: impl IntoElement,
    label: &'static str,
    s: f32,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(ICON_BUTTON * s))
        .rounded_full()
        .cursor_pointer()
        .text_size(px(BODY * s))
        .text_color(rgb(text_secondary()))
        .aria_label(label)
        .hover(|style| style.bg(rgba(0xffffff14)).text_color(rgb(text())))
        .active(|style| style.bg(rgba(0xffffff24)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(glyph)
}

/// The capsule's icon set, drawn rather than typed.
///
/// These used to be characters: `Ⅱ` for pause, `◀ ▶ ■` for the transport, `▶ ×`
/// for the row actions. A glyph's ink sits wherever its font's metrics leave it,
/// so each one landed a little off-centre in its button and changed shape with
/// the interface font — `Ⅱ` resolved through the Han fallback and came out as
/// two serifed hairlines, measured 1 px wide next to 9 px triangles on the same
/// row. `icons.md › Best practices`: "all interface icons — need to use a
/// consistent size, level of detail, stroke thickness (or weight), and
/// perspective." That only holds if the app draws its own marks: every one below
/// is built in a unit square and scaled to the control, so it is centred by
/// construction and identical in every font.
#[derive(Clone, Copy)]
enum Mark {
    /// Play the selected track; the transport's play button as well.
    Play,
    Pause,
    Stop,
    /// Skip to the previous track.
    Previous,
    /// Skip to the next track.
    Next,
    /// Take the track out of the queue.
    Remove,
    /// Put the track into the queue.
    Add,
    /// Processor usage, for the idle capsule's first reading.
    Cpu,
    /// Memory usage.
    Memory,
    /// Network download rate.
    Download,
    /// Network upload rate.
    Upload,
}

impl Mark {
    /// Solid marks for the transport row, outlines for the smaller actions, so
    /// the two families stay apart at 12 pt.
    fn solid(self) -> bool {
        matches!(
            self,
            Self::Play | Self::Pause | Self::Stop | Self::Previous | Self::Next
        )
    }

    /// Adds this mark's geometry to `path`, in a unit square with y down.
    fn draw(self, path: &mut PathBuilder) {
        match self {
            // A play triangle is not centred by its bounding box: with the tip
            // on the right the ink's mass sits left of it, so the box is nudged
            // right until the centroid lands on the middle. `icons.md › Best
            // practices` calls the same correction optical alignment.
            Self::Play | Self::Next => {
                path.add_polygon(
                    &[unit(0.34, 0.21), unit(0.80, 0.50), unit(0.34, 0.79)],
                    true,
                );
            }
            Self::Previous => {
                path.add_polygon(
                    &[unit(0.66, 0.21), unit(0.20, 0.50), unit(0.66, 0.79)],
                    true,
                );
            }
            // The bars fill the same optical box as the triangles, so switching
            // between play and pause does not move the row's weight.
            Self::Pause => {
                for (left, right) in [(0.34, 0.485), (0.655, 0.80)] {
                    path.add_polygon(
                        &[
                            unit(left, 0.21),
                            unit(right, 0.21),
                            unit(right, 0.79),
                            unit(left, 0.79),
                        ],
                        true,
                    );
                }
            }
            Self::Stop => {
                path.add_polygon(
                    &[
                        unit(0.30, 0.30),
                        unit(0.72, 0.30),
                        unit(0.72, 0.72),
                        unit(0.30, 0.72),
                    ],
                    true,
                );
            }
            Self::Remove => {
                path.move_to(unit(0.28, 0.28));
                path.line_to(unit(0.72, 0.72));
                path.move_to(unit(0.72, 0.28));
                path.line_to(unit(0.28, 0.72));
            }
            Self::Add => {
                path.move_to(unit(0.50, 0.26));
                path.line_to(unit(0.50, 0.74));
                path.move_to(unit(0.26, 0.50));
                path.line_to(unit(0.74, 0.50));
            }
            // A processor: a die with a solid core and two pins to each side.
            // Pins on all four sides of a stroked square read as a gear at this
            // size; `icons.md › Best practices` asks for a "recognizable, highly
            // simplified design", and four pins keep the chip silhouette without
            // the cogwheel.
            Self::Cpu => {
                path.add_polygon(
                    &[
                        unit(0.28, 0.28),
                        unit(0.72, 0.28),
                        unit(0.72, 0.72),
                        unit(0.28, 0.72),
                    ],
                    true,
                );
                path.add_polygon(
                    &[
                        unit(0.42, 0.42),
                        unit(0.58, 0.42),
                        unit(0.58, 0.58),
                        unit(0.42, 0.58),
                    ],
                    true,
                );
                for (from, to) in [
                    ((0.13, 0.40), (0.28, 0.40)),
                    ((0.13, 0.60), (0.28, 0.60)),
                    ((0.72, 0.40), (0.87, 0.40)),
                    ((0.72, 0.60), (0.87, 0.60)),
                ] {
                    path.move_to(unit(from.0, from.1));
                    path.line_to(unit(to.0, to.1));
                }
            }
            // A memory module: a long body with three contacts along its lower
            // edge. The silhouette is deliberately unlike the processor's square,
            // so the two are told apart at a glance.
            Self::Memory => {
                path.add_polygon(
                    &[
                        unit(0.12, 0.34),
                        unit(0.88, 0.34),
                        unit(0.88, 0.62),
                        unit(0.12, 0.62),
                    ],
                    true,
                );
                for x in [0.30, 0.50, 0.70] {
                    path.move_to(unit(x, 0.62));
                    path.line_to(unit(x, 0.77));
                }
            }
            // Transfer arrows, over a tray so the pair reads as movement between
            // the machine and the network rather than as scroll arrows.
            //
            // The heads are open and wide — a 0.22 of the box each way, against a
            // stroke of 0.13. A shorter head closes up: the two strokes of the V
            // merge with each other and with the shaft, and the arrow renders as
            // a lump on a stick instead of an arrowhead.
            //
            // Both are drawn above the middle: `icons.md › Best practices` notes
            // that a download icon "has more visual weight on the bottom than on
            // the top, which can make it look too low if it's geometrically
            // centered." They also fill more of their box than the chip and the
            // module do, which is the same page's advice to "adjust its
            // dimensions to ensure that it appears visually consistent with
            // other icons": an arrow is a narrow glyph, so at equal box size it
            // reads smaller than a filled square beside it.
            Self::Download => {
                path.move_to(unit(0.50, 0.07));
                path.line_to(unit(0.50, 0.62));
                path.move_to(unit(0.24, 0.38));
                path.line_to(unit(0.50, 0.64));
                path.line_to(unit(0.76, 0.38));
                path.move_to(unit(0.16, 0.86));
                path.line_to(unit(0.84, 0.86));
            }
            Self::Upload => {
                path.move_to(unit(0.50, 0.69));
                path.line_to(unit(0.50, 0.14));
                path.move_to(unit(0.24, 0.40));
                path.line_to(unit(0.50, 0.14));
                path.line_to(unit(0.76, 0.40));
                path.move_to(unit(0.16, 0.88));
                path.line_to(unit(0.84, 0.88));
            }
        }
    }
}

/// A point inside a mark's unit square.
fn unit(x: f32, y: f32) -> Point<Pixels> {
    point(px(x), px(y))
}

/// One icon at `size` points, in the colour the surrounding text inherits.
///
/// Taking the colour from the text style is what keeps the playing tint and the
/// hover state working on a mark that is not text.
fn mark(kind: Mark, size: f32) -> impl IntoElement {
    canvas(
        move |_, _, _| (),
        move |bounds, _, window, _| {
            let color = window.text_style().color;
            let mut path = if kind.solid() {
                PathBuilder::fill()
            } else {
                PathBuilder::stroke(px(size * 0.13))
            };
            kind.draw(&mut path);
            path.scale(size);
            path.translate(bounds.origin);
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        },
    )
    .size(px(size))
    // The painted path is laid out at `size` whatever the box does, so a mark
    // squeezed by a tight parent keeps painting full width and spills its ink
    // over whatever comes next. An icon is a fixed-size thing: never shrink it.
    .flex_shrink_0()
}

/// A small square control used inside track rows.
fn row_button(
    id: impl Into<ElementId>,
    glyph: impl IntoElement,
    label: &'static str,
    active: bool,
    s: f32,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(ROW_BUTTON * s))
        .rounded(px(10. * s))
        .cursor_pointer()
        .text_size(px(BODY * s))
        .text_color(if active { rgb(accent()) } else { rgb(text_secondary()) })
        .bg(if active {
            rgba(selection())
        } else {
            rgba(0xffffff00)
        })
        .aria_label(label)
        .hover(|style| style.bg(rgba(0xffffff16)).text_color(rgb(text())))
        .active(|style| style.bg(rgba(0xffffff26)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(glyph)
}

impl Render for Caps {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let e = self.expand_motion.value(now);
        let s = self.settings.capsule_size as f32 / 100.;
        window.set_rem_size(px(14. * s));
        let width = self.width_motion.value(now);
        let alpha = (self.settings.opacity as f32 / 100. * 255.) as u32;
        let header = row()
            .id("capsule")
            .relative()
            .w(px(width * s))
            .h(px(header_height(e) * s))
            .px(px((12. + 4. * e) * s))
            .gap(px(10. * s))
            .rounded_full()
            .bg(rgba((SURFACE << 8) | alpha))
            .border_1()
            .border_color(rgba(hairline()))
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if event.modifiers.shift {
                        this.begin_drag(window);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.end_drag(window);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            // Idle: the indicator is the fifth reading in the row, so it takes
            // the row's own spacing and scans as part of it. Playing: the music
            // header carries it, and the seek bar is laid over the capsule.
            .child(if self.has_music() {
                self.music_header(e, s, cx).into_any_element()
            } else {
                // The spectrum takes a fixed column and centres in it, so it sits
                // on the same pitch as the four readings rather than trailing
                // them at a different spacing.
                self.stats_header(s)
                    .child(
                        row()
                            .w(px(READ_SPECTRUM_W * s))
                            .flex_shrink_0()
                            .justify_center()
                            .child(self.spectrum_view(s)),
                    )
                    .into_any_element()
            })
            .when(self.has_music() && e > 0.6, |d| {
                d.child(self.seek_bar(e, s, width, cx))
            });
        let mut stage = column()
            .id("caps-stage")
            .relative()
            .p(px(BLEED * s))
            .gap(px(PANEL_GAP * e * s))
            .text_color(rgb(text()))
            .text_size(px(BODY * s))
            .font(capsule_font(
                self.latin_family(),
                self.han_fallbacks.to_owned(),
            ))
            .on_hover(cx.listener(|this, hovered, _, cx| {
                #[cfg(not(target_os = "windows"))]
                if this.set_hover(*hovered) {
                    cx.notify();
                }
                #[cfg(target_os = "windows")]
                let _ = (this, hovered, cx);
            }))
            .on_mouse_down(MouseButton::Right, |_, _, cx| {
                cx.stop_propagation();
            })
            .on_mouse_up(MouseButton::Right, |_, _, cx| {
                cx.stop_propagation();
                cx.quit();
            })
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    window.blur(cx);
                    this.input_focused = false;
                    this.expanded = false;
                    this.leave_at = Some(Instant::now());
                    cx.notify();
                }
            }))
            .child(header);
        if e > 0.001 {
            let tab = self.settings.active_tab.as_str();
            let tabs = segmented(s).children(
                [
                    ("search", self.tr("Search", "搜索")),
                    ("queue", self.tr("Queue", "队列")),
                    ("settings", self.tr("Settings", "设置")),
                ]
                .into_iter()
                .map(|(id, label)| {
                    button(id, label, tab == id, s)
                        .flex_1()
                        .h_full()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.settings.active_tab = id.into();
                            window.blur(cx);
                            this.input_focused = false;
                            this.list_scroll
                                .0
                                .borrow()
                                .base_handle
                                .set_offset(point(px(0.), px(0.)));
                            this.persist();
                            cx.notify();
                        }))
                }),
            );
            let content = match tab {
                "queue" => self.queue_panel(s, cx),
                "settings" => self.settings_panel(s, cx),
                _ => self.search_panel(s, cx),
            };
            let panel = column()
                .w(px(width * s))
                .h(px(panel_height(e) * s))
                .p(px(10. * s))
                .gap(px(10. * s))
                .rounded(px(PANEL_RADIUS * s))
                .bg(rgba((SURFACE_RAISED << 8) | alpha))
                .border_1()
                .border_color(rgba(hairline()))
                .overflow_hidden()
                .opacity(motion::ease(e))
                .child(tabs)
                .child(content);
            stage = stage.child(panel);
        }
        stage
    }
}

impl Caps {
    /// Idle capsule: the four figures people glance at, each with the same
    /// weight so no single reading shouts louder than the others, followed by
    /// the level indicator.
    ///
    /// Each group carries its own width and the row spreads what is left evenly
    /// between the groups, so the air either side of a group is the same whether
    /// it holds an `8%` that needs twenty points or an `196 KB/s` that needs
    /// sixty. Equal *columns* stop reading as even as soon as the figures differ
    /// in length: every short figure leaves a hole the size of the longest
    /// reading, which is what made the strip look bunched in the middle and
    /// empty at the edges.
    ///
    /// `layout.md › Visual hierarchy`: "Align elements to make them easier to
    /// scan. People assume that aligned items are related to each other." A tight
    /// pocket before the bars made them read as a stray mark rather than a fifth
    /// reading.
    fn stats_header(&self, s: f32) -> Div {
        // Each reading is an icon and its figure on one line. Both keep a fixed
        // width — the cell holds the longest rate `format_rate` can emit — so the
        // slots themselves never move as the figures change; only the air between
        // them is elastic.
        //
        // `icons.md › Best practices`: "Create a recognizable, highly simplified
        // design… icons work best when they use familiar visual metaphors that
        // are directly related to the actions they initiate or content they
        // represent", and "match the weights of interface icons and adjacent
        // text" — the marks are drawn at the same optical weight as the figures
        // beside them. The icons carry the meaning visually, so each reading also
        // carries the name and figure as its accessible label, which is what
        // "Provide alternative text labels for custom interface icons" asks for.
        row()
            .flex_1()
            .min_w_0()
            .justify_between()
            .gap(px(READ_GAP * s))
            .children(
            [
                (
                    Mark::Cpu,
                    self.tr("CPU", "处理器"),
                    format!("{:.0}%", self.stats.cpu),
                    READ_PERCENT_W,
                ),
                (
                    Mark::Memory,
                    self.tr("Memory", "内存"),
                    format!("{:.0}%", self.stats.memory),
                    READ_PERCENT_W,
                ),
                (
                    Mark::Download,
                    self.tr("Download", "下载"),
                    format_rate(self.stats.download),
                    READ_RATE_W,
                ),
                (
                    Mark::Upload,
                    self.tr("Upload", "上传"),
                    format_rate(self.stats.upload),
                    READ_RATE_W,
                ),
            ]
            .into_iter()
            .map(|(glyph, name, value, cell)| {
                row()
                    .id(name)
                    .flex_shrink_0()
                    .gap(px(READ_TEXT_GAP * s))
                    // The mark takes the colour it inherits from here; the figure
                    // sets its own below.
                    .text_color(rgb(text_tertiary()))
                    .aria_label(format!("{name} {value}"))
                    .child(mark(glyph, READ_ICON * s))
                    .child(
                        div()
                            .w(px(cell * s))
                            .flex_shrink_0()
                            // A figure wider than its cell (a gigabit download)
                            // overruns the cell rather than wrapping onto a
                            // second line, which would double the row's height.
                            .whitespace_nowrap()
                            .text_size(px(type_size(TITLE, s)))
                            .font_weight(SEMIBOLD)
                            .text_color(rgb(text()))
                            .font_features(tabular_figures())
                            .child(value),
                    )
            }),
        )
    }

    /// FFT bars tinted with the album's colours: the one piece of ornament the
    /// capsule has, and the only thing that moves while music plays.
    ///
    /// It is a compact level indicator, not a chart, and three rules follow
    /// from that:
    ///
    /// * It is centred on the row it shares with the figures. `layout.md ›
    ///   Visual hierarchy`: "Align elements to make them easier to scan." People
    ///   assume that aligned items are related to each other." Bars growing
    ///   from a baseline belong to a chart with an axis; here they sit on the
    ///   figures' centre line, so idle bars are centred and loud ones open out
    ///   from the middle like a waveform.
    /// * It is unobtrusive. `progress-indicators.md › Best practices`: "Prefer
    ///   an activity indicator (spinner) when space is constrained. Spinners are small
    ///   and unobtrusive." Its height stays close to the two text lines beside
    ///   it rather than shouting over them.
    /// * Its marks are weighted like the text. `icons.md › Best practices`:
    ///   "match the weights of interface icons and adjacent text". Hairline
    ///   bars read as decoration next to 12 pt semibold figures.
    fn spectrum_view(&self, s: f32) -> Stateful<Div> {
        const BAR_W: f32 = 5.;
        const GAP: f32 = 3.;
        const PLOT_H: f32 = 28.;
        const MAX_H: f32 = 26.;
        // A band at rest still reads as a bar rather than a speck.
        const MIN_H: f32 = 3.;
        let bands = self.spectrum.len() as f32;
        let width = (bands * BAR_W + (bands - 1.) * GAP) * s;
        row()
            .id("spectrum")
            .flex_shrink_0()
            .items_center()
            .gap(px(GAP * s))
            .w(px(width))
            .h(px(PLOT_H * s))
            .aria_label(self.tr("Audio spectrum", "音频频谱"))
            .children(self.spectrum.iter().enumerate().map(|(i, value)| {
                let t = i as f32 / (self.spectrum.len() - 1) as f32;
                let (a, b) = self.colors;
                let mix = |shift: u32| {
                    let x = (a >> shift) & 255u32;
                    let y = (b >> shift) & 255u32;
                    ((x as f32 + (y as f32 - x as f32) * t) as u32) << shift
                };
                // A centred bar of height h spans the middle outward, so the
                // mark's visual centre is the row's centre at every level.
                div()
                    .w(px(BAR_W * s))
                    .h(px((value.clamp(0.06, 1.) * MAX_H).max(MIN_H) * s))
                    .rounded(px(2. * s))
                    .bg(rgb(mix(16) | mix(8) | mix(0)))
            }))
    }

    fn music_header(&self, e: f32, s: f32, cx: &mut Context<Self>) -> Div {
        // `music_header` is only reached for a playing track. Falling back to
        // the idle capsule keeps a bad frame from taking the window down.
        let Some(track) = self.current_track.as_ref() else {
            return self.stats_header(s);
        };
        let diameter = (42. + 10. * e) * s;
        let mut cover = div()
            .size(px(diameter))
            .flex_shrink_0()
            .rounded_full()
            .overflow_hidden()
            .bg(rgb(0x14352f));
        // The record spins while it plays; with reduced motion it stays still,
        // which is the fade-instead-of-motion substitution from
        // accessibility.md › Cognitive.
        if let Some(image) = if self.reduce_motion {
            None
        } else {
            self.spinning_cover.as_ref()
        } {
            let frame = (self.cover_turn * crate::artwork::SPIN_FRAMES as f32) as usize
                % crate::artwork::SPIN_FRAMES;
            let image = image.to_owned();
            cover = cover.child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        let _ = window.paint_image(
                            bounds,
                            bounds,
                            Corners::all(px(diameter / 2.)),
                            image,
                            frame,
                            false,
                        );
                    },
                )
                .size_full(),
            );
        } else if let Some(image) = self.cover.to_owned() {
            cover = cover.child(
                img(image)
                    .size_full()
                    .rounded_full()
                    .object_fit(ObjectFit::Cover),
            );
        }
        let mut copy = column()
            .id("capsule-copy")
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .justify_center()
            .gap(px(4. * s))
            // Resting on the text is what starts it scrolling, so the target is
            // the whole column rather than the glyphs themselves.
            .on_hover(cx.listener(|this, entered, _, cx| {
                if this.set_line_hover(HoverLine::Capsule, *entered) {
                    cx.notify();
                }
            }));
        let title_offset = self.marquee_offset(HoverLine::Capsule, TITLE_SLOT, s);
        let artist_offset = self.marquee_offset(HoverLine::Capsule, ARTIST_SLOT, s);
        let capsule_hovered = self.hover_line == Some(HoverLine::Capsule);
        if e > 0.5 {
            copy = copy.child(scroll_line(
                TITLE_SLOT,
                track.name.to_owned(),
                LineStyle {
                    size: TITLE * s,
                    color: text(),
                    weight: SEMIBOLD,
                },
                title_offset,
                capsule_hovered,
            ));
        } else {
            copy = copy.child(self.lyric_view(s));
        }
        if e > 0.7 {
            copy = copy.child(scroll_line(
                ARTIST_SLOT,
                track.artist.to_owned(),
                LineStyle {
                    size: type_size(LABEL, s),
                    color: text_secondary(),
                    weight: FontWeight::default(),
                },
                artist_offset,
                capsule_hovered,
            ));
        }
        let mut content = row()
            .relative()
            .h_full()
            .flex_1()
            .min_w_0()
            .gap(px(10. * s))
            .child(cover)
            .child(copy);
        if e > 0.6 {
            let revealed = ((e - 0.6) / 0.4).clamp(0., 1.);
            let controls = row()
                .child(
                    icon_button(
                        "prev",
                        mark(Mark::Previous, MARK_SIZE * s),
                        self.tr("Previous track", "上一首"),
                        s,
                    )
                    .on_click(cx.listener(|this, _, _, _| this.step(-1))),
                )
                .child(
                    icon_button(
                        "pause",
                        mark(
                            if self.audio.is_playing {
                                Mark::Pause
                            } else {
                                Mark::Play
                            },
                            MARK_SIZE * s,
                        ),
                        if self.audio.is_playing {
                            self.tr("Pause", "暂停")
                        } else {
                            self.tr("Play", "播放")
                        },
                        s,
                    )
                    .text_color(rgb(if self.audio.is_playing {
                        accent()
                    } else {
                        text()
                    }))
                    .on_click(
                        cx.listener(|this, _, _, _| this.player.send(AudioCommand::PlayPause)),
                    ),
                )
                .child(
                    icon_button(
                        "next",
                        mark(Mark::Next, MARK_SIZE * s),
                        self.tr("Next track", "下一首"),
                        s,
                    )
                    .on_click(cx.listener(|this, _, _, _| this.step(1))),
                )
                .child(
                    icon_button(
                        "stop",
                        mark(Mark::Stop, MARK_SIZE * s),
                        self.tr("Stop", "停止"),
                        s,
                    )
                    .on_click(cx.listener(|this, _, _, _| this.stop())),
                );
            // Grouped with the spectrum at a tighter gap than the row's own:
            // that is what moves the transport right and hands the width back to
            // the title and artist, which are the lines that get clipped.
            content = content.child(
                row()
                    .gap(px(4. * s))
                    .child(controls.opacity(revealed))
                    .child(self.spectrum_view(s)),
            );
        } else {
            // The spectrum is the row's last flow child, so it sits at the
            // trailing edge.
            content = content.child(self.spectrum_view(s));
        }
        content
    }

    /// The seek bar, laid over the capsule's bottom band rather than over the
    /// text row.
    ///
    /// Both ends are inset by the same amount, so the bar is centred in the
    /// capsule and the dot travels the full width of it. The band is below the
    /// album art, which is what lets the bar start inside the artwork's own
    /// column: it is a quarter of the capsule's height lower than the text, so
    /// the two never touch.
    fn seek_bar(&self, e: f32, s: f32, width: f32, cx: &mut Context<Self>) -> Div {
        let seek_value = self.seek_slider.read(cx).value().end() / 100.;
        let (from, _) = self.colors;
        // The bar is placed with an explicit origin and length rather than with
        // insets: the slider inside forces a width from its content, so a `right`
        // inset is ignored, and an inset measured from the wrong box drifts.
        // `left` resolves against the capsule's border box, and the length is
        // derived from the capsule's own width, so the two ends stay equal.
        let track = ((width - 2. * SEEK_EDGE) * s).max(0.);
        div()
            .absolute()
            .left(px(SEEK_EDGE * s))
            .bottom(px(SEEK_BOTTOM * s))
            .w(px(track))
            .h(px(SEEK_HEIGHT * s))
            .opacity(((e - 0.6) / 0.4).clamp(0., 1.))
            .child(
                BaseSlider::new(&self.seek_slider)
                    .horizontal()
                    .w_full()
                    .h_full()
                    .child(
                        SliderTrack::new(&self.seek_slider)
                            .axis(Axis::Horizontal)
                            .flex()
                            .items_center()
                            .w_full()
                            .h(px(SEEK_TRACK * s))
                            .child(
                                SliderIndicator::new(&self.seek_slider)
                                    .relative()
                                    .w_full()
                                    .h(px(SEEK_RAIL * s))
                                    .rounded_full()
                                    .bg(rgba(track_tint()))
                                    .child(
                                        div()
                                            .absolute()
                                            .left_0()
                                            .top_0()
                                            .bottom_0()
                                            .w(relative(seek_value.clamp(0., 1.)))
                                            .rounded_full()
                                            .bg(rgb(from)),
                                    )
                                    .child(
                                        // The dot is centred on the rail rather
                                        // than on the box, so its offsets are
                                        // derived from both sizes.
                                        SliderThumb::new(&self.seek_slider)
                                            .axis(Axis::Horizontal)
                                            .absolute()
                                            .left(relative(seek_value.clamp(0., 1.)))
                                            .top(px((SEEK_RAIL - SEEK_DOT) * 0.5 * s))
                                            .ml(px(-SEEK_DOT * 0.5 * s))
                                            .size(px(SEEK_DOT * s))
                                            .rounded_full()
                                            .bg(rgb(0xe5e5ea)),
                                    ),
                            ),
                    ),
            )
    }

    /// One lyric line. Characters settle in with a small vertical offset; under
    /// reduced motion the line only fades.
    fn lyric_view(&self, s: f32) -> Div {
        let elapsed = self.lyric_changed.elapsed().as_secs_f32();
        let mut viewport = div()
            .relative()
            .w_full()
            .h(px(24. * s))
            .items_center()
            .justify_center()
            .text_size(px(LYRIC * s))
            .font_weight(SEMIBOLD)
            .overflow_hidden();
        for (text, outgoing) in [(&self.outgoing_lyric, true), (&self.lyric, false)] {
            if outgoing && elapsed > 0.3 {
                continue;
            }
            let estimated = text
                .chars()
                .map(|ch| if ch.is_ascii() { 9. } else { 18. })
                .sum::<f32>();
            // Keep long provider lyrics readable inside the fixed-width capsule:
            // typical lines keep the full face, unusually long ones scale down
            // before they can run into the rounded clipping edge.
            let available = LYRIC_WIDTH * s;
            let lyric_size = (LYRIC * s * (available / (estimated * s).max(available))).max(type_size(FLOOR, s));
            let rendered_width = estimated * (lyric_size / (LYRIC * s));
            let overflow = (rendered_width - available).max(0.);
            let travel = overflow + 28. * s;
            let phase = if overflow > 0. {
                (elapsed * 18. * s) % (travel * 2.)
            } else {
                0.
            };
            let offset = if phase <= travel {
                phase.min(overflow)
            } else {
                (travel * 2. - phase).min(overflow)
            };
            let mut letters = row()
                .absolute()
                .right_0()
                .top_0()
                .h(px(22. * s))
                .text_size(px(lyric_size))
                .children(text.chars().enumerate().map(|(i, ch)| {
                    let delay = (i.min(24) as f32) * 0.006;
                    let t = ((elapsed - delay) / if outgoing { 0.22 } else { 0.36 }).clamp(0., 1.);
                    let p = motion::ease(t);
                    let rise = if self.reduce_motion { 0. } else { 8. };
                    div()
                        .relative()
                        .top(px(if outgoing {
                            -rise * p * s
                        } else {
                            rise * (1. - p) * s
                        }))
                        .opacity(if outgoing { 1. - p } else { p })
                        .flex_shrink_0()
                        .child(ch.to_string())
                }));
            if overflow > 0. {
                letters = letters.left(px(-offset)).justify_start();
            } else {
                letters = letters.left_0().justify_center();
            }
            viewport = viewport.child(letters);
        }
        viewport
    }

    fn search_panel(&self, s: f32, cx: &mut Context<Self>) -> AnyElement {
        let mut panel = column().flex_1().min_h_0().gap(px(10. * s));
        panel = panel.child(
            segmented(s).children(
                [
                    ("online", Source::Online, self.tr("Online", "在线")),
                    ("bilibili", Source::Bilibili, "Bilibili"),
                    ("youtube", Source::Youtube, "YouTube"),
                    ("local", Source::Local, self.tr("Local", "本地")),
                ]
                .into_iter()
                .map(|(id, source, label)| {
                    button(id, label, self.source == source, s)
                        .flex_1()
                        .h_full()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.source = source;
                            window.blur(cx);
                            this.input_focused = false;
                            cx.notify();
                        }))
                }),
            ),
        );
        match self.source {
            Source::Online => {
                panel = panel.child(
                    row()
                        .gap(px(8. * s))
                        .child(div().flex_1().child(Input::new(&self.query).small()))
                        .child(
                            button("search-go", self.tr("Search", "搜索"), true, s)
                                .h(px(28. * s))
                                .on_click(cx.listener(|this, _, _, cx| this.search(cx))),
                        ),
                );
                panel = panel.child(
                    row()
                        .gap(px(7. * s))
                        .child(div().w(px(55. * s)).child(Input::new(&self.count).small()))
                        .child(
                            button("random-add", self.tr("Random +", "随机添加"), false, s)
                                .h(px(28. * s))
                                .on_click(cx.listener(|this, _, _, cx| this.random(false, cx))),
                        )
                        .child(
                            button("random-replace", self.tr("Replace", "随机替换"), false, s)
                                .h(px(28. * s))
                                .on_click(cx.listener(|this, _, _, cx| this.random(true, cx))),
                        )
                        .child(div().flex_1())
                        .child(
                            button("clear-results", self.tr("Clear", "清空"), false, s)
                                .h(px(28. * s))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.results.clear();
                                    cx.notify();
                                })),
                        ),
                );
                panel = panel.child(self.track_list(false, s, cx));
            }
            Source::Bilibili | Source::Youtube => {
                panel = panel
                    .child(
                        row()
                            .gap(px(8. * s))
                            .child(div().flex_1().child(Input::new(&self.video).small()))
                            .child(
                                button("import", self.tr("Import", "导入"), true, s)
                                    .h(px(28. * s))
                                    .on_click(cx.listener(|this, _, _, cx| this.import(cx))),
                            ),
                    )
                    .child(div().text_color(rgb(text_secondary())).child(self.tr(
                        "Paste a video link to add its audio to your queue.",
                        "粘贴视频链接，将音频添加到队列。",
                    )));
            }
            Source::Local => {
                panel = panel
                    .child(
                        row()
                            .gap(px(8. * s))
                            .child(
                                button("folder", self.tr("Choose folder", "选择文件夹"), true, s)
                                    .h(px(28. * s))
                                    .on_click(cx.listener(|this, _, _, _| this.pick_folder())),
                            )
                            .child(
                                button("load-local", self.tr("Load all", "加载全部"), false, s)
                                    .h(px(28. * s))
                                    .on_click(cx.listener(|this, _, _, _| this.load_local())),
                            ),
                    )
                    .child(
                        div()
                            .text_color(rgb(text_secondary()))
                            .text_ellipsis()
                            .child(self.settings.local_music_folder.to_owned()),
                    );
            }
        }
        panel
            .child(
                div()
                    .text_size(px(type_size(LABEL, s)))
                    .text_color(rgb(text_tertiary()))
                    .max_h(px(32. * s))
                    .overflow_hidden()
                    .child(self.status.to_owned()),
            )
            .into_any_element()
    }

    fn queue_panel(&self, s: f32, cx: &mut Context<Self>) -> AnyElement {
        let order = self.play_order;
        let (order_en, order_zh) = order.label();
        // Tinted once the order is something other than the one CAPS starts in,
        // the way a filter button marks that it is doing something. The default
        // stays quiet next to "clear queue".
        let order_played = order != PlayOrder::RepeatAll;
        column()
            .flex_1()
            .min_h_0()
            .gap(px(8. * s))
            .child(
                row()
                    .justify_between()
                    .text_color(rgb(text_secondary()))
                    .child(format!("{} {}", self.queue.len(), self.tr("tracks", "首歌曲")))
                    .child(
                        row()
                            .gap(px(6. * s))
                            // The order sits next to the button that empties the
                            // queue, because both act on the queue as a whole
                            // rather than on the track that happens to be on. It
                            // is named rather than drawn: a loop or crossing
                            // arrows are guesswork at this size, whereas "repeat
                            // one" is not.
                            .child(
                                button("play-order", self.tr(order_en, order_zh), order_played, s)
                                    .min_w(px(64. * s))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.cycle_play_order();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                button("clear-queue", self.tr("Clear queue", "清空队列"), false, s)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.stop();
                                        this.queue.clear();
                                        this.persist();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(self.track_list(true, s, cx))
            .into_any_element()
    }

    fn track_list(&self, queued: bool, s: f32, cx: &mut Context<Self>) -> AnyElement {
        let tracks = if queued { &self.queue } else { &self.results };
        // An empty list says what to do next instead of showing nothing.
        if tracks.is_empty() {
            let hint = if queued {
                self.tr(
                    "Nothing queued yet. Search for a song, or point the Local tab at a folder.",
                    "队列还是空的。搜索歌曲，或在“本地”里选择文件夹。",
                )
            } else {
                self.tr(
                    "Search results appear here. Pick one to add it to the queue.",
                    "搜索结果会显示在这里，选中即可加入队列。",
                )
            };
            return column()
                .flex_1()
                .min_h_0()
                .items_center()
                .justify_center()
                .px(px(28. * s))
                .child(
                    div()
                        .text_size(px(BODY * s))
                        .text_color(rgb(text_tertiary()))
                        .text_align(TextAlign::Center)
                        .child(hint),
                )
                .into_any_element();
        }
        let scroll = if queued {
            &self.queue_scroll
        } else {
            &self.list_scroll
        };
        let view = cx.entity().downgrade();
        let list = uniform_list(
            if queued { "queue-list" } else { "results-list" },
            tracks.len(),
            move |range, _, cx| {
                view.update(cx, |this, cx| {
                    range
                        .filter_map(|index| {
                            let track = if queued {
                                this.queue.get(index)
                            } else {
                                this.results.get(index)
                            }?
                            .to_owned();
                            Some(this.track_row(queued, index, &track, s, cx))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
            },
        )
        .flex_1()
        .min_h_0()
        // Room for the scrollbar, so a long album name ends at the bar rather
        // than under it.
        .pr(px(7. * s))
        .track_scroll(scroll);
        // A flex column, so the list's `flex_1` actually sizes it: a plain block
        // wrapper leaves the virtualized list with no height at all.
        column()
            .id(if queued {
                "queue-list-view"
            } else {
                "results-list-view"
            })
            .relative()
            .flex_1()
            .min_h_0()
            .child(list)
            .child(
                div()
                    .absolute()
                    .right_0()
                    .top_0()
                    .bottom_0()
                    // Always visible rather than only while scrolling: a queue
                    // that is quietly cut off at the fold hides the rest of the
                    // tracks from anyone who does not already know to scroll.
                    .child(
                        Scrollbar::vertical(scroll)
                            .mode(ScrollbarMode::Always)
                            .styles(|styles| {
                                styles
                                    .track(|style| style.bg(Hsla::from(rgba(well()))))
                                    .thumb(|style| style.bg(Hsla::from(rgba(track_tint()))))
                                    .thumb_hover(|style| style.bg(Hsla::from(rgba(0xffffff44))))
                                    .thumb_active(|style| style.bg(Hsla::from(rgba(0xffffff5c))))
                            }),
                    ),
            )
            .into_any_element()
    }

    fn track_row(
        &mut self,
        queued: bool,
        index: usize,
        track: &crate::track::Track,
        s: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let thumbnail = self.thumbnail(&track.cover);
        let name = if queued {
            track.name.to_owned()
        } else {
            format!("{} · {}", track.name, track.source)
        };
        // The length is not part of the metadata line any more: an album name
        // long enough to fill it used to push the length out of the ellipsis, so
        // some rows simply had no duration at all. In its own column it is
        // always there and lines up down the list, which is the comparison
        // `lists-and-tables.md › Best practices` expects of a table of tracks.
        let detail = format!("{} · {}", track.artist, track.album);
        let duration = duration_label(track.duration);
        let active = queued && self.current_index == Some(index) && self.current_track.is_some();
        // The line scrolls only for the row the pointer is on, and only when the
        // last paint found it wider than its box.
        let hovered = self.hover_line == Some(HoverLine::Row(queued, index));
        let name_offset = self.marquee_offset(HoverLine::Row(queued, index), NAME_SLOT, s);
        let offset = self.marquee_offset(HoverLine::Row(queued, index), ROW_SLOT, s);
        let row_element = row()
            .id((if queued { "queued" } else { "result" }, index))
            .w_full()
            .h(px(48. * s))
            .px(px(8. * s))
            .gap(px(9. * s))
            .rounded(px(9. * s))
            .cursor_pointer()
            .bg(if active {
                rgba(selection())
            } else {
                rgba(0xffffff00)
            })
            .hover(|style| style.bg(rgba(0xffffff0e)))
            .on_hover(cx.listener(move |this, entered, _, cx| {
                if this.set_line_hover(HoverLine::Row(queued, index), *entered) {
                    cx.notify();
                }
            }))
            // The whole row is the target, not just the icon at its end: a 48 pt
            // row is far easier to hit, and selecting a row to play it is what a
            // list of tracks is expected to do.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .size(px(32. * s))
                    .flex_shrink_0()
                    .rounded(px(8. * s))
                    .overflow_hidden()
                    .when_some(thumbnail, |d, image| {
                        d.child(img(image).size_full().object_fit(ObjectFit::Cover))
                    })
                    .bg(if active {
                        rgba(0x7df2ca24)
                    } else {
                        rgba(0xffffff12)
                    }),
            )
            .child(
                column()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2. * s))
                    .child(scroll_line(
                        NAME_SLOT,
                        name,
                        LineStyle {
                            size: BODY * s,
                            color: if active { accent() } else { text() },
                            weight: SEMIBOLD,
                        },
                        name_offset,
                        hovered,
                    ))
                    .child(scroll_line(
                        ROW_SLOT,
                        detail,
                        LineStyle {
                            size: type_size(FLOOR, s),
                            color: text_tertiary(),
                            weight: FontWeight::default(),
                        },
                        offset,
                        hovered,
                    )),
            )
            // Trailing figures in their own column, right-aligned so the lengths
            // can be read down the list instead of hunted for line by line.
            .child(
                div()
                    .w(px(DURATION_W * s))
                    .flex_shrink_0()
                    .text_size(px(type_size(FLOOR, s)))
                    .text_color(rgb(text_tertiary()))
                    .text_align(TextAlign::Right)
                    .child(duration),
            )
            // The action buttons sit tighter than the row's own gap, which hands
            // a little width back to the metadata beside them.
            .child(if queued {
                row()
                    .gap(px(2. * s))
                    .child(
                        row_button(
                            ("play-track", index),
                            mark(Mark::Play, MARK_SIZE * s),
                            self.tr("Play track", "播放歌曲"),
                            active,
                            s,
                        )
                        .on_click(cx.listener(move |this, _, _, _| this.play(index))),
                    )
                    .child(
                        row_button(
                            ("remove-track", index),
                            mark(Mark::Remove, MARK_SIZE * s),
                            self.tr("Remove from queue", "从队列移除"),
                            false,
                            s,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.remove(index);
                            cx.notify();
                        })),
                    )
                    .into_any_element()
            } else {
                row_button(
                    ("add-track", index),
                    mark(Mark::Add, MARK_SIZE * s),
                    self.tr("Add to queue", "添加到队列"),
                    false,
                    s,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.add_result(index);
                    cx.notify();
                }))
                .into_any_element()
            });
        // Selecting a row is the primary action; the icon at the end is only a
        // second way to reach it. Both handlers carry the row index and look the
        // track up when they fire, so painting a list never copies track data.
        let row_element = if queued {
            row_element.on_click(cx.listener(move |this, _, _, _| this.play(index)))
        } else {
            row_element.on_click(cx.listener(move |this, _, _, cx| {
                this.add_result(index);
                cx.notify();
            }))
        };
        row_element.into_any_element()
    }

    /// Queue the search result at `index`. Used by the row itself and by its
    /// `+` button.
    fn add_result(&mut self, index: usize) {
        // `queued` is false here, so the row came from the search results.
        let Some(track) = self.results.get(index).cloned() else {
            return;
        };
        let added = crate::actions::append_unique_tracks(&mut self.queue, [track.to_owned()]);
        self.status = if added == 0 {
            self.tr("Already queued.", "已在队列中。").into()
        } else {
            self.say(
                format!("Added {}", track.name),
                format!("已添加 {}", track.name),
            )
        };
        self.persist();
    }

    /// One font slot: what it applies to, the family in use, and a way in to the
    /// list of faces this machine actually has.
    fn font_choice_row(&self, slot: FontSlot, s: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        let (label, stored, default) = match slot {
            FontSlot::Latin => (
                self.tr("English", "英文"),
                self.settings.latin_font.as_deref(),
                self.system_font.to_string(),
            ),
            FontSlot::Han => (
                self.tr("Chinese", "中文"),
                self.settings.han_font.as_deref(),
                self.han_families().into_iter().next().unwrap_or_default(),
            ),
        };
        let missing = stored.is_some_and(|name| !self.font_installed(name, slot));
        let summary = match (stored, missing) {
            (Some(name), true) => self.say(
                format!("{name} (not installed)"),
                format!("{name}（未安装）"),
            ),
            (Some(name), false) => name.to_string(),
            (None, _) => self.say(format!("System · {default}"), format!("系统 · {default}")),
        };
        div()
            .id(("font-slot", slot as usize))
            .flex()
            .items_center()
            .gap(px(8. * s))
            .h(px(28. * s))
            .px(px(8. * s))
            .rounded(px(8. * s))
            .cursor_pointer()
            .bg(rgba(well()))
            .hover(|style| style.bg(rgba(0xffffff18)))
            .active(|style| style.bg(rgba(0xffffff24)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _, window, cx| {
                this.font_picker = Some(slot);
                // Every list opens unfiltered and at the top, rather than
                // wherever the last one was left.
                this.font_scroll
                    .0
                    .borrow()
                    .base_handle
                    .set_offset(point(px(0.), px(0.)));
                this.font_search
                    .update(cx, |input, cx| input.set_value("", window, cx));
                this.refilter_fonts(cx);
            }))
            .child(
                div()
                    .w(px(52. * s))
                    .flex_shrink_0()
                    .text_size(px(type_size(LABEL, s)))
                    .text_color(rgb(text_tertiary()))
                    .child(label),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(BODY * s))
                    .text_color(if missing { rgb(text_tertiary()) } else { rgb(text()) })
                    .text_ellipsis()
                    .child(summary),
            )
    }

    /// The list of installed faces for one slot. Each entry is drawn *in its own
    /// family*: choosing type by reading its name in another type tells you
    /// nothing. The selected face is marked, and the first entry restores the
    /// platform default.
    fn font_picker_panel(&self, slot: FontSlot, s: f32, cx: &mut Context<Self>) -> AnyElement {
        let title = match slot {
            FontSlot::Latin => self.tr("English font", "英文字体"),
            FontSlot::Han => self.tr("Chinese font", "中文字体"),
        };
        let total = self.font_choices(slot).len();
        let query = self.font_search.read(cx).value().to_string();
        // The default is pinned to the top only while the search box is empty;
        // once someone is typing a family name it is just noise.
        let pinned = usize::from(query.trim().is_empty());
        let count = pinned + self.font_matches.len();
        let matched = self.font_matches.len();
        let empty = self.say(
            format!("No font matches “{query}”"),
            format!("没有匹配“{query}”的字体。"),
        );
        let searching = !query.trim().is_empty();
        let sample = match slot {
            FontSlot::Latin => "Aa Bb Cc 123",
            FontSlot::Han => "汉字 歌声 明月",
        };
        // Every family the machine has is listed, so the list can be long: it is
        // virtualized, and the count above says how much of it is showing.
        let view = cx.entity().downgrade();
        let list = uniform_list("font-options", count, move |range, _, cx| {
            view.update(cx, |this, cx| {
                range
                    .filter_map(|row| {
                        let index = if row < pinned {
                            None
                        } else {
                            this.font_matches.get(row - pinned).copied()
                        };
                        this.font_option_row(slot, s, index, sample, cx)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
        })
        .flex_1()
        .min_h_0()
        .track_scroll(&self.font_scroll);
        let list: AnyElement = if count == 0 {
            column()
                .flex_1()
                .min_h_0()
                .items_center()
                .justify_center()
                .px(px(24. * s))
                .child(
                    div()
                        .text_size(px(BODY * s))
                        .text_color(rgb(text_tertiary()))
                        .text_align(TextAlign::Center)
                        .child(empty),
                )
                .into_any_element()
        } else {
            // A flex column, so the list's `flex_1` actually sizes it; a plain
            // block wrapper leaves the virtualized list with no height at all.
            column()
                .id("font-list-view")
                .relative()
                .flex_1()
                .min_h_0()
                .child(list.pr(px(7. * s)))
                .child(
                    div()
                        .absolute()
                        .right_0()
                        .top_0()
                        .bottom_0()
                        // Always visible: a list that is silently cut off at the
                        // fold hides half the fonts from anyone who is not
                        // already scrolling.
                        .child(
                            Scrollbar::vertical(&self.font_scroll)
                                .mode(ScrollbarMode::Always)
                                .styles(|styles| {
                                    styles
                                        .track(|style| style.bg(Hsla::from(rgba(well()))))
                                        .thumb(|style| style.bg(Hsla::from(rgba(track_tint()))))
                                        .thumb_hover(|style| {
                                            style.bg(Hsla::from(rgba(0xffffff44)))
                                        })
                                        .thumb_active(|style| {
                                            style.bg(Hsla::from(rgba(0xffffff5c)))
                                        })
                                }),
                        ),
                )
                .into_any_element()
        };
        column()
            .flex_1()
            .min_h_0()
            .gap(px(8. * s))
            .child(
                row()
                    .gap(px(8. * s))
                    .child(
                        button("font-back", self.tr("‹ Back", "‹ 返回"), false, s)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.font_picker = None;
                                window.blur(cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(BODY * s))
                            .text_color(rgb(text_secondary()))
                            .child(title),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(type_size(LABEL, s)))
                            .text_color(rgb(text_tertiary()))
                            .child(if searching {
                                self.say(
                                    format!("{matched} of {total}"),
                                    format!("{matched} / {total}"),
                                )
                            } else {
                                self.say(format!("{total} fonts"), format!("{total} 个字体"))
                            }),
                    ),
            )
            .child(div().child(Input::new(&self.font_search).small()))
            .child(list)
            .into_any_element()
    }

    /// One row of the font list. `index` is the entry in the open slot's list, or
    /// `None` for the pinned "System default" row. The label is looked up here so
    /// building a list of every installed family copies nothing but its text.
    fn font_option_row(
        &self,
        slot: FontSlot,
        s: f32,
        index: Option<usize>,
        sample: &'static str,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (default, stored) = match slot {
            FontSlot::Latin => (self.system_font.to_string(), self.settings.latin_font.as_deref()),
            FontSlot::Han => (
                self.han_families().into_iter().next().unwrap_or_default(),
                self.settings.han_font.as_deref(),
            ),
        };
        let candidate = index.and_then(|index| self.font_at(slot, index));
        let (label, selected) = match candidate {
            Some(family) => (family.to_string(), stored == Some(family)),
            None => (
                self.say(
                    format!("System default · {default}"),
                    format!("系统默认 · {default}"),
                ),
                stored.is_none(),
            ),
        };
        // The preview is drawn in the candidate family; the name beside it stays
        // in the interface font so the two can be told apart.
        let preview = capsule_font(
            match slot {
                FontSlot::Latin => candidate
                    .map(SharedString::from)
                    .unwrap_or_else(|| self.system_font.to_owned()),
                FontSlot::Han => self.latin_family(),
            },
            match slot {
                // Han previews ask for the candidate first, then the usual chain.
                FontSlot::Han => FontFallbacks::from_fonts(
                    candidate
                        .into_iter()
                        .chain(HAN_FALLBACKS.iter().copied())
                        .map(str::to_owned)
                        .collect(),
                ),
                FontSlot::Latin => self.han_fallbacks.to_owned(),
            },
        );
        Some(
            div()
                .id(SharedString::from(format!(
                    "font-option-{}-{}",
                    slot as usize,
                    index.map_or(usize::MAX, |index| index)
                )))
                .flex()
                .items_center()
                .gap(px(9. * s))
                .h(px(34. * s))
                .px(px(8. * s))
                .rounded(px(8. * s))
                .cursor_pointer()
                .bg(if selected {
                    rgba(selection())
                } else {
                    rgba(0xffffff00)
                })
                .hover(|style| style.bg(rgba(0xffffff12)))
                .active(|style| style.bg(rgba(0xffffff22)))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_font(slot, index, cx);
                }))
                .child(
                    div()
                        .w(px(14. * s))
                        .flex_shrink_0()
                        .text_color(rgb(accent()))
                        .child(if selected { "✓" } else { "" }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(LYRIC * s))
                        .font(preview)
                        .text_color(rgb(text()))
                        .text_ellipsis()
                        .child(sample),
                )
                .child(
                    div()
                        .w(px(170. * s))
                        .flex_shrink_0()
                        .text_size(px(type_size(FLOOR, s)))
                        .text_color(rgb(text_tertiary()))
                        .text_ellipsis()
                        .child(label),
                )
                .into_any_element(),
        )
    }

    /// A labelled row: name on the left, control on the right.
    fn setting_row(label: &'static str, s: f32, control: impl IntoElement) -> Div {
        row()
            .gap(px(12. * s))
            .child(
                div()
                    .w(px(84. * s))
                    .flex_shrink_0()
                    .text_color(rgb(text_secondary()))
                    .child(label),
            )
            .child(control)
    }

    fn settings_panel(&self, s: f32, cx: &mut Context<Self>) -> AnyElement {
        // One level of navigation: the picker replaces the settings list rather
        // than floating over it, because the capsule's window is only as large as
        // the capsule and has a region clipped to its shape — a popup big enough
        // for a font list would be cut off. Settings panes navigate between
        // views for the same reason (`settings.md › Platform considerations ›
        // Desktop (macOS)`).
        if let Some(slot) = self.font_picker {
            return self.font_picker_panel(slot, s, cx);
        }
        let mut panel = column()
            .id("settings-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .gap(px(16. * s));
        for (label, value, slider) in [
            (
                self.tr("Opacity", "透明度"),
                self.settings.opacity,
                &self.opacity_slider,
            ),
            (
                self.tr("Volume", "音量"),
                self.settings.volume,
                &self.volume_slider,
            ),
            (
                self.tr("Capsule size", "胶囊大小"),
                self.settings.capsule_size,
                &self.size_slider,
            ),
        ] {
            panel = panel.child(Self::setting_row(
                label,
                s,
                row()
                    .flex_1()
                    .gap(px(10. * s))
                    .child(div().flex_1().child(Slider::new(slider)))
                    .child(
                        div()
                            .w(px(38. * s))
                            .text_size(px(type_size(LABEL, s)))
                            .text_color(rgb(text_tertiary()))
                            .child(format!("{value}%")),
                    ),
            ));
        }
        panel = panel.child(Self::setting_row(
            self.tr("Mode", "模式"),
            s,
            row()
                .flex_1()
                .gap(px(6. * s))
                .children(
                    [
                        ("normal", MusicMode::Normal, self.tr("Normal", "普通")),
                        ("silent", MusicMode::Silent, self.tr("Silent", "静音")),
                        ("quiet", MusicMode::Quiet, self.tr("Quiet", "安静")),
                    ]
                    .into_iter()
                    .map(|(id, mode, label)| {
                        button(id, label, self.mode == mode, s)
                            .flex_1()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if mode == MusicMode::Silent {
                                    this.stop();
                                } else if this.current_track.is_some() {
                                    this.mode = mode;
                                } else {
                                    this.status = this
                                        .tr(
                                            "Pick a song from the queue first.",
                                            "请先从队列播放歌曲。",
                                        )
                                        .into();
                                }
                                cx.notify();
                            }))
                    }),
                ),
        ));
        panel = panel.child(Self::setting_row(
            self.tr("Language", "语言"),
            s,
            row()
                .flex_1()
                .gap(px(6. * s))
                .children(
                    [
                        ("en", self.tr("English", "英语")),
                        ("zh", self.tr("Chinese", "中文")),
                    ]
                    .into_iter()
                    .map(|(code, label)| {
                        button(code, label, self.settings.language == code, s)
                            .flex_1()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_language(code, window, cx);
                            }))
                    }),
                ),
        ));
        // Two font choices, one per script. This is an override of something
        // the app already detects — `settings.md › Best practices`: "Avoid using
        // settings to ask for setup information you can get in other ways" — so
        // each row opens on the platform default and says which face that is.
        panel = panel.child(Self::setting_row(
            self.tr("Fonts", "字体"),
            s,
            column()
                .flex_1()
                .min_w_0()
                .gap(px(6. * s))
                .child(self.font_choice_row(FontSlot::Latin, s, cx))
                .child(self.font_choice_row(FontSlot::Han, s, cx)),
        ));
        panel = panel.child(
            row()
                .gap(px(8. * s))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(type_size(LABEL, s)))
                        .text_color(rgb(text_tertiary()))
                        .text_ellipsis()
                        .child(self.update_status.to_owned()),
                )
                .child(
                    button("check-update", self.tr("Check update", "检查更新"), false, s)
                        .opacity(if self.update_busy { 0.4 } else { 1. })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.check_update();
                            cx.notify();
                        })),
                )
                .when(self.pending_update.is_some(), |d| {
                    d.child(
                        button("install-update", self.tr("Update", "更新"), true, s).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.install_update();
                                cx.notify();
                            }),
                        ),
                    )
                }),
        );
        if let Some(progress) = self.update_progress {
            panel = panel.child(
                div()
                    .h(px(3. * s))
                    .w_full()
                    .rounded_full()
                    .bg(rgba(track_tint()))
                    .child(
                        div()
                            .h_full()
                            .w(relative(progress.clamp(0., 1.)))
                            .bg(rgb(accent())),
                    ),
            );
        }
        // The two gestures that are not discoverable from the capsule itself.
        panel = panel.child(
            div()
                .text_size(px(type_size(LABEL, s)))
                .text_color(rgb(text_tertiary()))
                .child(self.tr(
                    "Hold Shift and drag the capsule to move it. Right-click the capsule to quit.",
                    "按住 Shift 拖动胶囊可移动位置。右键点击胶囊退出。",
                )),
        );
        panel.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    // The toolkit's glob would shadow the built-in `#[test]`, so the helper under
    // test is named explicitly.
    use super::{BODY, FLOOR, LABEL, LYRIC, TITLE, type_size};

    /// The capsule's size control scales type along with everything else and goes
    /// down to 85%, which used to take the 10 pt labels to 8.5 pt. This is the
    /// guard that nothing lands under the platform's minimum on the way.
    #[test]
    fn text_never_goes_below_the_platform_minimum() {
        for base in [FLOOR, LABEL, BODY, TITLE, LYRIC] {
            for size in 85..=150 {
                let scaled = type_size(base, size as f32 / 100.);
                assert!(scaled >= FLOOR, "{base} pt at {size}% came out {scaled} pt");
            }
        }
    }

    /// At full size and above the clamp must not touch anything, or the layout
    /// would drift away from the design.
    #[test]
    fn at_full_size_and_above_the_scale_is_untouched() {
        assert_eq!(type_size(BODY, 1.), BODY);
        assert_eq!(type_size(FLOOR, 1.5), FLOOR * 1.5);
        assert_eq!(type_size(LABEL, 1.2), LABEL * 1.2);
    }
}

