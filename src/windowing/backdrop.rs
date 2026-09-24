//! What the capsule is floating over.
//!
//! The capsule's surface can be faded to nothing, so its text cannot take its
//! contrast from that surface: it has to be the opposite of the pixels behind the
//! window. Those pixels are read here, from the `BLEED` margin around the capsule
//! and the panel — that margin is transparent *and* outside the window's own
//! region, so a ring of points sampled along it shows the desktop, a browser page
//! or a document, and never the capsule itself.
//!
//! The whole rectangle is read in one `BitBlt` into a DIB section. Reading the
//! screen point by point with `GetPixel` stalls the frame it runs on — which is
//! felt as a stutter every time the panel opens or folds — because each of those
//! calls synchronises with the compositor.
use gpui_kit::Window;
use std::ffi::c_void;

type Hwnd = *mut c_void;
type Hdc = *mut c_void;
type Hbitmap = *mut c_void;
type Hgdiobj = *mut c_void;

const BI_RGB: u32 = 0;
const DIB_RGB_COLORS: u32 = 0;
const SRCCOPY: u32 = 0x00CC_0020;
/// Include layered windows, so the samples are what the user is looking at.
const CAPTUREBLT: u32 = 0x4000_0000;

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
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_pels_per_meter: i32,
    y_pels_per_meter: i32,
    clr_used: u32,
    clr_important: u32,
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
struct RgbQuad {
    blue: u8,
    green: u8,
    red: u8,
    reserved: u8,
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
struct BitmapInfo {
    header: BitmapInfoHeader,
    colors: [RgbQuad; 1],
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetDC(hwnd: Hwnd) -> Hdc;
    fn ReleaseDC(hwnd: Hwnd, hdc: Hdc) -> i32;
    fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateCompatibleDC(hdc: Hdc) -> Hdc;
    fn DeleteDC(hdc: Hdc) -> i32;
    fn CreateDIBSection(
        hdc: Hdc,
        info: *const BitmapInfo,
        usage: u32,
        bits: *mut *mut c_void,
        section: *mut c_void,
        offset: u32,
    ) -> Hbitmap;
    fn SelectObject(hdc: Hdc, object: Hgdiobj) -> Hgdiobj;
    fn DeleteObject(object: Hgdiobj) -> i32;
    fn BitBlt(dst: Hdc, x: i32, y: i32, w: i32, h: i32, src: Hdc, sx: i32, sy: i32, rop: u32) -> i32;
}

/// The desktop behind the capsule, as samples of sRGB with the weight of the
/// area each one stands for.
///
/// A uniform grid over the window's whole rectangle, so the count of samples is
/// a count of area and the larger part of the backdrop is what wins. Our own
/// surface is faded by the user's opacity, so at the low opacities where this
/// decision matters most these points are very nearly the desktop itself — and
/// at high opacity they are the capsule's own dark surface, which is the right
/// answer there too.
///
/// Sampling only the transparent margin would be wrong in an easy case: put the
/// capsule on a dark editor with a white document around it and the margin votes
/// white while every pixel under the capsule is dark. `None` when the window
/// cannot be read.
pub fn sample(window: &Window) -> Option<Vec<([u8; 3], f32)>> {
    let hwnd = super::mouse::window_handle(window)? as Hwnd;
    let mut rect = Rect::default();
    unsafe {
        if GetWindowRect(hwnd, &mut rect) == 0 {
            return None;
        }
    }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width < 32 || height < 32 {
        return None;
    }
    let pixels = grab(rect, width, height)?;
    const COLS: i32 = 12;
    const ROWS: i32 = 8;
    let stride = width as usize * 4;
    let mut out = Vec::with_capacity((COLS * ROWS) as usize);
    for row in 0..ROWS {
        for col in 0..COLS {
            // Cell centres, so no sample sits on the very edge of the window.
            let px = ((col as f32 + 0.5) / COLS as f32 * width as f32) as i32;
            let py = ((row as f32 + 0.5) / ROWS as f32 * height as f32) as i32;
            if px < 0 || py < 0 || px >= width || py >= height {
                continue;
            }
            let at = py as usize * stride + px as usize * 4;
            // A 32-bit DIB section is laid out blue, green, red, alpha.
            out.push(([pixels[at + 2], pixels[at + 1], pixels[at]], 1.));
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

/// One screen read of `rect`, top-down and 32 bits per pixel.
fn grab(rect: Rect, width: i32, height: i32) -> Option<Vec<u8>> {
    let screen = std::ptr::null_mut();
    let screen_dc = unsafe { GetDC(screen) };
    if screen_dc.is_null() {
        return None;
    }
    let mem_dc = unsafe { CreateCompatibleDC(screen_dc) };
    if mem_dc.is_null() {
        let _ = unsafe { ReleaseDC(screen, screen_dc) };
        return None;
    }
    let info = BitmapInfo {
        header: BitmapInfoHeader {
            size: std::mem::size_of::<BitmapInfoHeader>() as u32,
            width,
            // Negative height: the rows come back starting at the top.
            height: -height,
            planes: 1,
            bit_count: 32,
            compression: BI_RGB,
            ..BitmapInfoHeader::default()
        },
        colors: [RgbQuad::default()],
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let dib = unsafe {
        CreateDIBSection(
            screen_dc,
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            std::ptr::null_mut(),
            0,
        )
    };
    if dib.is_null() || bits.is_null() {
        unsafe {
            DeleteDC(mem_dc);
            ReleaseDC(screen, screen_dc);
        }
        return None;
    }
    let previous = unsafe { SelectObject(mem_dc, dib) };
    let copied = unsafe {
        BitBlt(
            mem_dc,
            0,
            0,
            width,
            height,
            screen_dc,
            rect.left,
            rect.top,
            SRCCOPY | CAPTUREBLT,
        )
    } != 0;
    let out = if copied {
        let len = width as usize * height as usize * 4;
        // The section stays mapped until it is deleted, which happens below.
        Some(unsafe { std::slice::from_raw_parts(bits as *const u8, len) }.to_vec())
    } else {
        None
    };
    unsafe {
        SelectObject(mem_dc, previous);
        DeleteObject(dib);
        DeleteDC(mem_dc);
        ReleaseDC(screen, screen_dc);
    }
    out
}
