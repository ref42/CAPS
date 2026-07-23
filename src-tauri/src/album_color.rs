pub async fn spectrum_colors(cover_url: String) -> Option<(String, String)> {
    let bytes = reqwest::get(cover_url).await.ok()?.bytes().await.ok()?;
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

    if total_weight <= f32::EPSILON {
        return None;
    }

    let accent = normalize_color(
        red / total_weight,
        green / total_weight,
        blue / total_weight,
    );
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
