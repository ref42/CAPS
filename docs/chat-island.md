# Chat island

The chat window opens explicitly from Settings > Live chat. It remains above other windows and has its own layout and native window state.

## Controls

- Drag the title to move the window.
- Drag any edge or corner to resize it. The lower-right grip makes this discoverable. Minimum client size: 320 × 260 logical pixels.
- Window size is remembered independently of the main capsule and fitted to the display when reopened.
- Use A− and A+ in the footer to change message text from 12 to 28 pt. Stickers scale with the text. This preference is saved separately from capsule size.
- Hover over history to pause automatic following. Scrolling keeps following paused until Latest is clicked.
- Choose Sign in with QR, scan with the Bilibili app, and confirm on your phone. Then type in the native composer and click Send or press Enter. No browser is opened.
- Sign out removes the remembered session. Sessions are encrypted with Windows per-user DPAPI in a separate file, never in settings JSON. If Bilibili expires the session, sign in again.
- Messages are sent once per explicit action. Failures preserve the draft; uncertain delivery asks you to check history before retrying. Room restrictions and server length limits are reported by Bilibili.

## Identity and appearance

Available sender avatars, fan medals and guard badges are retained from live events. Initials stand in for unavailable avatars. Authenticated connections use the signed-in UID and session, and messages identified as belonging to the signed-in account receive a You marker. Login does not fabricate identities or override fields Bilibili omits.

## Arrivals and activity

Legacy INTERACT_WORD and protobuf INTERACT_WORD_V2 entry events appear as quiet arrival rows, including viewers who never send danmu. Follow, share and like events are not mistaken for arrivals. Arrival events never trigger song requests or paid-message highlights. Timestamped entry IDs suppress duplicate old/new-format delivery.

Message authors show recent activity or last-seen time, keyed by UID rather than display name. This is evidence of activity, not a live online roster: Bilibili's web stream provides no reliable per-viewer departure notification. Silence is never labeled as leaving. Connection loss, identity reconnect and room changes invalidate activity evidence. History survives disconnection with unknown presence. The activity cache is bounded to 1,000 viewers and clears on room change.

## Paid events

Super Chats have an amber card with sender, price (when supplied), and message. Gifts have a distinct label, sender, gift name, quantity, and an icon when supplied by the server. SEND_GIFT and protobuf SEND_GIFT_V2 are supported. Combo summaries and translated SC copies are excluded to avoid double display; transaction IDs suppress retransmissions.

The latest SC is highlighted for 60 seconds. Gifts can be highlighted for 10 seconds when no SC is highlighted. New SCs replace the highlighted SC; ordinary messages and gifts cannot displace it. History retains up to 200 entries. Server SC deletion events remove both history entries and their highlight.

## Delivery, latency and retention

The websocket-to-UI inbox has no fixed-capacity overflow drop. Sending never waits for UI work or disk writes, so a busy room cannot stall heartbeats through queue backpressure. Song searches keep their separate bounded work queue. The UI drains received events for up to 2 ms per tick instead of a fixed 64 events; backlog is visible while catching up. The 200 rows are the current display window, not the complete record.

Raw received danmu, SC, gift, arrival and deletion commands are independently archived before UI consumption, including original text before display truncation. Use **Archive** in the chat footer to reveal `%APPDATA%\\CAPS\\chat-history`. Each run has its own `session-<pid>-<timestamp>` folder, preventing separate app instances from overwriting each other. Each `room-<id>.jsonl` file contains receipt timestamps, room IDs and the original commands. No automatic purge is performed. These files contain viewers' received chat data and can be removed manually when no longer needed.

A dedicated writer batches records, syncs completed batches to disk, retries failed batches and exposes storage failures in the chat window. Normal application shutdown stops networking then drains the writer. Force termination, power loss, unavailable disk space or sustained traffic beyond available memory/disk throughput cannot be made lossless; storage failure must never be advertised as successful archival. The inbox and writer queue buffer bursts in memory and are not a promise of unlimited capacity. Bilibili/network disconnects can also create gaps before CAPS receives a message; the web connection does not replay those gaps automatically.

Regression tests retain and drain 100,000 ordered events with the consumer paused, archive 10,000 ordered Unicode records, drain the archive on shutdown and recover a pending batch after a simulated disk-path failure. These measure internal delivery and persistence, not end-to-end network/display latency.

## Implementation

- `src/ui/chat.rs`: independent window state, layout, drag/resize interaction, history following and font controls.
- `src/ui/chat/account_ui.rs`: QR sign-in panel, account controls, avatars and message composer.
- `src/live/bilibili_live/account.rs`: cancellable QR polling, profile validation and explicit sending.
- `src/live/bilibili_live/account/vault.rs`: Windows per-user encrypted session persistence.
- `src/ui/windowing/chat_resize.rs`: physical-pixel resize geometry and logical size restoration.
- `src/live/bilibili_live/activity.rs`: typed SC/gift events, bounded protobuf decoding and stable transaction keys.
- `src/live/live_emotes.rs`: history, deduplication, validated image loading and decoded image cache.
- `src/providers/`: music provider implementations and streaming decoder.

Main and chat update loops resolve their captured windows explicitly. Chat rendering uses a snapshot rather than reading the main entity during render. This prevents GPUI entity/window association from redirecting the main capsule's geometry updates to chat.

Native geometry operations are deferred so synchronous Windows resize/move callbacks cannot re-enter an already borrowed GPUI App. The rounded native mask leaves room outside the painted antialiased curve. Closing the main capsule quits the application, including the chat window.

## Validation

Automated checks cover all eight resize directions, opposite-edge anchoring, minimum size at high DPI, display fitting, preference persistence, legacy and batched gifts, SC amounts/deletions, duplicate suppression, Unicode, malformed packets and image URL restrictions. Desktop interaction and antialiasing still need visual verification on Windows; the automation surface has not exposed the popup as a targetable window.

QR endpoint smoke testing requests a fresh code and verifies its unscanned status without displaying keys, authorizing an account or sending messages. Real phone confirmation and delivery require an account and are not exercised automatically. Cancellation, Unicode form fields, alternate send-error fields and DPAPI round trips/tamper rejection have automated coverage.
