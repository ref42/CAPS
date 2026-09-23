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
  <img alt="GPUI Kit" src="https://img.shields.io/badge/GPUI_Kit-Native-7df2ca">
  <img alt="Windows" src="https://img.shields.io/badge/Windows-Desktop-0078d4?logo=windows11&logoColor=white">
  <img alt="Audio" src="https://img.shields.io/badge/Audio-Rodio%20%2B%20CPAL-ff69b4">
  <img alt="Spectrum" src="https://img.shields.io/badge/Spectrum-RustFFT-7df2ca">
  <img alt="License" src="https://img.shields.io/badge/License-MIT-white">
</p>

**CAPS** 取自 **`C`atch `A`ll `P`ossible `S`ources**，或 **capsule**。CAPS 使用 Rust 和 GPUI Kit 构建。空闲时它显示 CPU、内存和网络速度；播放时它展开成音乐岛，显示封面、歌词、频谱、进度条和基础播放控制。

CAPS 支持以下音频来源：

- **Online**：搜索网易云音乐、QQ 音乐和酷狗音乐。
- **Bilibili / YouTube**：粘贴视频链接，提取可用的音频内容。
- **Local**：选择本地音频文件夹并批量加入播放列表。
- 在线来源提供歌词（网易云音乐、QQ 音乐和酷狗音乐在有对应歌词时显示），本地歌曲也会读取同目录中的歌词文件。
- 随机添加歌曲时，Online 会从网易云音乐、QQ 音乐和酷狗音乐三个来源轮流加入，并保持用户指定的总数量。
- 搜索结果会显示来源后缀，用于区分不同平台的同名歌曲；加入播放列表后仍显示歌曲原名。
- 随机添加歌曲时，Online 会尽量按网易云音乐与 QQ 音乐各一半组成歌单，并保持用户指定的总数量。

## 写给普通用户

- Shift+鼠标左键，按住拖动胶囊，可以将胶囊放在你喜欢的位置；位置会被记住，下次启动仍停在那里。
- 鼠标悬停，可以展开胶囊，进行搜索、播放队列和设置操作；鼠标移出可见岛屿后会自动收起。
- 鼠标在胶囊上右键可以退出`CAPS`（面板内部右键不会退出）。
- 点击队列中的整行即可播放该歌曲，行尾的 ▶ 与 × 也可以使用。
- 队列每行显示歌手与专辑，时长单独占一列并右对齐，专辑名再长也不会把时长挤出这一行。鼠标停在某一行时，行里放不下的那行文字会自动横向滚动，滚到末尾后停住；胶囊上的歌名和歌手也一样。胶囊上的播放控制排得更紧凑，把宽度尽量留给歌名和歌手。
- 队列右侧有独立的滚动条，歌多的时候可以直接拖动，不用一直滚滚轮。
- 「清空队列」旁边的按钮用来切换播放顺序：顺序播放、列表循环、单曲循环、随机播放。点一下换一种，按钮上写着当前顺序，选择会被记住；不是默认顺序时会用强调色标出来。添加歌曲始终追加到队列末尾，列表也会停在新歌出现的位置。
- 设置里的模式选择说明：
  - 普通模式，如果未播放任何歌曲，则显示CPU占用，内存占用，上行速度，下行速度；点击播放列表选择歌曲后会立即开始播放，并显示可用歌词和频谱。
  - 静默模式，只显示显示CPU占用，内存占用，上行速度，下行速度。
  - 安静模式，隐藏歌曲的名称和歌词等信息，只显示CPU占用，内存占用，上行速度，下行速度。此时退化为桌面挂件（推荐摸鱼使用）。
- 透明度最低可调到 55%：再低时胶囊上的文字会看不清，所以不再往下开放。
- 设置里的「字体」可以分别指定英文和中文字体：每一行都会显示当前使用的字体，点进去会列出**本机安装的全部字体**（常见的排在前面），并带一个搜索框——输入即筛选，回车选中第一条，「雅黑」「宋体」这样的中文名也能搜到。**每个候选都用它自己的字体预览**，选哪个一眼就能看出来。默认是「系统」——英文用系统的 Segoe UI，中文用它的中文搭配字体（Microsoft YaHei UI），两者字重和基线是对齐的。
- 界面文字跟随系统字体（Windows 上为 Segoe UI），并遵循系统的“减少动画”设置：开启后展开、收起和封面旋转会直接切换，不再有过渡动画。

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
