# Bilibili live danmu

Branch: `feature/bilibili-live-danmu`.

## 表情与贴纸 / Emoji and stickers

Unicode emoji 原样保留，并为字体回退链补充系统 emoji 字体。B 站独立贴纸从
`info[0][13]` 提取，文字内嵌表情从 `info[0][15].extra.emots` 提取；两处均支持
对象或 JSON 字符串形式。匹配仅作用于消息正文，不会替换用户名中的同名文字。
图片异步加载，不阻塞弹幕接收或点歌，沿用直播互动的代理设置。加载期间预留与图片
相同大小的占位块，避免原标签闪现和行宽跳动。没有图片元数据、
格式无法识别或加载失败时保留原标签。动态图片目前只显示首帧。

Unicode emoji are preserved with platform emoji font fallbacks. Standalone sticker metadata
comes from `info[0][13]`; inline emotes come from `info[0][15].extra.emots`.
Both object and JSON-string representations are accepted. Substitutions apply only to
the message body, preserving usernames and the original text used for song requests.
Downloads use the live-chat proxy setting and run separately from reception/display.
Loading images reserve their final slot size instead of briefly showing their text labels.
Only supplied Bilibili image CDN URLs are fetched; missing or failed images keep their
original labels. Animated images currently display their first frame.

The image cache holds up to 64 entries, with four concurrent downloads, an eight-second
timeout, a 2 MiB download limit, and bounded decoded image dimensions/allocation.
Disconnecting or reconnecting cancels pending image downloads and clears the cache.

Protocol reference: [blivedm web message model](https://github.com/xfgryujk/blivedm/blob/master/blivedm/models/web.py).

## CAPS settings

Settings has separate **General** and **Live chat** pages. In Live chat, enter a
room ID and press **Connect**. Incoming messages temporarily replace the lyric
line, showing the full username supplied by Bilibili. When the latest message
expires, current lyrics return; without music, system stats return.
**Disconnect** stops reception. The HTTP proxy field under **Connection options**
applies to live-chat HTTP and WebSocket traffic;
blank means a direct connection. Room ID, proxy, display choice, and the
song-request toggle persist across restarts. Press Apply after editing the
room or proxy; invalid input leaves an existing connection intact.

点歌使用与搜索框相同的 `点歌 歌曲 [歌手] [平台]` 格式，例如 `点歌 晴天 周杰伦 qq`。
歌手和平台均可省略；支持网易云、QQ、酷狗、汽水及搜索框中的平台别名。
优先在指定平台中匹配歌名和歌手；该平台没有返回结果时使用其他平台。
歌曲已在普通播放列表中时，将原条目移到列表末尾并接受点歌，不重复添加音轨；只有正在播放或等待播放的点歌会跳过重复请求。已播放完的歌曲可以再次点播。
点歌搜索成功后立即打断普通歌曲（包括暂停或加载中的普通歌曲）。当前正在播放、暂停或加载点歌时，后续点歌按到达顺序等待，不互相打断；点歌优先于随机和单曲循环。点歌结束后，被打断的普通歌曲从头播放。
停止播放会清除等待点歌的优先级；关闭点歌、断开连接或切换直播间会取消等待点歌的优先级，不打断当前音频。已添加的歌曲仍保留在普通队列。

With **Song requests** on, `点歌 song [artist] [platform]` enqueues an online search.
Artist and platform are optional. The same platform aliases as the search box
work, for example `点歌 Hello Adele qq`. The platform suffix is removed before
matching the title and artist. The preferred platform ranks first, with other
providers used when it returns no results. Within that group, exact title/artist
matches rank first; otherwise the provider's first result is used. The selected track is
added to the queue and immediately interrupts regular music, including paused or
loading regular tracks. While a chat-requested track is playing, paused, or loading,
further requests wait in arrival order without interrupting it. Waiting requests
take priority over shuffle and repeat-one, including when using Next. Existing
entries are moved to the queue's end by source and song ID, without adding duplicate
tracks. Only active or waiting chat requests are deduplicated; finished songs can
be requested again.
Failed requests advance to the next waiting request. After requests finish, the
interrupted regular song restarts from the beginning.

Stopping playback clears waiting request priority. Disabling requests, disconnecting,
or switching rooms cancels waiting priority without interrupting current audio.
Already-added tracks remain in the normal queue. Request priority is session-only.

Song searches run sequentially with eight waiting slots and a 20-second timeout.
One request is accepted every three seconds; repeated queries are suppressed
for 60 seconds. Turning requests off invalidates queued and completed results.
Changing rooms cancels old song requests immediately, but keeps receiving the old
room's chat until the new room authenticates. The new room's buffered messages are
then consumed without clearing them; the old connection is dropped at that point.
During this handoff the capsule still labels messages with the active room, and
old-room song requests are ignored. Repeated Apply for the same room/proxy does
not restart the connection; choosing the active room again cancels the pending
switch. A failed new connection keeps retrying while the old room remains active.
Disconnect stops both active and pending sessions. Connection status uses a
separate channel so a full chat buffer cannot discard authentication notifications.
Connection and request results appear below the controls.

The display shows the newest received message on the next UI tick. New arrivals
replace the current line immediately; multiple arrivals in one tick collapse to
the newest line, so the capsule never replays a backlog. Song requests are parsed
before display replacement and retain their separate queue and rate limits.
When no newer message arrives, the current line remains for 4–120 seconds based
on its length so the full username and text can scroll, then lyrics return.
The network handoff remains bounded. This display follows live chat rather than
archiving every message.

## Findings

Anonymous live reception works in the test environment. A room ID is enough;
the receiver obtains public WBI signing keys, an anonymous `buvid3`, and a room
connection token before authenticating a secure WebSocket with `uid: 0`.
No account cookie, browser profile, or login credentials are used.

This captures messages delivered while connected, not a complete historical
archive. Disconnections leave gaps, and the server may filter or withhold events.
The recent-chat endpoint is a limited snapshot, not a reliable substitute for a
live connection or a complete history API. Anonymous usernames and UIDs may be
masked; the receiver does not depend on either being available.

The official Open Live alternative uses developer keys, an application ID, and
the streamer's authorization code. It does not offer the same room-ID-only setup.

## Run

From the repository root:

```powershell
cargo run --locked --example bilibili_danmu -- 1747113738 --seconds 60
```

If a proxy is required, use the same route for HTTP setup and WebSocket traffic:

```powershell
cargo run --locked --example bilibili_danmu -- 1747113738 --seconds 120 --proxy http://127.0.0.1:10808
```

The default duration is 60 seconds. Ctrl+C stops the capture early. The example
does not automatically use system proxy settings. Its explicit proxy option
supports HTTP CONNECT without proxy credentials.

Chat is printed to stdout as UTF-8 JSON lines with `room_id`, `kind`, `text`,
`user`, `uid`, the server's `timestamp`, and an `emotes` array containing each
supplied image's text label, URL, width, and height. Regular danmu timestamps are usually
milliseconds; Super Chat timestamps are usually seconds. Connection diagnostics
and the final counts go to stderr. Add `--raw` to emit every received JSON command
instead, including gifts and room events. Raw mode does not print the connection
token or authentication packet.

To save a capture in PowerShell with explicit UTF-8 encoding:

```powershell
cargo run --locked --example bilibili_danmu -- 1747113738 --seconds 120 |
    Out-File -Encoding utf8 "$env:TEMP\caps-danmu.jsonl"
```

The receiver sends 30-second heartbeats, detects missing authentication/heartbeat
responses, and reconnects with a fresh token and a capped backoff. It supports
concatenated packets, Brotli and zlib compression, and both regular danmu and
Super Chat text. Packet size, decompressed size, and nesting depth are bounded.

## Observed results — 2026-10-26

- A plain unsigned `getDanmuInfo` request returned `-352`. WBI-signed setup then
  succeeded with an anonymous fingerprint and no account cookie.
- Room `1747113738` was live. The initial 45-second capture authenticated,
  received two heartbeat replies and 12 room events, but no chat messages.
  A second 120-second capture received four heartbeats and 16 events, again
  without chat. Chat reception in this particular room is therefore not yet
  verified against a known message.
- Public room short ID `6` resolved to `7734200`. A 45-second capture authenticated,
  received two heartbeat replies and 37 events, including **five actual danmu
  messages**. This verifies end-to-end anonymous chat reception, not just a socket
  connection. Identities happened to be unmasked in this sample; that is not a
  guarantee for other rooms or sessions.
- Five tests cover WBI signing against an independently calculated fixture,
  concatenated packets, both compression formats, malformed/oversized input, and
  text extraction with masked identities. The example also passes Clippy with
  warnings denied.

```powershell
cargo test --locked --example bilibili_danmu
cargo clippy --locked --example bilibili_danmu -- -D warnings
```

CAPS and the CLI example share `src/bilibili_live.rs` and its packet decoder.
`src/live_mode.rs` owns the bounded UI handoff, display rotation, request parsing,
search worker, cancellation, and rate limits. The example remains useful for
diagnostics and JSON-line captures.

## Sources

- [blivedm](https://github.com/xfgryujk/blivedm): maintained client supporting web
  and official Open Live connections.
- [Web client implementation](https://github.com/xfgryujk/blivedm/blob/master/blivedm/clients/web.py):
  anonymous UID, buvid initialization, WBI signing, server discovery, authentication.
- [WebSocket implementation](https://github.com/xfgryujk/blivedm/blob/master/blivedm/clients/ws_base.py):
  framing, heartbeats, reconnection, Brotli/zlib packet decoding.
- [Anonymous web example](https://github.com/xfgryujk/blivedm/blob/master/sample.py):
  documents that `SESSDATA` may be omitted, with identity masking possible.
- [Open Live example](https://github.com/xfgryujk/blivedm/blob/master/open_live_sample.py):
  developer keys, app ID, and streamer authorization requirements.
- [Bilibili Open Live documentation](https://open-live.bilibili.com/document/657d8e34-f926-a133-16c0-300c1afc6e6b).

Web endpoints tested directly:

- `https://api.live.bilibili.com/room/v1/Room/room_init?id=ROOM_ID`
- `https://api.bilibili.com/x/frontend/finger/spi`
- `https://api.bilibili.com/x/web-interface/nav`
- `https://api.live.bilibili.com/xlive/web-room/v1/index/getDanmuInfo`
- `https://api.live.bilibili.com/xlive/web-room/v1/dM/gethistory?roomid=ROOM_ID`
