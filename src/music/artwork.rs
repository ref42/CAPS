//! Album decoding and rotation happen off the UI thread.
use gpui_kit::{Image, ImageFormat, RenderImage};
use image::{Frame, Rgba, RgbaImage, imageops::FilterType};
use std::{
    sync::{Arc, LazyLock},
    time::Duration,
};

/// The shared artwork client. Building it can fail (a missing TLS backend, for
/// instance); artwork then simply never loads, which is better than taking the
/// window down over a cover image.
static HTTP: LazyLock<Option<reqwest::Client>> = LazyLock::new(|| {
    match reqwest::Client::builder()
        .user_agent("Mozilla/5.0 CAPS")
        .timeout(Duration::from_secs(15))
        .build()
    {
        Ok(client) => Some(client),
        Err(err) => {
            log::error!("artwork: could not build the HTTP client: {err}");
            None
        }
    }
});
static DOWNLOADS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(6);

pub async fn load(url: &str) -> Option<Arc<Image>> {
    if url.is_empty() {
        return None;
    }
    let _permit = DOWNLOADS.acquire().await.ok()?;
    let bytes = if let Some(path) = url.strip_prefix("file:///") {
        let decoded = urlencoding::decode(path).ok()?;
        // Local metadata paths are already filesystem paths, including Windows
        // extended-length paths. Only file:// URI escaping needs decoding.
        tokio::fs::read(decoded.as_ref()).await.ok()?
    } else if std::path::Path::new(url).is_file() {
        tokio::fs::read(url).await.ok()?
    } else {
        let url = if url.starts_with("//") {
            format!("https:{url}")
        } else {
            url.into()
        };
        HTTP.as_ref()?
            .get(url)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .bytes()
            .await
            .ok()?
            .to_vec()
    };
    tokio::task::spawn_blocking(move || {
        let image = image::load_from_memory(&bytes).ok()?.thumbnail(180, 180);
        let mut png = std::io::Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png).ok()?;
        Some(Arc::new(Image::from_bytes(
            ImageFormat::Png,
            png.into_inner(),
        )))
    })
    .await
    .ok()
    .flatten()
}

pub const SPIN_FRAMES: usize = 180;
pub const SPIN_SECONDS: f32 = 9.;

/// GPUI's bitmap painter has no rotation transform. Keep a bounded set of
/// precomputed BGRA frames for the current record only (about 10 MB).
pub fn spin_frames(cover: &Image) -> Option<Arc<RenderImage>> {
    let square = image::load_from_memory(cover.bytes())
        .ok()?
        .resize_to_fill(120, 120, FilterType::Lanczos3)
        .to_rgba8();
    let frames = (0..SPIN_FRAMES)
        .map(|index| {
            Frame::new(rotate_bgra(
                &square,
                index as f32 * std::f32::consts::TAU / SPIN_FRAMES as f32,
            ))
        })
        .collect::<Vec<_>>();
    Some(Arc::new(RenderImage::new(frames)))
}

fn rotate_bgra(source: &RgbaImage, angle: f32) -> RgbaImage {
    let size = source.width();
    let center = (size as f32 - 1.) / 2.;
    let (sin, cos) = angle.sin_cos();
    RgbaImage::from_fn(size, size, |x, y| {
        let dx = x as f32 - center;
        let dy = y as f32 - center;
        let sx = (cos * dx + sin * dy + center).clamp(0., size as f32 - 1.);
        let sy = (-sin * dx + cos * dy + center).clamp(0., size as f32 - 1.);
        let (x0, y0) = (sx.floor() as u32, sy.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(size - 1), (y0 + 1).min(size - 1));
        let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
        let p = [
            source[(x0, y0)],
            source[(x1, y0)],
            source[(x0, y1)],
            source[(x1, y1)],
        ];
        let channel = |i: usize| {
            ((p[0][i] as f32 * (1. - fx) + p[1][i] as f32 * fx) * (1. - fy)
                + (p[2][i] as f32 * (1. - fx) + p[3][i] as f32 * fx) * fy)
                .round() as u8
        };
        Rgba([channel(2), channel(1), channel(0), channel(3)])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quarter_turn_rotates_art_clockwise_and_converts_to_bgra() {
        let mut source = RgbaImage::from_pixel(3, 3, Rgba([0, 0, 0, 255]));
        source[(1, 0)] = Rgba([220, 20, 10, 255]);
        let rotated = rotate_bgra(&source, std::f32::consts::FRAC_PI_2);
        assert_eq!(rotated[(2, 1)], Rgba([10, 20, 220, 255]));
        assert_eq!(rotated[(1, 0)], Rgba([0, 0, 0, 255]));
    }
}
