//! Native window procedure work that GPUI does not expose: click-through hit
//! testing, cursor geometry, deferred window moves and right-click ownership.
use gpui_kit::{App, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

type Hwnd = *mut c_void;
type Hrgn = *mut c_void;
type Hmonitor = *mut c_void;
type SubclassProc = unsafe extern "system" fn(Hwnd, u32, usize, isize, usize, usize) -> isize;
const SUBCLASS_ID: usize = 0x43415053;
const WM_CLOSE: u32 = 0x0010;
const WM_CANCELMODE: u32 = 0x001F;
const WM_NCHITTEST: u32 = 0x0084;
const WM_NCDESTROY: u32 = 0x0082;
const WM_CONTEXTMENU: u32 = 0x007B;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_RBUTTONDBLCLK: u32 = 0x0206;
const WM_CAPTURECHANGED: u32 = 0x0215;
const WM_NCRBUTTONDOWN: u32 = 0x00A4;
const WM_NCRBUTTONUP: u32 = 0x00A5;
/// Let the mouse fall through to whatever window is underneath.
const HTTRANSPARENT: isize = -1;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
static RIGHT_PRESSED: AtomicBool = AtomicBool::new(false);

#[repr(C)]
#[derive(Default, Copy, Clone)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
struct MonitorInfo {
    size: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
}

#[link(name = "comctl32")]
unsafe extern "system" {
    fn SetWindowSubclass(hwnd: Hwnd, proc: SubclassProc, id: usize, data: usize) -> i32;
    fn RemoveWindowSubclass(hwnd: Hwnd, proc: SubclassProc, id: usize) -> i32;
    fn DefSubclassProc(hwnd: Hwnd, message: u32, wparam: usize, lparam: isize) -> isize;
}
#[link(name = "user32")]
unsafe extern "system" {
    fn SetCapture(hwnd: Hwnd) -> Hwnd;
    fn GetCapture() -> Hwnd;
    fn ReleaseCapture() -> i32;
    fn PostMessageW(hwnd: Hwnd, message: u32, wparam: usize, lparam: isize) -> i32;
    fn GetCursorPos(point: *mut Point) -> i32;
    fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
    fn ScreenToClient(hwnd: Hwnd, point: *mut Point) -> i32;
    fn ClientToScreen(hwnd: Hwnd, point: *mut Point) -> i32;
    fn SetWindowRgn(hwnd: Hwnd, region: Hrgn, redraw: i32) -> i32;
    fn SetWindowPos(
        hwnd: Hwnd,
        after: Hwnd,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
    fn GetAsyncKeyState(key: i32) -> i16;
    fn MonitorFromPoint(point: Point, flags: u32) -> Hmonitor;
    fn GetMonitorInfoW(monitor: Hmonitor, info: *mut MonitorInfo) -> i32;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateRectRgn(left: i32, top: i32, right: i32, bottom: i32) -> Hrgn;
    fn CreateRoundRectRgn(
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        width: i32,
        height: i32,
    ) -> Hrgn;
    fn CombineRgn(dst: Hrgn, a: Hrgn, b: Hrgn, mode: i32) -> i32;
    fn DeleteObject(object: Hrgn) -> i32;
}

/// `MONITOR_DEFAULTTONEAREST`: answer with the closest monitor rather than
/// failing when the point is not on one.
const MONITOR_DEFAULTTONEAREST: u32 = 2;

/// `RGN_OR`, the combine mode that unions two regions.
const RGN_OR: i32 = 2;

const VK_LBUTTON: i32 = 0x01;

/// Whether the physical left button is down. The capsule keeps tracking a drag
/// from the button itself, because a release outside the visible capsule never
/// reaches the window as a mouse-up.
pub fn left_button_down() -> bool {
    unsafe { GetAsyncKeyState(VK_LBUTTON) as u16 & 0x8000 != 0 }
}

fn hwnd_of(window: &Window) -> Option<Hwnd> {
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as Hwnd),
        _ => None,
    }
}

pub fn window_handle(window: &Window) -> Option<isize> {
    hwnd_of(window).map(|hwnd| hwnd as isize)
}

pub fn install(window: &Window) {
    let Some(hwnd) = hwnd_of(window) else {
        return;
    };
    // Installed on the HWND's owning UI thread. Reference data is only a bool;
    // there are no borrowed pointers or allocated callback state to outlive it.
    if unsafe { SetWindowSubclass(hwnd, mouse_proc, SUBCLASS_ID, 0) } == 0 {
        log::error!("Could not install capsule mouse handling");
    }
}

/// The cursor in the window's client coordinates, in device pixels.
pub fn cursor_in_client(window: &Window) -> Option<(f32, f32)> {
    let hwnd = hwnd_of(window)?;
    let mut point = Point::default();
    unsafe {
        if GetCursorPos(&mut point) == 0 || ScreenToClient(hwnd, &mut point) == 0 {
            return None;
        }
    }
    Some((point.x as f32, point.y as f32))
}

/// The cursor on the virtual desktop, in device pixels.
pub fn cursor_screen() -> Option<(f32, f32)> {
    let mut point = Point::default();
    unsafe {
        if GetCursorPos(&mut point) == 0 {
            return None;
        }
    }
    Some((point.x as f32, point.y as f32))
}

/// `(left, top, right, bottom)` of the native window, in device pixels.
pub fn window_rect(window: &Window) -> Option<(f32, f32, f32, f32)> {
    let hwnd = hwnd_of(window)?;
    let mut rect = Rect::default();
    unsafe {
        if GetWindowRect(hwnd, &mut rect) == 0 {
            return None;
        }
    }
    Some((
        rect.left as f32,
        rect.top as f32,
        rect.right as f32,
        rect.bottom as f32,
    ))
}

/// `(left, top, right, bottom)` of the monitor nearest a screen point, in device
/// pixels.
///
/// The *monitor*, not its work area: the capsule is a floating overlay and is
/// allowed to sit over the taskbar. Windows answers with the closest display
/// when the point lies on one that has since been unplugged, which is what keeps
/// a remembered position reachable.
pub fn monitor_rect(point: (f32, f32)) -> Option<(f32, f32, f32, f32)> {
    let probe = Point {
        x: point.0.round() as i32,
        y: point.1.round() as i32,
    };
    let monitor = unsafe { MonitorFromPoint(probe, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_null() {
        return None;
    }
    let mut info = MonitorInfo {
        size: std::mem::size_of::<MonitorInfo>() as u32,
        ..MonitorInfo::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return None;
    }
    Some((
        info.monitor.left as f32,
        info.monitor.top as f32,
        info.monitor.right as f32,
        info.monitor.bottom as f32,
    ))
}

/// Ask the foreground executor to reposition the window. See
/// [`crate::windowing::move_window`] for why this is not done inline.
pub fn move_window(cx: &App, hwnd: isize, origin: (f32, f32)) {
    let hwnd = hwnd as Hwnd;
    cx.foreground_executor()
        .spawn(async move {
            unsafe {
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    origin.0.round() as i32,
                    origin.1.round() as i32,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        })
        .detach();
}

/// Whether a right-button message landed on the capsule rather than the panel.
/// `screen_coords` is true for the non-client variants of the message, whose
/// coordinates are relative to the desktop instead of the client area.
unsafe fn cursor_over_header(hwnd: Hwnd, lparam: isize, screen_coords: bool) -> bool {
    let mut point = Point {
        x: (lparam & 0xffff) as i16 as i32,
        y: ((lparam >> 16) & 0xffff) as i16 as i32,
    };
    if screen_coords && unsafe { ScreenToClient(hwnd, &mut point) } == 0 {
        return false;
    }
    super::contains_header(super::geometry(), point.x as f32, point.y as f32)
}

/// Clip the window to the painted shapes.
///
/// The popup is a rectangle, and both its non-client frame and the unpainted
/// part of its surface composite as an opaque plate over the desktop. So the
/// visible shape has to be handed to the system as a window region. The region
/// is grown outward from the painted edge — rectangle and corner radius
/// together, which keeps the offset uniform — so the mask can never cut the
/// antialiased outline that GPUI draws underneath it.
pub fn apply_region(window: &Window, geometry: super::Geometry, slack: f32) {
    let Some(hwnd) = hwnd_of(window) else {
        return;
    };
    let Some((border_x, border_y)) = (unsafe { client_offset(hwnd) }) else {
        return;
    };
    if geometry.width <= 0. || geometry.header <= 0. {
        return;
    }
    // One uniform offset for every rect, and the corner radius grows with it,
    // so the mask is the painted shape moved outward rather than a second,
    // slightly different shape laid over it.
    let outset = (super::REGION_OUTSET + slack) * geometry.unit;
    let region = unsafe {
        let region = CreateRectRgn(0, 0, 0, 0);
        if region.is_null() {
            return;
        }
        let add = |left: f32, top: f32, width: f32, height: f32, radius: f32| {
            let part = round_rect(
                border_x + left - outset,
                border_y + top - outset,
                width + outset * 2.,
                height + outset * 2.,
                radius + outset,
            );
            if !part.is_null() {
                CombineRgn(region, region, part, RGN_OR);
                DeleteObject(part);
            }
        };
        add(
            geometry.bleed,
            geometry.bleed,
            geometry.width,
            geometry.header,
            geometry.header * 0.5,
        );
        if geometry.panel > 0.5 {
            add(
                geometry.bleed,
                geometry.panel_top,
                geometry.width,
                geometry.panel,
                geometry.panel_radius,
            );
            // Keep the hover path between capsule and panel connected.
            add(
                geometry.bleed,
                geometry.bleed + geometry.header,
                geometry.width,
                geometry.panel_top - geometry.bleed - geometry.header,
                0.,
            );
        }
        region
    };
    unsafe {
        if SetWindowRgn(hwnd, region, 1) == 0 {
            DeleteObject(region);
        }
    }
}

/// Where the client area starts inside the window rectangle. Windows draws a
/// non-client frame for this popup even though GPUI paints none of it, so a
/// region given in client coordinates has to be shifted by this much.
unsafe fn client_offset(hwnd: Hwnd) -> Option<(f32, f32)> {
    let mut rect = Rect::default();
    let mut origin = Point::default();
    unsafe {
        if GetWindowRect(hwnd, &mut rect) == 0 || ClientToScreen(hwnd, &mut origin) == 0 {
            return None;
        }
    }
    Some(((origin.x - rect.left) as f32, (origin.y - rect.top) as f32))
}

/// A rounded rectangle in physical pixels. Windows takes the full width and
/// height of the corner ellipse, so the radius is doubled.
unsafe fn round_rect(left: f32, top: f32, width: f32, height: f32, radius: f32) -> Hrgn {
    let radius = radius.min(width * 0.5).min(height * 0.5).max(0.) * 2.;
    unsafe {
        CreateRoundRectRgn(
            left.round() as i32,
            top.round() as i32,
            (left + width).round() as i32,
            (top + height).round() as i32,
            radius.round() as i32,
            radius.round() as i32,
        )
    }
}

unsafe extern "system" fn mouse_proc(    hwnd: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
    id: usize,
    _data: usize,
) -> isize {
    // All HWND calls run synchronously on the window's UI thread. Forward every
    // unrelated message through the subclass chain, including destruction.
    unsafe {
        match message {
            // The popup is a rectangle; the shapes inside it are not. Answering
            // `HTTRANSPARENT` keeps the transparent margin out of the way of the
            // desktop while leaving the painted edge fully antialiased, which a
            // window region (a 1-bit mask) cannot do.
            WM_NCHITTEST => {
                let mut point = Point {
                    x: (lparam & 0xffff) as i16 as i32,
                    y: ((lparam >> 16) & 0xffff) as i16 as i32,
                };
                if ScreenToClient(hwnd, &mut point) != 0
                    && !super::contains_point(super::geometry(), point.x as f32, point.y as f32)
                {
                    return HTTRANSPARENT;
                }
            }
            WM_RBUTTONDOWN | WM_RBUTTONDBLCLK | WM_NCRBUTTONDOWN => {
                // Quitting is irreversible, so it is bound to the capsule rather
                // than to the whole window: a stray right click on a slider or a
                // track row must not close the app.
                let non_client = message == WM_NCRBUTTONDOWN;
                if !cursor_over_header(hwnd, lparam, non_client) {
                    return DefSubclassProc(hwnd, message, wparam, lparam);
                }
                RIGHT_PRESSED.store(true, Ordering::Release);
                SetCapture(hwnd);
                return 0;
            }
            WM_RBUTTONUP | WM_NCRBUTTONUP if RIGHT_PRESSED.swap(false, Ordering::AcqRel) => {
                if GetCapture() == hwnd {
                    ReleaseCapture();
                }
                // Defer destruction until after the entire release message is
                // consumed; no right-button event reaches DefWindowProc/GPUI.
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                return 0;
            }
            WM_CONTEXTMENU => return 0,
            WM_CANCELMODE | WM_CAPTURECHANGED => {
                RIGHT_PRESSED.store(false, Ordering::Release);
                if message == WM_CANCELMODE && GetCapture() == hwnd {
                    ReleaseCapture();
                }
            }
            WM_NCDESTROY => {
                RemoveWindowSubclass(hwnd, mouse_proc, id);
            }
            _ => {}
        }
        DefSubclassProc(hwnd, message, wparam, lparam)
    }
}
