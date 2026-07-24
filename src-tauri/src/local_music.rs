use crate::storage;
use crate::track::{SOURCE_LOCAL, Track};
use image::ImageFormat;
use std::collections::HashSet;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use symphonia::core::common::Limit;
use symphonia::core::formats::FormatOptions;
use symphonia::core::formats::probe::Hint;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, MetadataRevision, StandardTag, StandardVisualKey};

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "ogg", "m4a", "aac"];
const MAX_SCAN_ITEMS: usize = 8_000;
const MAX_VISUAL_BYTES: usize = 8 * 1024 * 1024;
const COVER_THUMBNAIL_SIZE: u32 = 320;

pub fn load_all_batched(
    folder: &str,
    batch_size: usize,
    mut on_batch: impl FnMut(Vec<Track>, usize) -> bool,
) -> Result<usize, String> {
    if folder.trim().is_empty() {
        return Ok(0);
    }
    scan_batched(folder, MAX_SCAN_ITEMS, batch_size.max(1), &mut on_batch)
}

pub fn read_audio(path: &str) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|err| format!("Local file read failed: {err}"))
}

pub fn read_lyrics(path: &str) -> String {
    let audio_path = Path::new(path);
    let Some(stem) = audio_path.file_stem().and_then(|value| value.to_str()) else {
        return String::new();
    };
    let lrc_path = audio_path.with_file_name(format!("{stem}.lrc"));
    fs::read_to_string(lrc_path).unwrap_or_default()
}

fn scan_batched(
    folder: &str,
    limit: usize,
    batch_size: usize,
    on_batch: &mut impl FnMut(Vec<Track>, usize) -> bool,
) -> Result<usize, String> {
    let root = PathBuf::from(folder.trim());
    if !root.is_dir() {
        return Err("Local music folder is not valid.".to_string());
    }
    let mut batch = Vec::with_capacity(batch_size);
    let mut seen = HashSet::new();
    let mut stack = vec![root];
    let mut total = 0;
    while let Some(path) = stack.pop() {
        let Ok(entries) = fs::read_dir(path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !is_audio_file(&path) {
                continue;
            }
            let id = path.to_string_lossy().to_string();
            if !seen.insert(id.clone()) {
                continue;
            }
            batch.push(track_from_path(&path, id));
            total += 1;
            if batch.len() >= batch_size {
                if !on_batch(std::mem::take(&mut batch), total) {
                    return Ok(total);
                }
                batch = Vec::with_capacity(batch_size);
            }
            if total >= limit {
                if !batch.is_empty() {
                    let _ = on_batch(batch, total);
                }
                return Ok(total);
            }
        }
    }
    if !batch.is_empty() {
        let _ = on_batch(batch, total);
    }
    Ok(total)
}

fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|ext| {
            AUDIO_EXTENSIONS
                .iter()
                .any(|candidate| ext.eq_ignore_ascii_case(candidate))
        })
        .unwrap_or(false)
}

fn track_from_path(path: &Path, id: String) -> Track {
    let fallback = fallback_info(path);
    let metadata = read_metadata(path);
    let name = metadata
        .as_ref()
        .and_then(|meta| meta.title.clone())
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| fallback.name.clone());
    let artist = metadata
        .as_ref()
        .and_then(|meta| meta.artist.clone())
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| fallback.artist.clone());
    let album = metadata
        .as_ref()
        .and_then(|meta| meta.album.clone())
        .filter(|text| !text.trim().is_empty())
        .unwrap_or(fallback.album);
    let cover = metadata
        .and_then(|meta| meta.cover)
        .and_then(|cover| cache_cover(path, &cover))
        .unwrap_or_default();
    Track {
        source: SOURCE_LOCAL.to_string(),
        id,
        name,
        artist,
        album,
        cover,
    }
}

#[derive(Clone)]
struct LocalMetadata {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    cover: Option<Vec<u8>>,
}

struct FallbackInfo {
    name: String,
    artist: String,
    album: String,
}

fn fallback_info(path: &Path) -> FallbackInfo {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Local track")
        .trim();
    let (artist, name) = stem
        .split_once(" - ")
        .map(|(artist, name)| (artist.trim(), name.trim()))
        .unwrap_or(("Local file", stem));
    let album = path
        .parent()
        .and_then(|value| value.file_name())
        .and_then(|value| value.to_str())
        .unwrap_or("Local library")
        .to_string();
    FallbackInfo {
        name: name.to_string(),
        artist: artist.to_string(),
        album,
    }
}

fn read_metadata(path: &Path) -> Option<LocalMetadata> {
    let file = fs::File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(ext);
    }
    let fmt_opts = FormatOptions::default();
    let meta_opts = MetadataOptions::default()
        .limit_tag_bytes(Limit::Maximum(512 * 1024))
        .limit_visual_bytes(Limit::Maximum(MAX_VISUAL_BYTES));
    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, fmt_opts, meta_opts)
        .ok()?;
    let revision = format.metadata().skip_to_latest()?.clone();
    metadata_from_revision(&revision)
}

fn metadata_from_revision(revision: &MetadataRevision) -> Option<LocalMetadata> {
    let mut metadata = LocalMetadata {
        title: None,
        artist: None,
        album: None,
        cover: None,
    };

    for tag in &revision.media.tags {
        apply_tag(&mut metadata, tag);
    }
    for track in &revision.per_track {
        for tag in &track.metadata.tags {
            apply_tag(&mut metadata, tag);
        }
    }

    metadata.cover = cover_from_revision(revision);
    if metadata.title.is_some()
        || metadata.artist.is_some()
        || metadata.album.is_some()
        || metadata.cover.is_some()
    {
        Some(metadata)
    } else {
        None
    }
}

fn apply_tag(metadata: &mut LocalMetadata, tag: &symphonia::core::meta::Tag) {
    match &tag.std {
        Some(StandardTag::TrackTitle(value)) if metadata.title.is_none() => {
            metadata.title = Some(value.to_string());
        }
        Some(StandardTag::Artist(value)) if metadata.artist.is_none() => {
            metadata.artist = Some(value.to_string());
        }
        Some(StandardTag::AlbumArtist(value)) if metadata.artist.is_none() => {
            metadata.artist = Some(value.to_string());
        }
        Some(StandardTag::Album(value)) if metadata.album.is_none() => {
            metadata.album = Some(value.to_string());
        }
        _ => {}
    }
}

fn cover_from_revision(revision: &MetadataRevision) -> Option<Vec<u8>> {
    let mut visuals = revision.media.visuals.iter().chain(
        revision
            .per_track
            .iter()
            .flat_map(|track| track.metadata.visuals.iter()),
    );
    let fallback = visuals.next();
    revision
        .media
        .visuals
        .iter()
        .chain(
            revision
                .per_track
                .iter()
                .flat_map(|track| track.metadata.visuals.iter()),
        )
        .find(|visual| {
            visual.usage.is_none()
                || visual
                    .usage
                    .is_some_and(|usage| usage == StandardVisualKey::FrontCover)
        })
        .or(fallback)
        .map(|visual| visual.data.to_vec())
}

fn cache_cover(audio_path: &Path, bytes: &[u8]) -> Option<String> {
    let cache_dir = storage::cover_cache_path()?;
    fs::create_dir_all(&cache_dir).ok()?;
    let cover_path = cache_dir.join(format!("{:016x}.png", cover_cache_key(audio_path)));
    if !cover_path.exists() {
        let image = image::load_from_memory(bytes)
            .ok()?
            .thumbnail(COVER_THUMBNAIL_SIZE, COVER_THUMBNAIL_SIZE);
        let mut encoded = Cursor::new(Vec::new());
        image.write_to(&mut encoded, ImageFormat::Png).ok()?;
        fs::write(&cover_path, encoded.into_inner()).ok()?;
    }
    Some(file_url(&cover_path))
}

fn cover_cache_key(path: &Path) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    if let Ok(metadata) = fs::metadata(path) {
        metadata.len().hash(&mut hasher);
        if let Ok(modified) = metadata.modified() {
            modified.hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn file_url(path: &Path) -> String {
    let text = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "%27");
    format!("file:///{text}")
}
