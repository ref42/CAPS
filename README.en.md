# CAPS

<p align="center">
  <img src="assets/brand.svg" alt="CAPS brand logo" width="600px">
</p>

<p align="center">
  <strong>A lightweight always-on-top desktop music island for search, import, playback, and system status.</strong>
</p>

<p align="center">
  <a href="README.md">中文</a>
</p>

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-2024-f74c00?logo=rust&logoColor=white">
  <img alt="Dioxus" src="https://img.shields.io/badge/Dioxus-Desktop-22a6f2?logo=dioxus&logoColor=white">
  <img alt="Windows" src="https://img.shields.io/badge/Windows-Desktop-0078d4?logo=windows11&logoColor=white">
  <img alt="Audio" src="https://img.shields.io/badge/Audio-Rodio%20%2B%20CPAL-ff69b4">
  <img alt="Spectrum" src="https://img.shields.io/badge/Spectrum-RustFFT-7df2ca">
  <img alt="License" src="https://img.shields.io/badge/License-MIT-white">
</p>

**CAPS** means **`C`atch `A`ll `P`ossible `S`ources**, and it also comes from **capsule**. It is not trying to be a full music platform. It is a quiet desktop control layer that turns different content sources into a listening queue.

CAPS is built with Rust and Dioxus Desktop. While idle, it shows CPU, memory, and network speed. While music is active, it expands into a music island with cover art, lyrics, spectrum, progress, and compact playback controls.

## What It Does

- Search NetEase music and add tracks to the queue.
- Load a random batch of online songs.
- Scan a local music folder, batch-add tracks, and avoid adding the same local file repeatedly.
- Extract listenable audio from Bilibili video URLs.
- Extract playable audio streams from YouTube video URLs.
- Show imported content duration, size, bitrate, codec, and download progress.
- Play, pause, stop, skip, and seek from the island.
- Show lyrics, spin cover art while playing, and derive spectrum/progress colors from cover art.
- Tune opacity, volume, island size, and playback mode in Settings.
- Clean downloaded audio files from disk in Settings.
- Right-click the island to clean cache and exit.

## Sources

| Source | How to use | Notes |
| --- | --- | --- |
| NetEase | Enter a song, artist, or album keyword | Search results can be queued directly; random loading is also supported. |
| Bilibili | Paste a Bilibili video URL | Resolves video metadata and audio, then downloads and caches it for playback. |
| YouTube | Paste a YouTube video URL | Resolves video metadata and chooses a playable MP4/M4A-compatible stream when available. |
| Local | Enter a local music folder path | Scans common audio formats such as mp3, flac, ogg, wav, and m4a, including metadata, cover art, and duration. |

Bilibili and YouTube tabs validate their own source. A YouTube URL will not import from the Bilibili tab, and a Bilibili URL will not import from the YouTube tab.

## Downloads And Cache

Audio from video sources is downloaded into CAPS' song cache and reused on later playback. Large files first try concurrent HTTP range downloads. If a CDN does not support ranges, CAPS falls back to a normal serial download.

Use **Clean cache** in Settings to remove downloaded audio from disk. CAPS stops playback before cleaning so Windows can release any open cache file handle.

## Tech Stack

| Tech | Role |
| --- | --- |
| Rust 2024 | Application core, audio, downloads, state |
| Dioxus Desktop | Desktop UI |
| Rodio + CPAL | Audio playback and device access |
| RustFFT | Real-time spectrum |
| Reqwest + Tokio | Online requests and async downloads |
| Symphonia | Local music metadata, cover art, and duration |
| Sysinfo | CPU, memory, and network stats |

## Author

ref42

## License

MIT License. See [LICENSE](LICENSE).
