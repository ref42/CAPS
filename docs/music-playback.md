# 音乐搜索与播放 / Music search and playback

搜索页仅保留 **在线 / 本地** 两个入口。已移除媒体链接导入、视频解析器及登录会话设置。Bilibili 直播弹幕和点歌功能独立保留。旧队列 JSON 仍可读取，但不再解析旧视频条目的远程音轨。

## 搜索

输入 `歌曲 [歌手] [平台]`。歌手、平台均可省略，例如：

- `晴天`
- `晴天 周杰伦`
- `晴天 qq`
- `晴天 周杰伦 qq`
- `The Sound of Silence Simon & Garfunkel netease`

支持的末尾平台名：`netease` / `neteast` / `网易云`、`kugou` / `酷狗`、`qq` / `qqmusic` / `QQ音乐`、`qishui` / `汽水音乐`。也可使用显式后缀 `platform:qq` 或 `@qq`。只识别完整的末尾词，不匹配歌名内部的子串；单独搜索 `qq` 仍是普通关键词。末尾的带引号词保持字面含义。

平台词不会发给搜索服务。歌曲和歌手作为原查询交给四个来源，指定平台的返回结果整体优先，保留每个平台原本的相关度顺序，其他平台的结果全部保留。输入框下方会提示当前优先平台。

文本选择使用实心蓝色背景，焦点使用细绿色边框，光标与焦点颜色一致，适配深浅两种外观。保留 Ctrl+A、Shift+方向键、鼠标拖选和中文输入的原生行为。

## 播放

网易云、QQ 音乐、酷狗和汽水音乐共用边下载边播放流程，包括队列点击、上一首/下一首、循环、自动续播及空闲时的弹幕点歌。已完成缓存和本地文件直接播放。停止或切歌会取消未完成的下载，只有下载完整且初始解码成功的文件才成为缓存。

服务器支持 HTTP Range 时优先请求跳转位置，否则顺序下载；向前跳转可能等待。缓冲静音不会推进歌曲时间。文件末尾的索引和慢速网络可能增加首次等待时间。

汽水只接入公开可用完整音轨，拒绝试听和加密音轨，不读取浏览器配置或登录会话。来源可用性取决于服务端。

## 验证

- `晴天 周杰伦 qq` 在线实测返回 QQ 音乐 11 条、网易云 55 条、酷狗 99 条、汽水 77 条；QQ 音乐全部排在其他来源之前。
- 自动测试覆盖平台别名、可选歌手、完整末尾词、稳定排序与保留全部结果，以及流式下载、跳转、缓冲、缓存完整性、取消和失败处理。
- 独立静音窗口检查了输入选区和焦点边框。

```powershell
cargo test --locked --offline --bin caps --example music_playback
cargo clippy --locked --offline --bin caps --example music_playback -- -D warnings
cargo run --locked --example music_playback -- qishui 晴天
```

## English

The search page has **Online / Local** only. Video-link import, its resolvers, and session settings have been removed. Bilibili live chat and song requests remain separate features. Old queue files stay readable, but remote video tracks are no longer resolved.

Use `song [artist] [platform]`. The trailing platform (`netease`, `neteast`, `kugou`, `qq`, `qqmusic`, `qishui`, or its Chinese alias) is a ranking preference, not a filter. `platform:qq` and `@qq` also work. All four services receive the remaining song/artist query. The preferred provider appears first; all other results and each provider's relevance order are preserved. Quoted trailing words remain literal.

Selected text has an opaque blue highlight and a distinct thin green focus border in both appearances. Online music continues to play progressively, with complete-file caching and seek prioritization where the server supports ranges. Qishui uses publicly available full tracks only. Live search and the automated checks above validate ranking and streaming behavior.
