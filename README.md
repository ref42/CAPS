# QiuNiu

QiuNiu is a Rust desktop music island built with Dioxus Desktop.

The app searches NetEase Music, adds tracks to a local queue, plays public stream URLs directly, shows compact playback state, displays network speed, and includes random queue loading for 50 or 100 tracks.

Logo: `src/assets/qiuniu.logo`

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
