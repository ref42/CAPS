//! Live provider check: cargo run --example music_playback -- qishui "晴天"
//! Search providers accept a song/artist query.
#![allow(dead_code)]
#[path = "../src/providers/mod.rs"]
mod providers;
#[path = "../src/music/track.rs"]
pub(crate) mod track;
// Providers share this track type; the standalone sample needs no app or UI.
mod music {
    pub(crate) use super::track;
}
use providers::{kugou, qishui, qqmusic, shitease, streaming};

use rodio::Source;
use std::time::{Duration, Instant};
use streaming::{RemoteMedia, media_client};

#[tokio::main]
async fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let provider = args.next().ok_or("Expected provider and query/ID")?;
    let query = args.next().ok_or("Expected query or video ID")?;
    let started = Instant::now();
    let remote = match provider.as_str() {
        "qishui" => {
            let songs = qishui::search(query, 8).await?;
            println!("Qishui search: {} available catalogue results", songs.len());
            let song = songs.first().ok_or("No search results")?;
            println!("{} / {} / {:?}s", song.name, song.artist, song.duration);
            println!("Lyrics: {} bytes", qishui::lyric(&song.id).await?.len());
            qishui::resolve_audio(&song.id).await?
        }
        "netease" => {
            let songs = shitease::search_shitease_songs(query, Some(8), None).await?;
            let song: track::Track = songs.into_iter().next().ok_or("No search results")?.into();
            println!("{} / {}", song.name, song.artist);
            let url = shitease::get_shitease_song_url(song.id, Some("exhigh".into()), None)
                .await?
                .url
                .ok_or("No stream URL")?;
            RemoteMedia {
                client: media_client("https://music.163.com/")?,
                urls: vec![url],
            }
        }
        "qqmusic" => {
            let song = qqmusic::search(query, 8)
                .await?
                .into_iter()
                .next()
                .ok_or("No search results")?;
            println!("{} / {}", song.name, song.artist);
            RemoteMedia {
                client: media_client("https://y.qq.com/")?,
                urls: vec![qqmusic::stream_url_with_media(&song.songmid, &song.media_mid).await?],
            }
        }
        "kugou" => {
            let song = kugou::search(query, 8)
                .await?
                .into_iter()
                .next()
                .ok_or("No search results")?;
            println!("{} / {}", song.name, song.artist);
            RemoteMedia {
                client: media_client("https://www.kugou.com/")?,
                urls: vec![
                    kugou::stream_url(
                        &song.hash,
                        &song.quality_hash,
                        &song.album_id,
                        &song.album_audio_id,
                    )
                    .await?,
                ],
            }
        }
        _ => return Err("Unknown provider".into()),
    };
    let resolved = started.elapsed();
    let probe = remote
        .client
        .get(&remote.urls[0])
        .header("Range", "bytes=0-0")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    println!(
        "Media response: {}, range {:?}, length {:?}",
        probe.status(),
        probe.headers().get("content-range"),
        probe.content_length()
    );
    drop(probe);
    let target =
        std::env::temp_dir().join(format!("caps-live-{provider}-{}.audio", std::process::id()));
    let source = remote.prepare(&target).await?;
    let seek_seconds = source
        .total_duration()
        .map(|d| (d.as_secs_f64() / 2.0).min(60.0))
        .unwrap_or(60.0);
    let control = source.control();
    println!(
        "Audio ready after {:?} (resolution {:?}); downloaded {:?}",
        started.elapsed(),
        resolved,
        control.downloaded()
    );
    let mut output = rodio::DeviceSinkBuilder::open_default_sink().map_err(|e| e.to_string())?;
    output.log_on_drop(false);
    let player = rodio::Player::connect_new(output.mixer());
    player.set_volume(0.0);
    player.append(source);
    tokio::time::sleep(Duration::from_secs(2)).await;
    if let Some(error) = control.error() {
        return Err(error);
    }
    println!(
        "Playback {:.2}s, buffering {}",
        control.position(),
        control.buffering()
    );
    if control.position() < 0.5 {
        return Err("Output did not advance".into());
    }
    control.seek(Duration::from_secs_f64(seek_seconds));
    tokio::time::timeout(Duration::from_secs(40), async {
        while control.position() < seek_seconds + 0.5 {
            if let Some(error) = control.error() {
                return Err(error);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok::<_, String>(())
    })
    .await
    .map_err(|_| "Seek timed out")??;
    println!(
        "Seek resumed at {:.2}s; downloaded {:?}",
        control.position(),
        control.downloaded()
    );
    control.cancel();
    player.stop();
    let _ = std::fs::remove_file(target);
    Ok(())
}
