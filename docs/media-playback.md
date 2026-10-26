# 统一媒体与流式播放 / Unified media and streaming

分支：`feature/unified-media-streaming`。

## 使用方式

搜索页保留三个入口：**在线 / 媒体 / 本地**。

- 在线搜索和随机添加涵盖网易云、QQ 音乐、酷狗和汽水音乐；搜索结果标注来源，汽水歌曲也支持歌词和 JSON 队列导入、导出。
- 媒体入口接受 Bilibili、YouTube、抖音链接或整段分享文字，自动识别来源。支持 Bilibili 分 P 和 b23.tv 短链接；一次导入一个链接。导入追加到队列并跳过重复项，不打断当前歌曲。
- 所有在线来源通过同一播放流程边下载边播放。队列点击、上一首 / 下一首、自动续播、循环播放及空闲时的弹幕点歌都使用该流程。本地文件和已完成的缓存直接播放。
- 停止或切歌会取消尚未完成的下载；只有完整下载且初始音频解码成功的文件才成为缓存。拖动进度条时优先请求目标数据，缓冲静音不会推进歌曲时间。

## 来源限制

汽水接入公开目录、歌词和完整音轨，不要求登录，也不读取浏览器 Cookie。仅试听、加密或需要登录的音轨会报告不可用，不会替换成其他平台的同名歌曲。公开接口可能随平台调整而变化。

YouTube 沿用直接媒体地址解析；部分视频或请求需要 JS 签名、n-challenge 或 PO token，当前不支持。实测同一视频可能一度可播放、随后无法返回直接地址。

### YouTube 手动会话

遇到 `LOGIN_REQUIRED` / “Sign in to confirm you’re not a bot” 时，可在 **设置 → 常规 → YouTube** 手动提供会话：

1. 自行在 YouTube 完成登录或验证，打开浏览器开发者工具的 Network（网络）面板后刷新视频页。
2. 选择发往 `www.youtube.com` 的页面请求，复制 Request Headers（请求头）中的 `Cookie` 值（不是 `Set-Cookie`，也不是整条 cURL 命令）。
3. 粘贴到 CAPS 的隐藏输入框，点击 **应用**，再重试媒体链接。会话有效性由 YouTube 判断；应用成功只表示输入格式正确。
4. 点击 **清除** 可恢复后续请求的匿名访问。已开始的请求可能继续使用其会话快照；停止播放可取消当前加载任务。

Cookie 是登录凭据，只粘贴到本地 CAPS，不要放进聊天、截图或问题报告。会话仅保存在本次进程内存中，退出后清除，不写入配置或队列 JSON。CAPS 不启动浏览器、不读取浏览器文件；网页和 WEB API 请求使用 Rust HTTP，WEB 请求附带 SAPISID 签名，媒体下载和其他来源不携带会话 Cookie。验证跳转不会自动跟随。

`x2OfvEnMviU` 的匿名请求曾返回 `LOGIN_REQUIRED`；后续发现导入成功后，播放时重复解析又可能失败。现在导入解析出的地址在内存中按视频与会话隔离，最多保留 2 分钟、8 条，并供下一次播放复用一次，不写入队列。本次匿名实测成功识别《靜悄悄（Remix版）》/ EHPMusicChannel / 165 秒，约 10.91 秒音频就绪，当时仅下载 1.57 / 4.72 MB，实际播放和跳转至 60 秒均通过。匿名访问仍可能受平台验证限制。会话请求、签名、输入校验及凭据隐藏已通过自动测试，未提供有效登录会话进行实测；Cookie 不保证解决 JS 签名、n-challenge 或 PO-token 限制。

`sy3nQLseSmc` 的短链接和带 `list` / `start_radio` 参数的完整链接均能提取正确视频 ID。该视频实测出现过 `LOGIN_REQUIRED`（要求确认不是机器人），独立未登录浏览器也遇到相同限制。现在使用普通 watch 页面地址，并在失败时保留平台原因，不再一律提示 metadata missing。后续实测成功识别《浪漫愛》、Dreamer Music、193 秒时长；约下载 1.48 / 9.35 MB 后开始播放，并成功跳到 60 秒。测试通过不代表平台的间歇性验证限制已消除。

抖音只使用 Rust HTTP 客户端读取公开分享页和页面内的路由数据，不启动 Edge / Chrome，也不读取浏览器配置。需要 JavaScript、验证、登录或仅限 App 的内容会提示不可用，详见[抖音说明](douyin-audio.md)。

服务器支持 HTTP Range 时可优先下载跳转位置；否则按顺序下载，向前跳转可能需要等待。容器索引在文件尾部也可能增加首次缓冲时间。分片 MP4 使用项目已有的 Symphonia 0.6 解码，避免旧解复用器扫描整段媒体后才开始播放。

## 验证记录（2026-10-26）

以下为本机网络实测，时间包含来源解析，不能视为所有歌曲的速度保证。播放通过默认音频设备静音验证，并检查实际播放时间与跳转后继续输出。

| 来源 | 样例 | 音频就绪 | 就绪时下载量 / 总量 | 结果 |
| --- | --- | --- | --- | --- |
| 网易云 | 晴天（深情版）/ Lucky小爱 | 0.93 秒 | 0.52 / 11.16 MB | 播放及跳转通过 |
| QQ 音乐 | 晴天（交响改编版）/ 维也纳碎片 | 0.69 秒 | 0.52 / 1.91 MB | 播放及跳转通过 |
| 酷狗 | 晴天（交响改编版）/ 维也纳碎片 | 0.53 秒 | 0.52 / 1.91 MB | 播放及跳转通过 |
| 汽水音乐 | 晴天 / 晚风纯音，约 237 秒 | 5.61 秒 | 1.35 / 7.64 MB | 歌词、播放及跳转通过 |
| Bilibili | `BV1xUUiBKE1n` | 1.38 秒 | 1.08 / 59.27 MB | 分片 MP4 提前播放及跳转通过 |
| 抖音 | `https://v.douyin.com/gYsL-kehIgg/` | 当前不可用 | — | 纯 HTTP 页面缺少媒体元数据；已删除浏览器回退 |
| YouTube | `iKbylgzysHw` | 首次 12.17 秒 | 首次 4.60 / 151.40 MB | 首次播放及跳转通过；后续请求受直接地址限制 |
| YouTube | `x2OfvEnMviU` | 10.91 秒 | 1.57 / 4.72 MB | 匿名播放及跳转通过；复用导入地址避免重复解析 |

YouTube 的首次记录使用原 MP4 解码路径；更新解码器后，另一个样例 `jNQXAC9IVRw` 成功播放，但其文件只有约 0.63 MB，就绪时已下载完成，因此不能用该样例证明提前播放。

本地 HTTP 回归测试覆盖 Range、普通响应、未知长度响应、缓冲时间、跳转后音频内容、缓存完整性、错误响应、网络中断和启动取消清理。解析测试覆盖分享文字、域名伪装、多链接歧义，以及汽水试听 / 加密音轨拒绝。界面使用独立静音配置检查。

```powershell
cargo test --locked --offline --bin caps --example douyin_audio --example media_playback
cargo clippy --locked --offline --bin caps --example douyin_audio --example media_playback -- -D warnings

# 在线实测需要网络和默认音频设备；播放音量为零。
cargo run --locked --example media_playback -- qishui 晴天
cargo run --locked --example media_playback -- bilibili BV1xUUiBKE1n
cargo run --locked --example media_playback -- douyin https://v.douyin.com/gYsL-kehIgg/
```

## English

For YouTube verification errors, **Settings → General → YouTube** accepts a manually copied `Cookie` request header. Copy it from a `www.youtube.com` page request in the browser's Network panel after completing sign-in/verification yourself, paste into CAPS, click **Apply**, and retry. **Clear** affects new requests; existing jobs can retain their session snapshot. Apply validates syntax, not account access. The masked value stays in process memory until exit and is never saved to settings or queue exports. CAPS neither launches nor reads browsers. Web requests use Rust HTTP and SAPISID authentication; media downloads and other providers receive no session cookies. Treat the value as a login credential and keep it out of reports. Authenticated playback has not been verified with a valid session. Signature/n-challenge/PO-token requirements remain unsupported.

The exact `x2OfvEnMviU` URL later passed anonymous playback and seeking: audio ready after 10.91 seconds, only 1.57 of 4.72 MB downloaded, and seeking resumed at 60.60 seconds. Import had succeeded while an immediate second resolution failed. CAPS now reuses the import's stream URL once, for up to two minutes, with a bounded in-memory cache partitioned by video and session. URLs never enter exported tracks; anonymous access can still be challenged by YouTube.

The search panel now has **Online / Media / Local** tabs. Online includes NetEase, QQ Music, KuGou, and Qishui, with lyrics and queue serialization. Media automatically detects Bilibili, YouTube, and Douyin links or share messages, including Bilibili page selection and short links. Imports append unique tracks without interrupting playback.

Every online playback entry point uses the shared buffered decoder and download cache. Local files and completed caches play directly. Stop or track changes cancel incomplete downloads. Range requests prioritize seek targets; ordinary HTTP responses download progressively. Buffering silence does not advance media time. Fragmented MP4 uses the existing Symphonia 0.6 dependency to avoid scanning the whole file at startup.

Qishui supports publicly available full tracks only; previews and protected streams are rejected. YouTube still depends on direct URLs and can fail when signature or token challenges are required. Douyin uses direct Rust HTTP page parsing only; pages requiring JavaScript, verification, login, or the app can be unavailable. Sequential servers and media indexes at the end can delay startup or seeking.

Both URL forms for `sy3nQLseSmc`, including radio/playlist parameters, resolve to the same video. YouTube intermittently required sign-in even in a fresh browser session. CAPS now uses the ordinary watch URL and reports the actual platform restriction. Subsequent tests imported the correct title/artist/duration, started with 1.48 MB of 9.35 MB downloaded, and resumed after seeking to 60 seconds; anonymous access remains intermittent.

The table above records real muted playback tests, including startup byte counts. The Douyin sample currently fails direct HTTP resolution after removal of the browser fallback; its previous browser-assisted playback timings do not validate the current resolver. The YouTube success was intermittent; its later direct-URL failures remain a provider limitation. Automated tests cover progressive HTTP playback, seeking, buffering, complete caches, failures, cancellation, URL detection, and Qishui restrictions.

## 接口参考 / Endpoint references

- [Qishui public catalogue and SEO endpoint research](https://github.com/2706521883-lgtm/folia-qishui/blob/main/electron/qishui/qishui-api.js)
- [Qishui share-page analysis reference](https://github.com/520Qiuyu/qishuiMusicAnalysis)
- [HTTP Range requests](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Range_requests)

接口参考仅用于确认公开端点；实现使用 Rust 独立编写，没有添加解析服务或运行时依赖。
