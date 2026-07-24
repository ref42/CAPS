# CAPS

**CAPS** means **`C`atch `A`ll `P`ossible `S`ources**.

<p align="center">
  <img src="src/assets/caps.svg" alt="caps" width="600px">
</p>



CAPS is a lightweight desktop music island built with Rust and Dioxus Desktop. It sits on screen like a compact capsule: quiet when idle, expressive when music is playing.

## What It Does

CAPS has two main states:

- **Idle**: shows local weather, upload speed, download speed, and a live audio spectrum.
- **Music**: searches NetEase Music, queues playable songs, plays audio, shows synced lyrics, spins album art, and renders a bouncy FFT spectrum.

The app is designed to stay small, fast, and out of the way. The current build is usually around a few MB of memory while idle.

## Features

- Search NetEase Music and add playable songs to the queue.
- Randomly load a chosen number of playable songs.
- Play, pause, stop, skip, and seek from the island.
- Display synced lyrics with smooth transitions.
- Pick spectrum colors from album art when available.
- Show local weather and network speed when no music is active.
- Right-click the island to clean song cache and exit CAPS.

## Tech Stack

- Rust
- Dioxus Desktop
- Rodio
- CPAL
- RustFFT
- Reqwest
- Sysinfo

## Author

ref42

## License

MIT License. See [LICENSE](LICENSE).
