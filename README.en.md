# CAPS

<p align="center">
  <img src="assets/brand.svg" alt="CAPS brand logo" width="600px">
</p>

<p align="center">
  <strong>A lightweight, always-on-top Windows music island shaped like a capsule.</strong>
</p>

<p align="center">
  <a href="README.md">中文</a>
</p>

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-2024-f74c00?logo=rust&logoColor=white">
  <img alt="Dioxus" src="https://avatars.githubusercontent.com/u/79236386?s=20" width="20" height="20">
  <img alt="Dioxus" src="https://img.shields.io/badge/Dioxus-Desktop-22a6f2?logo=dioxus&logoColor=white">
  <img alt="Windows" src="https://img.shields.io/badge/Windows-Desktop-0078d4?logo=windows11&logoColor=white">
  <img alt="RustAudio" src="https://avatars.githubusercontent.com/u/9999738?s=20&v=4" width="20" height="20">
  <img alt="Audio" src="https://img.shields.io/badge/Audio-Rodio%20%2B%20CPAL-ff69b4">
  <img alt="FFT" src="https://img.shields.io/badge/Spectrum-RustFFT-7df2ca">
  <img alt="License" src="https://img.shields.io/badge/License-MIT-white">
</p>

**CAPS** means **`C`atch `A`ll `P`ossible `S`ources**, and it also comes from **capsule**: a small pill to heal, carrying music, status, and controls in one quiet island.

CAPS is built with Rust and Dioxus Desktop. It lives at the top of the screen, stays quiet while idle, and turns into a dynamic music island while playing: album art, lyrics, progress, spectrum, and compact playback controls.

## States

- **Idle**: shows CPU usage, RAM usage, upload speed, download speed, and a live audio spectrum.
- **Music**: shows album art, synced lyrics, playback progress, playback controls, and a bouncy FFT spectrum.

CAPS is not trying to be a full music client. It is a small, polished desktop control layer for music.

## Features

- Search online music and add playable songs to the queue.
- Randomly load a chosen number of playable songs.
- Load a local music folder and batch-add tracks to the queue.
- Play, pause, stop, skip, and seek from the island.
- Display synced lyrics with smooth transitions.
- Spin album art while music is playing.
- Pick spectrum and progress colors from album art when available.
- Show CPU, RAM, and network speed when no music is active.
- Right-click the island to clean song cache and exit CAPS.

## Tech Stack

| Tech | Role |
| --- | --- |
| ![Rust](https://img.shields.io/badge/Rust-2024-f74c00?logo=rust&logoColor=white) | Application core, audio, state |
| <img alt="Dioxus" src="https://avatars.githubusercontent.com/u/79236386?s=18" width="18" height="18"> ![Dioxus](https://img.shields.io/badge/Dioxus-Desktop-22a6f2?logo=dioxus&logoColor=white) | Desktop UI |
| <img alt="Rodio" src="https://avatars.githubusercontent.com/u/9999738?s=18&v=4" width="18" height="18"> ![Rodio](https://img.shields.io/badge/Rodio-Playback-ff69b4) | Audio playback |
| <img alt="RustAudio" src="https://avatars.githubusercontent.com/u/9999738?s=18&v=4" width="18" height="18"> ![CPAL](https://img.shields.io/badge/CPAL-Audio%20Device-8e8e93) | Audio device and sampling |
| ![RustFFT](https://img.shields.io/badge/RustFFT-Spectrum-7df2ca) | Spectrum analysis |
| ![Reqwest](https://img.shields.io/badge/Reqwest-HTTP-34c759) | Online music requests |
| ![Sysinfo](https://img.shields.io/badge/Sysinfo-System%20Stats-0078d4) | CPU, RAM, and network stats |
| ![Symphonia](https://img.shields.io/badge/Symphonia-Metadata-f5c542) | Local music metadata and cover art |

## Project Layout

```text
.
├── assets/              # README and project brand assets
├── src-tauri/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   └── src/
│       ├── main.rs          # Dioxus Desktop entry and app state
│       ├── components.rs    # Island, search, queue, settings UI
│       ├── app.css          # Visual design and animation
│       ├── audio.rs         # Audio playback thread
│       ├── audio_spectrum.rs
│       ├── local_music.rs   # Local music scan and metadata
│       ├── lyrics.rs
│       ├── storage.rs
│       └── windowing.rs
├── README.md
├── README.en.md
└── LICENSE
```

## Author

ref42

## License

MIT License. See [LICENSE](LICENSE).
