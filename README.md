<p align="center">
  <img src="assets/brand.svg" alt="CAPS brand logo" width="600px" height = "100px">
</p>

<p align="center">
  <strong>一个常驻桌面顶部的轻量音乐岛：搜索、导入、播放和观察系统状态。</strong>
</p>

<p align="center">
  <a href="README.en.md">English</a>
</p>

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-2024-f74c00?logo=rust&logoColor=white">
  <img alt="Dioxus" src="https://img.shields.io/badge/Dioxus-Desktop-22a6f2?logo=dioxus&logoColor=white">
  <img alt="Windows" src="https://img.shields.io/badge/Windows-Desktop-0078d4?logo=windows11&logoColor=white">
  <img alt="Audio" src="https://img.shields.io/badge/Audio-Rodio%20%2B%20CPAL-ff69b4">
  <img alt="Spectrum" src="https://img.shields.io/badge/Spectrum-RustFFT-7df2ca">
  <img alt="License" src="https://img.shields.io/badge/License-MIT-white">
</p>

**CAPS** 是 **`C`atch `A`ll `P`ossible `S`ources**，也来自 **capsule**。它不是一个完整音乐平台，而是一个安静、常驻、可以快速把不同来源内容变成可听队列的桌面控制层。

CAPS 使用 Rust 和 Dioxus Desktop 构建。空闲时它显示 CPU、内存和网络速度；播放时它展开成音乐岛，显示封面、歌词、频谱、进度条和基础播放控制。

## 写给普通用户

- Shift+鼠标左键，按住拖动胶囊，可以将胶囊放在你喜欢的位置。
- 鼠标悬停，可以展开胶囊，进行设置。
- 鼠标在胶囊上右键可以退出`CAPS`。
- 设置里的模式选择说明：
  - 普通模式，如果未播放任何歌曲，则显示CPU占用，内存占用，上行速度，下行速度；点击播放列表选择了歌曲之后则会开始播放歌曲，并且显示歌词（某易歌曲），频谱。
  - 静默模式，只显示显示CPU占用，内存占用，上行速度，下行速度。
  - 安静模式，隐藏歌曲的名称和歌词等信息，只显示CPU占用，内存占用，上行速度，下行速度。此时退化为桌面挂件（推荐摸鱼使用）。

## 写给开发者

- 欢迎给我提issue，反馈bug或者添加新功能。
- 若是想要进行pr，最好是先在issue里提出，讨论之后再开始实现，避免浪费精力。
- 每个人都有自己的想法，但是不管如何拓展，都不要让CAPS吃掉超过30MB的内存，现在的正常占用在5~25MB左右。

## 参考
- [apple animation design](https://developer.apple.com/design/human-interface-guidelines/motion)
- [apple dynamic island](https://developer.apple.com/documentation/widgetkit/dynamicisland)
- [apple motion/ui ref](https://developer.apple.com/design/human-interface-guidelines/live-activities?pubDate=20250703&utm_source=openai)
- [mdn http requests](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Range_requests)

## 致谢
- 所有开源项目维护者。

## 打赏

- 如果你觉得这个项目还不错，可以考虑给我打赏。感谢~
- 推荐添加打赏备注，如果CAPS让您体验很糟糕，可以进行退款。
- 感谢支持。
<div align="center">
微信/支付宝<br>
<img src="qr/pay.jpg" width="200" alt="收款码">
</div>

## 许可证

MIT License. See [LICENSE](LICENSE).
