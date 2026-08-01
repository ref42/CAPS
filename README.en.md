<p align="center">
  <img src="assets/brand.svg" alt="CAPS brand logo" width="600px" height = "100px">
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

**CAPS** means **`C`atch `A`ll `P`ossible `S`ources**, and it also comes from **capsule**. CAPS is built with Rust and Dioxus Desktop. While idle, it shows CPU, memory, and network speed. While music is active, it expands into a music island with cover art, lyrics, spectrum, progress, and compact playback controls. It can also split out a smaller companion island so Coco or Dodo can stay beside the main island.

<p align="center">
  <img src="assets/coco.gif" alt="Coco companion" width="96">
  &nbsp;&nbsp;&nbsp;&nbsp;
  <img src="assets/dodo.gif" alt="Dodo companion" width="96">
</p>

## For Users

- Hold Shift + left mouse button to drag the capsule to a position you like.
- Hover over the capsule to expand it and access search, queue, pet, and settings.
- Long-press the left mouse button on the capsule to split or merge the companion island beside the main island.
- Right-click the capsule to exit `CAPS`.
- Use the Pet page to switch between Coco and Dodo.
- Mode notes in Settings:
  - Normal mode: when no song is playing, CAPS shows CPU usage, memory usage, upload speed, and download speed. After you pick a song from the queue, it starts playback and shows lyrics for NetEase tracks plus the spectrum.
  - Silent mode: only shows CPU usage, memory usage, upload speed, and download speed.
  - Quiet mode: hides song title, lyrics, and related music details, and only shows CPU usage, memory usage, upload speed, and download speed. It becomes a desktop widget for low-profile use.

## For Developers

- Issues are welcome for bugs, feedback, and feature requests.
- For PRs, it is better to open an issue first and discuss the idea before implementation, so effort is not wasted.

## References

- [human-interface-guidelines/motion](https://developer.apple.com/design/human-interface-guidelines/motion)
- [widgetkit/dynamicisland](https://developer.apple.com/documentation/widgetkit/dynamicisland)
- [apple motion/ui ref](https://developer.apple.com/design/human-interface-guidelines/live-activities?pubDate=20250703&utm_source=openai)
- [mdn http requests](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Range_requests)
- [human-interface-guidelines/materials](https://developer.apple.com/design/human-interface-guidelines/materials)
- [liquid-glass](https://developer.apple.com/documentation/technologyoverviews/liquid-glass)
- [wwdc2025/219/](https://developer.apple.com/videos/play/wwdc2025/219/)
- [css-liquid-glass/](https://freefrontend.com/css-liquid-glass/)

## Thanks

- Thanks to all open-source project maintainers.

## License

MIT License. See [LICENSE](LICENSE).
