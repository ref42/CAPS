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

**CAPS** 取自 **`C`atch `A`ll `P`ossible `S`ources**，或 **capsule**。CAPS 使用 Rust 和 Dioxus Desktop 构建。空闲时它显示 CPU、内存和网络速度；播放时它展开成音乐岛，显示封面、歌词、频谱、进度条和基础播放控制。它也可以分离出一个更小的宠物岛，让 Coco 或 Dodo 常驻在旁边。

CAPS 支持以下音频来源：

- **Online**：搜索网易云音乐、QQ 音乐和酷狗音乐。只展示存在可用播放地址的歌曲，QQ 音乐还会排除仅会员可播放的歌曲。
- **Bilibili / YouTube**：粘贴视频链接，提取可用的音频内容。
- **Local**：选择本地音频文件夹并批量加入播放列表。
- 在线来源提供歌词（网易云音乐、QQ 音乐和酷狗音乐在有对应歌词时显示），本地歌曲也会读取同目录中的歌词文件。
- 搜索结果会显示来源后缀，用于区分不同平台的同名歌曲；加入播放列表后仍显示歌曲原名。
- 随机添加歌曲时，Online 会尽量按网易云音乐与 QQ 音乐各一半组成歌单，并保持用户指定的总数量。

<p align="center">
  <img src="assets/coco.gif" alt="Coco companion" width="96">
  &nbsp;&nbsp;&nbsp;&nbsp;
  <img src="assets/dodo.gif" alt="Dodo companion" width="96">
</p>

## 写给普通用户

- Shift+鼠标左键，按住拖动胶囊，可以将胶囊放在你喜欢的位置。
- 鼠标悬停，可以展开胶囊，进行搜索、播放队列、宠物和设置操作。
- 在胶囊上长按鼠标左键，可以分离或合并主岛旁的宠物岛。
- 鼠标在胶囊上右键可以退出`CAPS`。
- Pet 页面可以切换 Coco 和 Dodo。
- 设置里的模式选择说明：
  - 普通模式，如果未播放任何歌曲，则显示CPU占用，内存占用，上行速度，下行速度；点击播放列表选择歌曲后会立即开始播放，并显示可用歌词和频谱。
  - 静默模式，只显示显示CPU占用，内存占用，上行速度，下行速度。
  - 安静模式，隐藏歌曲的名称和歌词等信息，只显示CPU占用，内存占用，上行速度，下行速度。此时退化为桌面挂件（推荐摸鱼使用）。

## 写给开发者

- 欢迎给我提issue，反馈bug或者添加新功能。
- 若是想要进行pr，最好是先在issue里提出，讨论之后再开始实现，避免浪费精力。

## 参考
- [human-interface-guidelines/motion](https://developer.apple.com/design/human-interface-guidelines/motion)
- [widgetkit/dynamicisland](https://developer.apple.com/documentation/widgetkit/dynamicisland)
- [apple motion/ui ref](https://developer.apple.com/design/human-interface-guidelines/live-activities?pubDate=20250703&utm_source=openai)
- [mdn http requests](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Range_requests)
- [human-interface-guidelines/materials](https://developer.apple.com/design/human-interface-guidelines/materials)
- [liquid-glass](https://developer.apple.com/documentation/technologyoverviews/liquid-glass)
- [wwdc2025/219/](https://developer.apple.com/videos/play/wwdc2025/219/)
- [css-liquid-glass/](https://freefrontend.com/css-liquid-glass/)

## 致谢
- 所有开源项目维护者。

## 许可证

MIT License. See [LICENSE](LICENSE).
