# CAPS

<p align="center">
  <img src="assets/brand.svg" alt="CAPS brand logo" width="600px">
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

## 现在可以做什么

- 从 NetEase 搜索音乐并加入队列。
- 随机加载一批在线歌曲。
- 扫描本地音乐文件夹，批量加入队列，并避免重复加入同一首本地歌。
- 从 Bilibili 视频链接提取可听音频。
- 从 YouTube 视频链接提取可播放音频流。
- 显示导入内容的时长、大小、码率、编码和下载进度。
- 播放、暂停、停止、上一首、下一首和拖动进度条。
- 显示歌词，播放时旋转封面，并根据封面提取频谱/进度条颜色。
- 在 Settings 中调节透明度、音量、岛尺寸和播放模式。
- 在 Settings 中清理下载到磁盘的音频缓存。
- 右键音乐岛清理缓存并退出。

## 来源

| 来源 | 用法 | 说明 |
| --- | --- | --- |
| NetEase | 输入歌曲、歌手或专辑关键词 | 搜索结果可直接加入队列；支持随机加载。 |
| Bilibili | 粘贴 Bilibili 视频 URL | 解析视频元数据和 DASH 音频，下载后缓存播放。 |
| YouTube | 粘贴 YouTube 视频 URL | 使用纯 Rust 的窄路径解析 YouTube Innertube 响应，优先选择可播放的 MP4/M4A 兼容流。 |
| Local | 填入本地音乐文件夹路径 | 扫描 mp3、flac、ogg、wav、m4a 等常见音频文件，并读取元数据、封面和时长。 |

Bilibili 和 YouTube 标签页会严格校验对应来源。YouTube 链接不会在 Bilibili 标签页导入，反过来也一样。

## 下载和缓存

视频来源的音频会下载到 CAPS 的歌曲缓存目录中，后续播放会直接复用缓存。较大的文件会先尝试并发 HTTP range 下载；如果 CDN 不支持 range，CAPS 会回退到普通串行下载。

缓存可以通过 Settings 里的 **Clean cache** 清理。清理时 CAPS 会先停止当前播放，避免 Windows 因文件句柄未释放而删除失败。

## 技术栈

| 技术 | 用途 |
| --- | --- |
| Rust 2024 | 主程序、音频、下载、状态管理 |
| Dioxus Desktop | 桌面 UI |
| Rodio + CPAL | 音频播放和设备 |
| RustFFT | 实时频谱 |
| Reqwest + Tokio | 在线请求和异步下载 |
| Symphonia | 本地音乐元数据、封面和时长 |
| Sysinfo | CPU、内存和网络状态 |

## 作者

ref42

## 许可证

MIT License. See [LICENSE](LICENSE).
