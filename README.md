# CAPS

CAPS is a Rust desktop music island built with Dioxus Desktop.

It searches NetEase Music, adds tracks to a local queue, plays public stream URLs directly, shows compact playback state, displays synced lyrics, renders an FFT spectrum, displays network speed when idle, and includes random queue loading.

CAPS runs as a small always-on-top capsule instead of a normal music app window. Hover the island to open search, queue, stats, and settings. Right-click the island to exit.

Logo: `src/assets/caps.logo`

Author: ref42

## Run

```powershell
cargo run --manifest-path src-tauri\Cargo.toml
```

## Check

```powershell
cargo check --manifest-path src-tauri\Cargo.toml
```

## Build

```powershell
cargo build --manifest-path src-tauri\Cargo.toml --release
```

## Tech

- Rust
- Dioxus Desktop
- Rodio
- CPAL
- Reqwest
- Sysinfo

## License

MIT License

Copyright (c) 2026 ref42
