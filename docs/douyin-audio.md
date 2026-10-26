# 抖音音轨导入 / Douyin audio import

实验分支：`feature/unified-media-streaming`（包含先前的抖音导入工作）。

在搜索页选择「媒体」，粘贴视频链接或整段分享文字后点「导入」，自动识别来源。成功后只向队列
追加一条视频音轨，按抖音来源和视频 ID 去重；不会打断当前播放。点击队列中的歌曲
后先下载解码所需的数据，短暂缓冲后即可边下载边播放。拖动进度条会优先下载目标
位置；音频解码和网络等待不会阻塞音频输出线程。网络较慢时显示缓冲状态，播放时间
不会随静音等待推进。完整下载并通过初始解码校验后才发布缓存；停止或切歌会取消
未完成的下载并清理临时文件。不支持分段请求的服务器按顺序缓冲下载，跳转可能需要等待。缓存仍是视频
容器，只播放其中的音频；不会拆分合集，也不会用可能截短的 `music.play_url` 替代原声。

只使用 Rust HTTP 客户端读取公开分享页和页面内的路由数据，不启动 Edge 或 Chrome，
不读取浏览器 Cookie 或登录状态，也不使用第三方解析服务。页面数据缺失、需要 JavaScript
或遇到验证、登录、仅限 App 的内容时报告失败，不承诺所有链接可用。

当前验证（2026-10-26）：

- 分享链接：`https://v.douyin.com/gYsL-kehIgg/`
- 视频 ID：`7676446085899963684`，作者：音乐法师。
- 标题：【中文 DJ 舞曲 100 首】电音舞力全开！全程高燃劲爆热门 DJ，车载蹦迪必备歌单。
- 页面标记时长 2,336,234 毫秒，约 38 分 56 秒；缓存下载 342,351,571 字节（约 326 MiB）。
- 当前纯 HTTP 请求返回的页面只有路由配置，没有视频元数据；详情接口返回 403，旧版 iteminfo 接口返回空响应。因此此样例当前不能重新解析、导入或在无缓存时播放。
- 标题、作者、时长和文件大小来自删除浏览器回退前的历史验证。先前的边下边播成功记录使用了浏览器解析地址，不代表当前纯 HTTP 解析成功。
- 本地限速 HTTP 测试覆盖提前播放、未下载位置跳转、缓冲不推进时间、完整缓存字节校验、错误分段/截断响应、网络故障与取消清理。
- 已下载完成的缓存仍可直接播放；未完成的下载不作为缓存使用。

## English

Choose **Media**, paste a Bilibili, YouTube, or Douyin link (or complete share message), and select **Import**; CAPS detects the provider automatically.
The video becomes one queue entry, deduplicated by source and video ID. Import does
not interrupt playback. Audio starts once the initial data can be decoded, while
the video continues downloading. Seeking prioritizes the requested section. Network
waits and decoding happen away from the output thread; buffering does not advance
the media clock. Only complete downloads with a successful initial decode become
cache files. Stopping or changing tracks cancels unfinished streams and removes
their temporary files. Servers without byte ranges download progressively in order;
seeking may wait for the requested bytes. Media indexes at the end can delay startup.
The cache contains the video container; CAPS plays only its audio. Compilations
remain one track.

The sample link currently returns route configuration without video metadata over
HTTP; the detail API returns 403 and the older iteminfo API returns an empty body.
It cannot currently be imported or played without a completed cache. Earlier
successful streaming measurements used the removed browser fallback to resolve the
URL and do not validate the current HTTP-only resolver. The title, artist, duration,
and file size above are historical measurements. Completed caches still play directly.

The resolver uses direct Rust HTTP requests to read public share-page metadata and
embedded route data. It never launches Edge or Chrome, reads browser cookies, or uses
a third-party extraction service. Pages that require JavaScript, login, verification,
the app, or a site-specific token can fail. Site changes can also prevent import.

```powershell
# Probe direct HTTP resolution (the sample currently reports missing metadata).
cargo run --locked --example douyin_audio -- https://v.douyin.com/gYsL-kehIgg/

# Verify buffered playback and seeking through the default audio device (muted).
cargo run --locked --example douyin_audio -- https://v.douyin.com/gYsL-kehIgg/ --stream stream-test.mp4

# Download into a new file and verify audio decoding (about 326 MiB for this link).
cargo run --locked --example douyin_audio -- https://v.douyin.com/gYsL-kehIgg/ mix.mp4
```
