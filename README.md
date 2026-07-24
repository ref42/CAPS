# CAPS

<p align="center">
  <img src="assets/brand.svg" alt="CAPS brand logo" width="600px">
</p>

<p align="center">
  <strong>一个轻量、常驻、像胶囊一样的 Windows 桌面音乐岛。</strong>
</p>

<p align="center">
  <a href="README.en.md">English</a>
</p>

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-2024-f74c00?logo=rust&logoColor=white">
  <img alt="Dioxus" src="https://img.shields.io/badge/Dioxus-Desktop-22a6f2?logo=dioxus&logoColor=white">
  <img alt="Windows" src="https://img.shields.io/badge/Windows-Desktop-0078d4?logo=windows11&logoColor=white">
  <img alt="Audio" src="https://img.shields.io/badge/Audio-Rodio%20%2B%20CPAL-ff69b4">
  <img alt="FFT" src="https://img.shields.io/badge/Spectrum-RustFFT-7df2ca">
  <img alt="License" src="https://img.shields.io/badge/License-MIT-white">
</p>

**CAPS** 是 **`C`atch `A`ll `P`ossible `S`ources** 的缩写。

CAPS 用 Rust 和 Dioxus Desktop 构建。它停留在屏幕顶部，默认只显示必要的系统状态；当音乐播放时，它会变成一个动态音乐岛，展示封面、歌词、进度、频谱和基础播放控制。

## 状态

- **空闲**：显示 CPU、RAM、上传速度、下载速度和实时音频频谱。
- **音乐**：显示专辑封面、同步歌词、播放进度、播放控制和更有弹性的 FFT 频谱。

CAPS 的目标不是成为完整音乐软件，而是做一个轻、小、好看的桌面音乐控制层。

## 功能

- 搜索网易云音乐，并把可播放歌曲加入队列。
- 随机加载指定数量的可播放歌曲。
- 加载本地音乐文件夹，并批量加入播放队列。
- 播放、暂停、停止、上一首、下一首和拖动进度。
- 显示同步歌词，并带有平滑过渡效果。
- 播放时旋转专辑封面。
- 根据专辑封面提取频谱和进度条颜色。
- 空闲时显示 CPU、RAM 和网络速度。
- 右键音乐岛即可清理歌曲缓存并退出 CAPS。

## 技术栈

| 技术 | 用途 |
| --- | --- |
| ![Rust](https://img.shields.io/badge/Rust-2024-f74c00?logo=rust&logoColor=white) | 主程序、音频、状态管理 |
| ![Dioxus](https://img.shields.io/badge/Dioxus-Desktop-22a6f2?logo=dioxus&logoColor=white) | 桌面 UI |
| ![Rodio](https://img.shields.io/badge/Rodio-Playback-ff69b4) | 音频播放 |
| ![CPAL](https://img.shields.io/badge/CPAL-Audio%20Device-8e8e93) | 音频设备与采样 |
| ![RustFFT](https://img.shields.io/badge/RustFFT-Spectrum-7df2ca) | 频谱分析 |
| ![Reqwest](https://img.shields.io/badge/Reqwest-HTTP-34c759) | 网易云音乐请求 |
| ![Sysinfo](https://img.shields.io/badge/Sysinfo-System%20Stats-0078d4) | CPU、RAM、网络状态 |
| ![Symphonia](https://img.shields.io/badge/Symphonia-Metadata-f5c542) | 本地音乐元数据和封面 |

## 项目结构

```text
.
├── assets/              # README 和项目品牌资产
├── src-tauri/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   └── src/
│       ├── main.rs          # Dioxus Desktop 入口和应用状态
│       ├── components.rs    # Island、搜索、队列、设置等 UI
│       ├── app.css          # 视觉样式和动画
│       ├── audio.rs         # 音频播放线程
│       ├── audio_spectrum.rs
│       ├── netease.rs       # 网易云音乐接口
│       ├── local_music.rs   # 本地音乐扫描和元数据
│       ├── lyrics.rs
│       ├── storage.rs
│       └── windowing.rs
├── README.md
├── README.en.md
└── LICENSE
```

## 作者

ref42

## 许可证

MIT License. See [LICENSE](LICENSE).
