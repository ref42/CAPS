//! Read-only probe or download using the same Douyin path as CAPS.
#[path = "../src/douyin.rs"]
mod douyin;
#[path = "../src/download.rs"]
mod download;
#[allow(dead_code)]
#[path = "../src/streaming.rs"]
mod streaming;

#[tokio::main]
async fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let link = args
        .next()
        .ok_or("Usage: cargo run --example douyin_audio -- URL [output.audio]")?;
    if !douyin::is_supported_url(&link) {
        return Err("Expected a Douyin link or share message.".into());
    }
    let started = std::time::Instant::now();
    let result = douyin::preview(&link).await?;
    println!(
        "{}",
        serde_json::json!({"id":result.id,"title":result.title,"author":result.author,"duration_seconds":result.duration,"download_bytes":result.size,"cover_available":!result.cover.is_empty()})
    );
    if let Some(output) = args.next() {
        if output == "--stream" {
            let target = args.next().ok_or("--stream requires a new cache path")?;
            let target = std::path::Path::new(&target);
            if target.exists() {
                return Err("Output already exists.".into());
            }
            let resolved = started.elapsed();
            let source = douyin::remote(&result)?.prepare(target).await?;
            let control = source.control();
            println!(
                "Audio ready after {:?} (resolution {:?}); downloaded {:?}",
                started.elapsed(),
                resolved,
                control.downloaded()
            );
            let mut output =
                rodio::DeviceSinkBuilder::open_default_sink().map_err(|e| e.to_string())?;
            output.log_on_drop(false);
            let player = rodio::Player::connect_new(output.mixer());
            player.set_volume(0.0); // Verify the real output path without blasting a DJ mix.
            player.append(source);
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            println!(
                "Playback position {:.2}s, buffering {}",
                control.position(),
                control.buffering()
            );
            if control.position() < 0.5 {
                return Err("Playback did not advance.".into());
            }
            let seek_to = result.duration.unwrap_or(60) as f64 * 0.75;
            let seeking = std::time::Instant::now();
            control.seek(std::time::Duration::from_secs_f64(seek_to));
            while control.position() < seek_to + 0.5 {
                if let Some(error) = control.error() {
                    return Err(error);
                }
                if seeking.elapsed().as_secs() >= 45 {
                    return Err("Seek timed out.".into());
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            println!(
                "Seek to {seek_to:.2}s resumed after {:?}; downloaded {:?}",
                seeking.elapsed(),
                control.downloaded()
            );
            control.cancel();
            player.stop();
            return Ok(());
        }
        let path = std::path::Path::new(&output);
        if path.exists() {
            return Err("Output already exists; choose a new path.".into());
        }
        let mut last = 0;
        douyin::download(&result, path, |received, total| {
            if received.saturating_sub(last) >= 32 * 1024 * 1024 {
                eprintln!("Downloaded {received} / {total:?} bytes");
                last = received;
            }
        })
        .await?;
        println!("Saved and verified decodable audio: {}", path.display());
    }
    Ok(())
}
