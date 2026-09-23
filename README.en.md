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
  <img alt="GPUI Kit" src="https://img.shields.io/badge/GPUI_Kit-Native-7df2ca">
  <img alt="Windows" src="https://img.shields.io/badge/Windows-Desktop-0078d4?logo=windows11&logoColor=white">
  <img alt="Audio" src="https://img.shields.io/badge/Audio-Rodio%20%2B%20CPAL-ff69b4">
  <img alt="Spectrum" src="https://img.shields.io/badge/Spectrum-RustFFT-7df2ca">
  <img alt="License" src="https://img.shields.io/badge/License-MIT-white">
</p>

**CAPS** means **`C`atch `A`ll `P`ossible `S`ources**, and it also comes from **capsule**. CAPS is built with Rust and GPUI Kit. While idle, it shows CPU, memory, and network speed. While music is active, it expands into a music island with cover art, lyrics, spectrum, progress, and compact playback controls.

CAPS supports these audio sources:

- **Online**: search NetEase Cloud Music, QQ Music, and KuGou.
- **Bilibili / YouTube**: paste a video link to extract available audio content.
- **Local**: choose a local audio folder and add its tracks to the queue in batches.
- Lyrics are shown when available for NetEase, QQ Music, and KuGou tracks; local tracks can also load lyric files from the same folder.
- Search results show a provider suffix to distinguish identical songs from different services; queued tracks keep their original names.
- Random Online queues draw from NetEase, QQ Music, and KuGou in a round-robin split while preserving the total number requested.

## For Users

- Hold Shift + left mouse button to drag the capsule to a position you like. CAPS remembers where you left it and opens there next time.
- Hover over the capsule to expand it and access search, queue, and settings. It collapses automatically after the pointer leaves the visible island.
- Right-click the capsule to exit `CAPS`. Right-clicking inside the panel does not quit.
- Select a row in the queue to play that track; the `▶` and `×` at the end of the row work too.
- Each queue row shows the artist and album, with the track length in its own right-aligned column, so a long album name can no longer push the length out of the row. Rest the pointer on a row and either of its text lines scrolls if it does not fit, then stops when it has shown the end; the capsule's title and artist do the same. The transport controls are grouped tightly so the title and artist have as much width as possible.
- The queue has its own scrollbar down the right-hand side, so a long queue can be dragged through rather than only wheeled.
- The button beside **Clear queue** sets the play order: in order, repeat all, repeat one, or shuffle. It cycles through the four, names the one in force, and remembers the choice; it takes the accent tint while an order other than the default is on. Adding tracks always appends them to the end of the queue and leaves the list there, where they arrived.
- Mode notes in Settings:
  - Normal mode: when no song is playing, CAPS shows CPU usage, memory usage, upload speed, and download speed. Picking a song from the queue starts playback immediately and shows available lyrics plus the spectrum.
  - Silent mode: only shows CPU usage, memory usage, upload speed, and download speed.
  - Quiet mode: hides song title, lyrics, and related music details, and only shows CPU usage, memory usage, upload speed, and download speed. It becomes a desktop widget for low-profile use.
- Opacity stops at 55%: below that the island's own text drops under the contrast floor, so the slider no longer goes there.
- **Fonts** in Settings sets the English and the Chinese typeface separately. Each row shows the face in use; opening it lists **every font installed on the machine** (the likely choices first) with a search box — typing filters as you go, Enter takes the first match, and the Chinese names people actually type work too ("雅黑" finds Microsoft YaHei). **Every entry is previewed in its own face**, so the choice is made by looking at it. The default is *System*: Segoe UI for Latin with Microsoft YaHei UI for Han, which is the pairing Windows itself uses, so weights and baselines line up on mixed lines.
- The interface uses the system UI font (Segoe UI on Windows) and follows the system's reduced-motion setting: with it on, expanding, collapsing, and the spinning cover switch instantly instead of animating.
## For Developers

- Issues are welcome for bugs, feedback, and feature requests.
- For PRs, it is better to open an issue first and discuss the idea before implementation, so effort is not wasted.
- The font picker lists every family the system reports. The list is virtualized and searchable, and no family is ever hidden; families that suit the slot are simply listed first.

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
