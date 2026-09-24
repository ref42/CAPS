//! Native window geometry: where the shapes are painted, where the pointer is,
//! and how the frameless popup is dragged.
//!
//! The popup is a rectangle that is much larger than the visible capsule: the
//! rest is transparent padding that the rounded edge and its animation need.
//! Click-through is decided by `WM_NCHITTEST` (see [`mouse`]) rather than a
//! window region, because a region is a 1-bit mask and would cut the painted
//! antialiased edge into visible stair steps at every capsule size.
use gpui_kit::*;
use std::sync::atomic::{AtomicU32, Ordering};

#[cfg(target_os = "windows")]
mod mouse;

#[cfg(target_os = "windows")]
mod backdrop;

#[cfg(target_os = "windows")]
pub use mouse::install as install_mouse_handling;

#[cfg(not(target_os = "windows"))]
pub fn install_mouse_handling(_: &Window) {}

/// The desktop behind the capsule, sampled for choosing the palette its text can
/// be read against. `None` off Windows, or when the window cannot be read.
#[cfg(target_os = "windows")]
pub use backdrop::sample as sample_backdrop;

#[cfg(not(target_os = "windows"))]
pub fn sample_backdrop(_: &Window) -> Option<Vec<([u8; 3], f32)>> {
    None
}

/// The resting capsule: the five groups it shows, plus the minimum air between
/// them. The row spreads any extra width it is given evenly, so this is the one
/// width at which the gaps are exactly `READ_GAP`.
pub const COLLAPSED_W: f32 = 400.;
pub const COLLAPSED_H: f32 = 56.;
pub const EXPANDED_W: f32 = 460.;
pub const EXPANDED_H: f32 = 490.;
/// Transparent margin around the painted shapes.
pub const BLEED: f32 = 18.;
/// Vertical gap that opens between the capsule and the panel.
pub const PANEL_GAP: f32 = 8.;
/// How much taller the expanded capsule is than the collapsed one.
pub const EXPAND_STEP: f32 = 30.;
/// Corner radius of the panel, before the capsule scale.
pub const PANEL_RADIUS: f32 = 22.;
/// How far outside the painted edge the native region reaches. Windows masks a
/// region with one bit per pixel, so the mask has to sit clear of the
/// antialiased outline rather than through it. Every side is offset by the same
/// amount, which keeps the clipped shape symmetric around the paint.
pub const REGION_OUTSET: f32 = 2.;
/// Extra region room given while the capsule is animating. The paint that
/// follows a tick is one frame further along than the geometry that tick
/// published, so the mask is kept ahead of it instead of cutting the opening
/// panel short.
pub const REGION_LEAD: f32 = 24.;

/// Height of the capsule at a given expansion progress.
pub fn header_height(expansion: f32) -> f32 {
    COLLAPSED_H + EXPAND_STEP * expansion
}

/// Height of the panel at a given expansion progress.
pub fn panel_height(expansion: f32) -> f32 {
    (EXPANDED_H - COLLAPSED_H - EXPAND_STEP - PANEL_GAP) * expansion
}

/// Vertical offset of the panel's top edge at a given expansion progress.
pub fn panel_top(expansion: f32) -> f32 {
    BLEED + header_height(expansion) + PANEL_GAP * expansion
}

/// Total window height needed to paint the shapes, in logical points.
pub fn window_height(expansion: f32) -> f32 {
    header_height(expansion) + panel_height(expansion) + PANEL_GAP * expansion + 2. * BLEED
}

/// Total window width needed to paint the shapes, in logical points.
pub fn window_width(width: f32) -> f32 {
    width + 2. * BLEED
}

/// The painted shapes' device-pixel geometry, relative to the window's client
/// area. The tick publishes it every frame; the native hit test reads it so
/// that click-through matches what was actually painted.
#[derive(Clone, Copy, Default)]
pub struct Geometry {
    /// Capsule scale multiplied by the display scale factor.
    pub unit: f32,
    pub bleed: f32,
    pub width: f32,
    pub header: f32,
    pub panel: f32,
    pub panel_top: f32,
    pub panel_radius: f32,
}

/// `Geometry` is read on the window procedure's thread only, but the tick and
/// the message handler are not the same call stack, so it lives in atomics
/// rather than a lock. Every field is an independent `f32` bit pattern.
static GEOMETRY: [AtomicU32; 7] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];

pub fn publish_geometry(geometry: Geometry) {
    for (slot, value) in GEOMETRY.iter().zip([
        geometry.unit,
        geometry.bleed,
        geometry.width,
        geometry.header,
        geometry.panel,
        geometry.panel_top,
        geometry.panel_radius,
    ]) {
        slot.store(value.to_bits(), Ordering::Relaxed);
    }
}

pub fn geometry() -> Geometry {
    let mut values = [0f32; 7];
    for (value, slot) in values.iter_mut().zip(GEOMETRY.iter()) {
        *value = f32::from_bits(slot.load(Ordering::Relaxed));
    }
    Geometry {
        unit: values[0],
        bleed: values[1],
        width: values[2],
        header: values[3],
        panel: values[4],
        panel_top: values[5],
        panel_radius: values[6],
    }
}

fn in_rounded_rect(
    x: f32,
    y: f32,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    radius: f32,
) -> bool {
    let local_x = x - left;
    let local_y = y - top;
    if local_x < 0. || local_y < 0. || local_x > width || local_y > height {
        return false;
    }
    let radius = radius.min(width * 0.5).min(height * 0.5);
    if radius <= 0. {
        return true;
    }
    if (radius..=width - radius).contains(&local_x) || (radius..=height - radius).contains(&local_y)
    {
        return true;
    }
    let corner_x = if local_x < radius { radius } else { width - radius };
    let corner_y = if local_y < radius {
        radius
    } else {
        height - radius
    };
    let dx = local_x - corner_x;
    let dy = local_y - corner_y;
    dx * dx + dy * dy <= radius * radius
}

/// Whether the capsule itself, rather than the panel, is under a point. Used so
/// that the documented right-click-to-quit only fires where people expect it.
pub fn contains_header(geometry: Geometry, x: f32, y: f32) -> bool {
    geometry.width > 0.
        && geometry.header > 0.
        && in_rounded_rect(
            x,
            y,
            geometry.bleed,
            geometry.bleed,
            geometry.width,
            geometry.header,
            geometry.header * 0.5,
        )
}

/// Whether a client-relative device point is over a painted capsule. The gap
/// between the capsule and the panel counts as inside, so the pointer can
/// travel between them without the capsule collapsing underneath it.
pub fn contains_point(geometry: Geometry, x: f32, y: f32) -> bool {
    if geometry.width <= 0. || geometry.header <= 0. {
        return false;
    }
    let header = in_rounded_rect(
        x,
        y,
        geometry.bleed,
        geometry.bleed,
        geometry.width,
        geometry.header,
        geometry.header * 0.5,
    );
    if header {
        return true;
    }
    if geometry.panel > 0.5 {
        let panel = in_rounded_rect(
            x,
            y,
            geometry.bleed,
            geometry.panel_top,
            geometry.width,
            geometry.panel,
            geometry.panel_radius,
        );
        if panel {
            return true;
        }
        let bridge_top = geometry.bleed + geometry.header - geometry.unit;
        let bridge_bottom = geometry.panel_top + geometry.unit;
        if geometry.bleed <= x
            && x <= geometry.bleed + geometry.width
            && bridge_top <= y
            && y <= bridge_bottom
        {
            return true;
        }
    }
    false
}

/// Whether one of the visible shapes sits under the cursor, in the window's
/// own client coordinates. `None` when the platform cannot answer.
#[cfg(target_os = "windows")]
pub fn pointer_inside(window: &Window) -> Option<bool> {
    let client = mouse::cursor_in_client(window)?;
    Some(contains_point(geometry(), client.0, client.1))
}

#[cfg(not(target_os = "windows"))]
pub fn pointer_inside(_: &Window) -> Option<bool> {
    None
}

/// Clip the window to the painted shapes. See [`mouse::apply_region`].
/// `slack` widens the mask on every side without moving it, which is what the
/// animation lead needs.
pub fn apply_region(window: &Window, geometry: Geometry, slack: f32) {
    #[cfg(target_os = "windows")]
    mouse::apply_region(window, geometry, slack);
    #[cfg(not(target_os = "windows"))]
    let _ = (window, geometry, slack);
}

/// The cursor's screen position, in device pixels.
pub fn cursor_position() -> Option<(f32, f32)> {
    #[cfg(target_os = "windows")]
    {
        mouse::cursor_screen()
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// Where the window currently sits on the virtual desktop, in device pixels.
pub fn window_origin(window: &Window) -> Option<(f32, f32)> {
    #[cfg(target_os = "windows")]
    {
        mouse::window_rect(window).map(|(left, top, _, _)| (left, top))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
        None
    }
}

/// Move the frameless popup to a screen position, in device pixels.
///
/// The move is handed to the foreground executor rather than performed inline:
/// `SetWindowPos` dispatches `WM_MOVE` synchronously, and the window's move
/// callback re-enters the application, which is already mutably borrowed while
/// a view update is running. GPUI's own `Window::resize` defers the same way.
pub fn move_window(window: &Window, cx: &App, origin: (f32, f32)) {
    #[cfg(target_os = "windows")]
    {
        let Some(handle) = mouse::window_handle(window) else {
            return;
        };
        mouse::move_window(cx, handle, origin);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, cx, origin);
    }
}

/// The window's own size, in device pixels.
pub fn window_size(window: &Window) -> Option<(f32, f32)> {
    window_rect(window).map(|(left, top, right, bottom)| (right - left, bottom - top))
}

/// `(left, top, right, bottom)` of the window, in device pixels.
pub fn window_rect(window: &Window) -> Option<(f32, f32, f32, f32)> {
    #[cfg(target_os = "windows")]
    {
        mouse::window_rect(window)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
        None
    }
}

/// The nearest position that keeps a `size`-sized window inside `bounds`, all in
/// device pixels.
///
/// A window larger than the monitor is pinned to the monitor's top-left corner
/// rather than pushed off its far edge, so there is nothing to clamp against
/// when the two disagree.
pub fn clamp_to_bounds(
    position: (f32, f32),
    size: (f32, f32),
    bounds: (f32, f32, f32, f32),
) -> (f32, f32) {
    let (left, top, right, bottom) = bounds;
    let max_x = (right - size.0).max(left);
    let max_y = (bottom - size.1).max(top);
    (
        position.0.clamp(left, max_x),
        position.1.clamp(top, max_y),
    )
}

/// Keep a window of `size` on the monitor nearest `near`, in device pixels.
///
/// A drag follows the pointer, and the pointer can be taken to the very edge of
/// the desktop — which would otherwise be enough to park the capsule off the
/// screen, where it could not be grabbed again. Clamping against the monitor
/// under the *pointer* rather than the one under the window is what still allows
/// a drag across to a second display. Falls back to the position it was given
/// when the monitor cannot be read.
pub fn clamp_to_monitor(position: (f32, f32), size: (f32, f32), near: (f32, f32)) -> (f32, f32) {
    #[cfg(target_os = "windows")]
    {
        if let Some(bounds) = mouse::monitor_rect(near) {
            return clamp_to_bounds(position, size, bounds);
        }
    }
    let _ = (size, near);
    position
}

/// Whether the primary mouse button is physically held down.
pub fn left_button_down() -> bool {
    #[cfg(target_os = "windows")]
    {
        mouse::left_button_down()
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// The size and position the popup should open at.
pub fn options(cx: &App) -> WindowOptions {
    let settings = crate::storage::load_state().settings;
    let scale = settings.capsule_size as f32 / 100.;
    let size = size(
        px(window_width(COLLAPSED_W) * scale),
        px(window_height(0.) * scale),
    );
    let display = cx.primary_display().map(|d| d.bounds());
    let fallback = display
        .map(|d| {
            point(
                d.origin.x + (d.size.width - size.width) / 2.,
                d.origin.y + px(8.),
            )
        })
        .unwrap_or(point(px(200.), px(8.)));
    // A dragged capsule stays where the user left it, as long as enough of it is
    // still on a display to grab again. Anything else falls back to top centre.
    let origin = match settings.window_position {
        Some((x, y)) if display.is_some_and(|d| on_display(d, x, y, size)) => point(px(x), px(y)),
        _ => fallback,
    };
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, size))),
        titlebar: None,
        kind: WindowKind::PopUp,
        focus: false,
        is_resizable: false,
        is_minimizable: false,
        app_owns_titlebar_drag: true,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    }
}

/// Whether a remembered top-left keeps enough of the window on a display to be
/// grabbable again.
fn on_display(display: Bounds<Pixels>, x: f32, y: f32, size: Size<Pixels>) -> bool {
    let left = display.origin.x.as_f32();
    let top = display.origin.y.as_f32();
    let right = left + display.size.width.as_f32();
    let bottom = top + display.size.height.as_f32();
    let visible_x = (x + size.width.as_f32()).min(right) - x.max(left);
    let visible_y = (y + size.height.as_f32()).min(bottom) - y.max(top);
    visible_x >= 64. && visible_y >= 24.
}

#[cfg(test)]
mod tests {
    // `use gpui_kit::*` would shadow the built-in `#[test]` attribute with the
    // toolkit's own macro, so the geometry under test is imported by name.
    use super::{
        BLEED, COLLAPSED_H, EXPANDED_W, Geometry, PANEL_RADIUS, clamp_to_bounds, contains_header,
        contains_point, header_height, panel_height, panel_top, window_height, window_width,
    };

    fn geometry(expansion: f32) -> Geometry {
        let unit = 1.;
        Geometry {
            unit,
            bleed: BLEED * unit,
            width: EXPANDED_W * unit,
            header: header_height(expansion) * unit,
            panel: panel_height(expansion) * unit,
            panel_top: panel_top(expansion) * unit,
            panel_radius: PANEL_RADIUS * unit,
        }
    }

    #[test]
    fn transparent_margins_are_click_through() {
        let collapsed = geometry(0.);
        // The bleed margin around the capsule belongs to the desktop underneath.
        assert!(!contains_point(collapsed, 4., 4.));
        assert!(!contains_point(collapsed, BLEED + EXPANDED_W / 2., BLEED - 2.));
        assert!(contains_point(
            collapsed,
            BLEED + EXPANDED_W / 2.,
            BLEED + COLLAPSED_H / 2.
        ));
        // Collapsed, the area below the capsule is not part of the capsule.
        assert!(!contains_point(collapsed, BLEED + 40., BLEED + COLLAPSED_H + 20.));
    }

    #[test]
    fn open_capsule_keeps_the_gap_between_capsule_and_panel() {
        let open = geometry(1.);
        let middle = BLEED + EXPANDED_W / 2.;
        assert!(contains_point(open, middle, BLEED + header_height(1.) - 4.));
        assert!(contains_point(open, middle, panel_top(1.) + 4.));
        assert!(contains_point(open, middle, panel_top(1.) + panel_height(1.) - 4.));
        // Outside the rounded ends, but inside the bounding box.
        assert!(!contains_point(open, BLEED + 1., BLEED + 1.));
    }

    #[test]
    fn empty_geometry_hits_nothing() {
        assert!(!contains_point(Geometry::default(), 5., 5.));
        assert!(!contains_header(Geometry::default(), 5., 5.));
    }

    #[test]
    fn window_box_wraps_the_painted_shapes() {
        let tight = window_height(0.);
        assert_eq!(tight, COLLAPSED_H + 2. * BLEED);
        assert!(window_width(EXPANDED_W) > EXPANDED_W);
        assert!(window_height(1.) > window_height(0.));
    }

    #[test]
    fn a_dragged_capsule_stays_on_its_display() {
        let display = (0., 0., 1920., 1080.);
        let size = (760., 146.);
        // Already inside: not moved.
        assert_eq!(clamp_to_bounds((100., 40.), size, display), (100., 40.));
        // Past one edge, pulled back until the whole window is on the display.
        assert_eq!(clamp_to_bounds((-500., 40.), size, display), (0., 40.));
        assert_eq!(clamp_to_bounds((5000., 40.), size, display), (1160., 40.));
        assert_eq!(clamp_to_bounds((100., -500.), size, display), (100., 0.));
        assert_eq!(clamp_to_bounds((100., 5000.), size, display), (100., 934.));
    }

    #[test]
    fn a_second_display_clamps_to_its_own_bounds() {
        // A display to the right of the primary, as Windows reports it.
        let display = (1920., 0., 3200., 1080.);
        let size = (760., 146.);
        assert_eq!(clamp_to_bounds((4000., 100.), size, display), (2440., 100.));
        assert_eq!(clamp_to_bounds((1500., 100.), size, display), (1920., 100.));
    }

    #[test]
    fn a_capsule_larger_than_its_display_pins_to_the_corner() {
        let display = (0., 0., 640., 480.);
        let size = (760., 146.);
        assert_eq!(clamp_to_bounds((100., 100.), size, display), (0., 100.));
        assert_eq!(clamp_to_bounds((100., 900.), size, display), (0., 334.));
    }
}
