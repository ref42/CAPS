# CAPS

**CAPS** 是 **`C`atch `A`ll `P`ossible `S`ources** 的缩写。

[English](README.en.md)

<p align="center">
  <img src="assets/caps.svg" alt="CAPS logo" width="600px">
</p>

CAPS 是一个用 Rust 和 Dioxus Desktop 构建的轻量桌面音乐岛。它像胶囊一样停留在屏幕上：空闲时安静显示系统状态，播放音乐时展示歌词、封面和频谱动画。

## 它能做什么

CAPS 主要有两种状态：

- **空闲**：显示 CPU、RAM、上传速度、下载速度和实时音频频谱。
- **音乐**：搜索网易云音乐、加入可播放歌曲队列、播放音频、显示同步歌词、旋转专辑封面，并渲染更有弹性的 FFT 频谱。

它的目标是保持小、快、不打扰：常驻桌面，但不占用你的主要工作空间。

## 功能

- 搜索网易云音乐，并把可播放歌曲加入队列。
- 随机加载指定数量的可播放歌曲。
- 支持从音乐岛直接播放、暂停、停止、上一首、下一首和拖动进度。
- 显示同步歌词，并带有平滑过渡效果。
- 根据专辑封面提取频谱和进度条颜色。
- 空闲时显示 CPU、RAM 和网络速度。
- 支持加载本地音乐文件夹并加入播放队列。
- 支持 iPhone Fun Mode：在 Safari 打开 Settings 里的局域网地址后，可以用摇晃和倾斜控制播放。
- 右键音乐岛即可清理歌曲缓存并退出 CAPS。

## 技术栈

- Rust
- Dioxus Desktop
- Rodio
- CPAL
- RustFFT
- Reqwest
- Sysinfo

## 作者

ref42

## 许可证

MIT License. See [LICENSE](LICENSE).
