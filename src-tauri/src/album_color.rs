pub async fn spectrum_colors(cover_url: String) -> Option<(String, String)> {
    let bytes = if cover_url.starts_with("file:///") {
        std::fs::read(file_url_path(&cover_url)?).ok()?
    } else {
        reqwest::get(cover_url)
            .await
            .ok()?
            .bytes()
            .await
            .ok()?
            .to_vec()
    };
    tokio::task::spawn_blocking(move || extract_colors(&bytes))
        .await
        .ok()
        .flatten()
}

fn extract_colors(bytes: &[u8]) -> Option<(String, String)> {
    let image = image::load_from_memory(bytes)
        .ok()?
        .thumbnail(48, 48)
        .to_rgba8();
    let mut total_weight = 0.0_f32;
    let mut red = 0.0_f32;
    let mut green = 0.0_f32;
    let mut blue = 0.0_f32;
    let mut muted_total_weight = 0.0_f32;
    let mut muted_red = 0.0_f32;
    let mut muted_green = 0.0_f32;
    let mut muted_blue = 0.0_f32;

    for pixel in image.pixels() {
        let [r, g, b, a] = pixel.0;
        if a < 128 {
            continue;
        }

        let rf = r as f32 / 255.0;
        let gf = g as f32 / 255.0;
        let bf = b as f32 / 255.0;
        let max = rf.max(gf).max(bf);
        let min = rf.min(gf).min(bf);
        let chroma = max - min;
        let saturation = if max == 0.0 { 0.0 } else { chroma / max };
        let luma = 0.2126 * rf + 0.7152 * gf + 0.0722 * bf;
        if (0.04..=0.96).contains(&luma) {
            let neutral_weight = 0.2 + luma.powf(1.2);
            muted_red += rf * neutral_weight;
            muted_green += gf * neutral_weight;
            muted_blue += bf * neutral_weight;
            muted_total_weight += neutral_weight;
        }

        if saturation < 0.14 || !(0.08..=0.92).contains(&luma) {
            continue;
        }

        let luma_balance = 1.0 - (luma - 0.58).abs().min(0.58) / 0.58;
        let weight = saturation.powf(1.45) * (0.35 + luma_balance);
        red += rf * weight;
        green += gf * weight;
        blue += bf * weight;
        total_weight += weight;
    }

    let accent = if total_weight > f32::EPSILON {
        normalize_color(
            red / total_weight,
            green / total_weight,
            blue / total_weight,
        )
    } else if muted_total_weight > f32::EPSILON {
        normalize_muted_color(
            muted_red / muted_total_weight,
            muted_green / muted_total_weight,
            muted_blue / muted_total_weight,
        )
    } else {
        return None;
    };
    let highlight = lift_color(accent, 0.46);
    Some((css_rgb(highlight), css_rgb(accent)))
}

fn normalize_color(red: f32, green: f32, blue: f32) -> [u8; 3] {
    let luma = 0.2126 * red + 0.7152 * green + 0.0722 * blue;
    let lift = if luma < 0.42 { 0.42 - luma } else { 0.0 };
    [
        channel(red + lift * 0.8),
        channel(green + lift * 0.8),
        channel(blue + lift * 0.8),
    ]
}

fn normalize_muted_color(red: f32, green: f32, blue: f32) -> [u8; 3] {
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    if max - min < 0.035 {
        let luma = (0.2126 * red + 0.7152 * green + 0.0722 * blue).clamp(0.50, 0.82);
        return [channel(luma), channel(luma), channel(luma)];
    }
    normalize_color(red, green, blue)
}

fn lift_color(color: [u8; 3], amount: f32) -> [u8; 3] {
    [
        mix_channel(color[0], 255, amount),
        mix_channel(color[1], 255, amount),
        mix_channel(color[2], 255, amount),
    ]
}

fn mix_channel(value: u8, target: u8, amount: f32) -> u8 {
    channel(value as f32 / 255.0 * (1.0 - amount) + target as f32 / 255.0 * amount)
}

fn channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn css_rgb(color: [u8; 3]) -> String {
    format!("rgb({}, {}, {})", color[0], color[1], color[2])
}

fn file_url_path(url: &str) -> Option<std::path::PathBuf> {
    let raw = url.strip_prefix("file:///")?;
    let decoded = urlencoding::decode(raw).ok()?;
    Some(std::path::PathBuf::from(decoded.replace('/', "\\")))
}

#[cfg(test)]
mod tests {
    use super::extract_colors;
    use image::{ImageBuffer, ImageFormat, Rgba};
    use std::io::Cursor;

    #[test]
    fn extracts_neutral_colors_from_monochrome_cover() {
        let mut image = ImageBuffer::from_pixel(8, 8, Rgba([8_u8, 8, 8, 255]));
        for y in 2..6 {
            for x in 2..6 {
                image.put_pixel(x, y, Rgba([235_u8, 235, 235, 255]));
            }
        }

        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        let colors = extract_colors(&bytes).expect("monochrome cover should produce colors");

        assert_ne!(colors.0, "rgb(255, 196, 224)");
        assert_ne!(colors.1, "rgb(255, 105, 180)");
    }
}
