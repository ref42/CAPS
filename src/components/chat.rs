//! A separate window with its own state, scroll position, assets and drag loop.
//! Never read Caps during render: GPUI associates entities with the last window
//! that rendered them, which can redirect an entity's update_in to this window.
use super::*;
use crate::bilibili_live::account::{Account, SendState, State as AccountState};
use crate::bilibili_live::activity::Kind;
use crate::live_emotes::{Cache, ChatMessage, ImageState, Segment};
use gpui_kit::component::input::{InputEvent, InputState};

mod account_ui;

const RADIUS: f32 = 22.;
const CHAT_TEXT: u32 = 0xf1f3f5;
const CHAT_SECONDARY: u32 = 0xa8b0ba;
const CHAT_ACCENT: u32 = 0x91d8c2;

pub struct ChatIsland {
    parent: WeakEntity<Caps>,
    history: Vec<ChatMessage>,
    language: String,
    font: Font,
    message_size: u32,
    room: Option<u64>,
    status: String,
    proxy: String,
    cache: Cache,
    scroll: ScrollHandle,
    follow_latest: bool,
    reading: bool,
    spotlight: Option<(ChatMessage, Instant)>,
    drag: Option<((f32, f32), (f32, f32))>,
    resize: Option<chat_resize::Drag>,
    region: Option<(Size<Pixels>, f32)>,
    _subscription: Option<Subscription>,
    account: Account,
    composer: Entity<InputState>,
    _composer_subscription: Option<Subscription>,
    in_flight_draft: Option<String>,
}

impl ChatIsland {
    pub fn new(parent: WeakEntity<Caps>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let proxy = parent
            .upgrade()
            .map(|entity| entity.read(cx).settings.danmu_proxy.clone())
            .unwrap_or_default();
        let composer = cx.new(|cx| InputState::new(window, cx).placeholder("Message / 发送弹幕…"));
        let composer_subscription =
            cx.subscribe_in(&composer, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.send_message(window, cx);
                }
                cx.notify();
            });
        let subscription = parent.upgrade().map(|entity| {
            cx.observe(&entity, |this, main, cx| {
                if this.sync(main.read(cx)) {
                    this.cache.reset(cx);
                }
                cx.notify();
            })
        });
        cx.on_release(|this, app| {
            this.cache.reset(app);
            this.clear_login_image(app);
            let _ = this.parent.update(app, |main, cx| {
                main.chat_window_open = false;
                cx.notify();
            });
        })
        .detach();
        // Windows' GPUI backend does not implement start_window_move(). Track
        // the physical button, even after the cursor leaves the moving window.
        cx.spawn_in(window, async move |view, cx| {
            let mut image_tick = Instant::now();
            loop {
                tokio::time::sleep(Duration::from_millis(16)).await;
                if cx
                    .update(|window, app| {
                        view.update(app, |this, cx| {
                            let old_qr = if let AccountState::Qr { image, .. } = &this.account.state
                            {
                                Some(image.clone())
                            } else {
                                None
                            };
                            if this.account.poll() {
                                if !matches!(this.account.state, AccountState::Qr { .. })
                                    && let Some(image) = old_qr
                                {
                                    ImageSource::from(image).remove_asset(cx);
                                }
                                if matches!(this.account.send_state, SendState::Sent) {
                                    if let Some(sent) = this.in_flight_draft.take()
                                        && this.composer.read(cx).value().as_ref() == sent
                                    {
                                        this.composer.update(cx, |input, cx| {
                                            input.set_value("", window, cx)
                                        });
                                    }
                                }
                                cx.notify();
                            }
                            if std::mem::take(&mut this.account.identity_changed) {
                                let _ = this
                                    .parent
                                    .update(cx, |main, cx| main.refresh_chat_identity(cx));
                            }
                            if let Some(resize) = this.resize {
                                if !left_button_down() {
                                    this.resize = None;
                                    let size = window.viewport_size();
                                    let _ = this.parent.update(cx, |main, cx| {
                                        main.settings.chat_window_size =
                                            Some((size.width.as_f32(), size.height.as_f32()));
                                        main.persist();
                                        cx.notify();
                                    });
                                } else if let Some(cursor) = cursor_position() {
                                    resize_chat_window(window, cx, resize.bounds_at(cursor));
                                }
                            }
                            if let Some((pointer, origin)) = this.drag {
                                if !left_button_down() {
                                    this.drag = None;
                                } else if let Some(cursor) = cursor_position() {
                                    let target = (
                                        origin.0 + cursor.0 - pointer.0,
                                        origin.1 + cursor.1 - pointer.1,
                                    );
                                    let target = clamp_to_monitor(
                                        target,
                                        window_size(window).unwrap_or((420., 560.)),
                                        cursor,
                                    );
                                    move_window(window, cx, target);
                                }
                            }
                            if image_tick.elapsed() >= Duration::from_millis(100) {
                                image_tick = Instant::now();
                                if this.spotlight.as_ref().is_some_and(|(message, since)| {
                                    since.elapsed()
                                        > Duration::from_secs(
                                            if matches!(message.kind, Kind::SuperChat { .. }) {
                                                60
                                            } else {
                                                10
                                            },
                                        )
                                }) {
                                    this.spotlight = None;
                                    cx.notify();
                                }
                                let start = this.scroll.top_item().min(this.history.len());
                                let end = (this.scroll.bottom_item() + 1).min(this.history.len());
                                let mut visible: Vec<_> = this.history[start..end.max(start)]
                                    .iter()
                                    .chain(this.spotlight.iter().map(|(message, _)| message))
                                    .flat_map(ChatMessage::media)
                                    .collect();
                                if let AccountState::SignedIn(profile) = &this.account.state
                                    && let Some(url) = &profile.avatar
                                {
                                    visible.push(Segment::Emote(
                                        crate::bilibili_live::danmu::Emote {
                                            text: profile.name.clone(),
                                            url: url.clone(),
                                            width: 32,
                                            height: 32,
                                        },
                                    ));
                                }
                                if this.cache.update(&visible, &this.proxy, cx) {
                                    if this.follow_latest && !this.reading {
                                        this.scroll.scroll_to_bottom();
                                    }
                                    cx.notify();
                                }
                            }
                        })
                    })
                    .and_then(|result| result)
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let mut island = Self {
            parent,
            history: Vec::new(),
            language: "en".into(),
            font: font("Segoe UI"),
            message_size: 14,
            room: None,
            status: String::new(),
            proxy: String::new(),
            cache: Cache::default(),
            scroll: ScrollHandle::new(),
            follow_latest: true,
            reading: false,
            spotlight: None,
            drag: None,
            resize: None,
            region: None,
            _subscription: subscription,
            account: Account::new(&proxy),
            composer,
            _composer_subscription: Some(composer_subscription),
            in_flight_draft: None,
        };
        if let Some(main) = island.parent.upgrade() {
            island.sync(main.read(cx));
        }
        island
    }

    /// Snapshot only; the main island's window and layout are never touched here.
    fn sync(&mut self, main: &Caps) -> bool {
        let reset = self.room != main.danmu_active_room || self.proxy != main.settings.danmu_proxy;
        if reset {
            self.follow_latest = true;
            self.spotlight = None;
        }
        if self.history != main.chat_history {
            if let Some(message) = main
                .chat_history
                .iter()
                .rev()
                .find(|message| !matches!(message.kind, Kind::Danmu))
                && !self.history.contains(message)
                && (matches!(message.kind, Kind::SuperChat { .. })
                    || !self
                        .spotlight
                        .as_ref()
                        .is_some_and(|(active, _)| matches!(active.kind, Kind::SuperChat { .. })))
            {
                self.spotlight = Some((message.clone(), Instant::now()));
            }
            self.history.clone_from(&main.chat_history);
            if self.follow_latest && !self.reading {
                self.scroll.scroll_to_bottom();
            }
        }
        if self.spotlight.as_ref().is_some_and(|(message, _)| {
            message
                .kind
                .id()
                .is_some_and(|id| main.chat_deleted_sc.iter().any(|deleted| deleted == id))
        }) {
            self.spotlight = None;
        }
        self.language.clone_from(&main.settings.language);
        self.font = capsule_font(main.latin_family(), main.han_fallbacks.clone());
        self.message_size = main.settings.chat_font_size;
        self.room = main.danmu_active_room;
        self.status.clone_from(&main.danmu_status);
        self.proxy.clone_from(&main.settings.danmu_proxy);
        reset
    }

    fn tr(&self, en: &'static str, zh: &'static str) -> &'static str {
        if self.language == "zh" { zh } else { en }
    }

    fn adjust_font_size(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.message_size = self
            .message_size
            .saturating_add_signed(delta)
            .clamp(crate::storage::CHAT_FONT_MIN, crate::storage::CHAT_FONT_MAX);
        let size = self.message_size;
        let _ = self.parent.update(cx, |main, cx| {
            main.settings.chat_font_size = size;
            main.persist();
            cx.notify();
        });
        if self.follow_latest {
            self.scroll.scroll_to_bottom();
        }
        cx.notify();
    }

    fn font_controls(&self, cx: &Context<Self>) -> AnyElement {
        row()
            .gap(px(2.))
            .child(
                button("chat-font-smaller", "A−", false, 1.)
                    .w(px(30.))
                    .h(px(30.))
                    .px_0()
                    .text_color(rgb(CHAT_TEXT))
                    .aria_label(self.tr("Decrease message font size", "缩小弹幕字号"))
                    .hover(|style| style.bg(rgba(0xffffff14)).text_color(rgb(CHAT_TEXT)))
                    .when(self.message_size == crate::storage::CHAT_FONT_MIN, |d| {
                        d.opacity(0.4)
                    })
                    .on_click(cx.listener(|this, _, _, cx| this.adjust_font_size(-1, cx))),
            )
            .child(
                div()
                    .w(px(38.))
                    .text_center()
                    .text_size(px(12.))
                    .text_color(rgb(CHAT_SECONDARY))
                    .child(format!("{} pt", self.message_size)),
            )
            .child(
                button("chat-font-larger", "A+", false, 1.)
                    .w(px(30.))
                    .h(px(30.))
                    .px_0()
                    .text_color(rgb(CHAT_TEXT))
                    .aria_label(self.tr("Increase message font size", "放大弹幕字号"))
                    .hover(|style| style.bg(rgba(0xffffff14)).text_color(rgb(CHAT_TEXT)))
                    .when(self.message_size == crate::storage::CHAT_FONT_MAX, |d| {
                        d.opacity(0.4)
                    })
                    .on_click(cx.listener(|this, _, _, cx| this.adjust_font_size(1, cx))),
            )
            .into_any_element()
    }

    fn resize_handles(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        use ResizeEdge::*;
        [
            Top,
            TopRight,
            Right,
            BottomRight,
            Bottom,
            BottomLeft,
            Left,
            TopLeft,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, edge)| {
            let mut handle = div().id(("chat-resize", index)).absolute();
            // The corner target reaches into the rounded silhouette. The
            // straight edge band is only 6 pt, clear of content controls.
            handle = match edge {
                Top => handle.top_0().left(px(24.)).right(px(24.)).h(px(6.)),
                Bottom => handle.bottom_0().left(px(24.)).right(px(24.)).h(px(6.)),
                Left => handle.left_0().top(px(24.)).bottom(px(24.)).w(px(6.)),
                Right => handle.right_0().top(px(24.)).bottom(px(24.)).w(px(6.)),
                TopLeft => handle.top_0().left_0().size(px(24.)),
                TopRight => handle.top_0().right_0().size(px(24.)),
                BottomLeft => handle.bottom_0().left_0().size(px(24.)),
                BottomRight => handle.bottom_0().right_0().size(px(24.)),
            };
            let cursor = match edge {
                Top | Bottom => CursorStyle::ResizeUpDown,
                Left | Right => CursorStyle::ResizeLeftRight,
                TopLeft | BottomRight => CursorStyle::ResizeUpLeftDownRight,
                TopRight | BottomLeft => CursorStyle::ResizeUpRightDownLeft,
            };
            handle
                .cursor(cursor)
                .when(edge == BottomRight, |d| {
                    d.child(
                        canvas(
                            |_, _, _| (),
                            |bounds, _, window, _| {
                                let mut path = PathBuilder::stroke(px(1.2));
                                path.move_to(point(px(2.), px(10.)));
                                path.line_to(point(px(10.), px(2.)));
                                path.move_to(point(px(7.), px(10.)));
                                path.line_to(point(px(10.), px(7.)));
                                path.translate(bounds.origin);
                                if let Ok(path) = path.build() {
                                    window.paint_path(path, rgba(0xffffff66));
                                }
                            },
                        )
                        .absolute()
                        .right(px(8.))
                        .bottom(px(8.))
                        .size(px(12.)),
                    )
                })
                .aria_label(self.tr("Resize chat window", "调整聊天窗口大小"))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        this.drag = None;
                        #[cfg(target_os = "windows")]
                        if let (Some(pointer), Some(bounds), Some(client)) =
                            (cursor_position(), window_rect(window), client_size(window))
                        {
                            let dpi = window.scale_factor();
                            this.resize = Some(chat_resize::Drag {
                                edge,
                                pointer,
                                bounds,
                                minimum: (
                                    chat_resize::MIN_SIZE.0 * dpi + bounds.2 - bounds.0 - client.0,
                                    chat_resize::MIN_SIZE.1 * dpi + bounds.3 - bounds.1 - client.1,
                                ),
                            });
                        }
                        #[cfg(not(target_os = "windows"))]
                        window.start_window_resize(edge);
                        cx.stop_propagation();
                    }),
                )
                .into_any_element()
        })
        .collect()
    }

    fn message(&self, index: usize, message: &ChatMessage) -> AnyElement {
        let sticker_only = matches!(message.body.as_slice(), [Segment::Emote(_)]);
        let scale = self.message_size as f32 / 14.;
        let is_gift = matches!(message.kind, Kind::Gift { .. });
        let is_sc = matches!(message.kind, Kind::SuperChat { .. });
        let own_profile = if let AccountState::SignedIn(profile) = &self.account.state {
            (profile.uid == message.author.uid && message.author.uid != 0).then_some(profile)
        } else {
            None
        };
        let avatar_url = message
            .author
            .avatar
            .as_deref()
            .or_else(|| own_profile.and_then(|profile| profile.avatar.as_deref()));
        let mut body = row()
            .w_full()
            .min_w_0()
            .flex_wrap()
            .gap(px(3.))
            .items_center();
        for (part, segment) in message.body.iter().enumerate() {
            body = match segment {
                Segment::Text(text) => body.child(div().max_w_full().child(text.clone())),
                Segment::Emote(emote) => {
                    let height = if sticker_only {
                        56.
                    } else if is_gift {
                        32.
                    } else {
                        26.
                    } * scale;
                    let width =
                        (height * emote.width as f32 / emote.height.max(1) as f32).clamp(16., 240.);
                    let asset = match self.cache.state(&emote.url) {
                        ImageState::Ready(image) => img(image)
                            .w(px(width))
                            .h(px(height))
                            .object_fit(ObjectFit::Contain)
                            .into_any_element(),
                        // Keep a readable fallback while loading or if the CDN is unavailable.
                        ImageState::Loading | ImageState::Failed => div()
                            .text_color(rgb(CHAT_SECONDARY))
                            .child(if is_gift {
                                String::new()
                            } else {
                                emote.text.clone()
                            })
                            .into_any_element(),
                    };
                    body.child(
                        div()
                            .id(("chat-emote", part))
                            .flex_shrink_0()
                            .min_h(px(height))
                            .aria_label(match &message.kind {
                                Kind::Gift { name, .. } => name.clone(),
                                _ => emote.text.clone(),
                            })
                            .child(asset),
                    )
                }
            };
        }
        let badge = match &message.kind {
            Kind::Danmu => None,
            Kind::SuperChat { yuan, .. } => Some((
                yuan.map(|value| format!("SC · ¥{value}"))
                    .unwrap_or_else(|| "SC".into()),
                0xf6d48b,
            )),
            Kind::Gift { .. } => Some((self.tr("Gift", "礼物").to_owned(), 0xcbb8ef)),
        };
        let author = row()
            .gap(px(8.))
            .flex_wrap()
            .child(
                div()
                    .text_size(px((self.message_size as f32 * 0.85).max(12.)))
                    .font_weight(MEDIUM)
                    .text_color(rgb(CHAT_TEXT))
                    .child(if message.user.is_empty() {
                        self.tr("Viewer", "观众").to_owned()
                    } else {
                        message.user.clone()
                    }),
            )
            .when(own_profile.is_some(), |d| {
                d.child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(CHAT_ACCENT))
                        .child(self.tr("You", "你")),
                )
            })
            .when_some(message.author.medal.as_ref(), |d, (name, level)| {
                d.child(
                    div()
                        .text_size(px(11.))
                        .px(px(5.))
                        .rounded(px(4.))
                        .bg(rgba(0x91d8c218))
                        .text_color(rgb(CHAT_ACCENT))
                        .child(format!("{name} {level}")),
                )
            })
            .when(message.author.guard > 0, |d| {
                d.child(div().text_size(px(11.)).text_color(rgb(0xcbb8ef)).child(
                    match message.author.guard {
                        1 => self.tr("Governor", "总督"),
                        2 => self.tr("Admiral", "提督"),
                        _ => self.tr("Captain", "舰长"),
                    },
                ))
            })
            .when_some(badge, |d, (label, color)| {
                d.child(
                    div()
                        .text_size(px(12.))
                        .font_weight(SEMIBOLD)
                        .text_color(rgb(color))
                        .child(label),
                )
            });
        let content = column()
            .flex_1()
            .min_w_0()
            .flex_shrink_0()
            .gap(px(3.))
            .py(px(8.))
            .text_size(px(self.message_size as f32))
            .child(author)
            .child(body);
        row()
            .id(("chat-message", index))
            .w_full()
            .flex_shrink_0()
            .items_start()
            .gap(px(10.))
            .py(px(5.))
            .when(is_sc, |d| {
                d.px(px(10.))
                    .my(px(4.))
                    .rounded(px(14.))
                    .bg(rgba(0xf6d48b12))
                    .border_1()
                    .border_color(rgba(0xf6d48b38))
            })
            .when(is_gift, |d| {
                d.px(px(10.))
                    .my(px(2.))
                    .rounded(px(14.))
                    .bg(rgba(0xcbb8ef0c))
            })
            .child(div().pt(px(8.)).child(self.avatar(
                avatar_url,
                &message.user,
                32. + (scale - 1.) * 8.,
            )))
            .child(content)
            .into_any_element()
    }
}

impl Render for ChatIsland {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let region = (window.viewport_size(), window.scale_factor());
        if self.region != Some(region) {
            self.region = Some(region);
            apply_chat_region(window, cx, RADIUS);
            if self.follow_latest {
                self.scroll.scroll_to_bottom();
            }
        }
        let heading = self.tr("Live chat", "直播聊天");
        let empty = if self.room.is_some() {
            self.tr("Waiting for messages…", "等待新的弹幕…")
        } else {
            self.tr(
                "Connect to a Bilibili room in Settings to see messages here.",
                "在设置中连接 B 站直播间，弹幕会显示在这里。",
            )
        };
        let close = icon_button(
            "close-chat",
            "×",
            self.tr("Close chat window", "关闭聊天窗口"),
            1.,
        )
        .size(px(32.))
        .text_size(px(20.))
        .text_color(rgb(CHAT_SECONDARY))
        .hover(|style| style.bg(rgba(0xffffff14)).text_color(rgb(CHAT_TEXT)))
        .on_click(|_, window, _| window.remove_window());
        let header = row()
            .w_full()
            .flex_shrink_0()
            .px(px(26.))
            .py(px(14.))
            .gap(px(12.))
            .border_b_1()
            .border_color(rgba(0xffffff12))
            .child(
                column()
                    .id("chat-drag-handle")
                    .flex_1()
                    .min_w_0()
                    .gap(px(4.))
                    .cursor_move()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            #[cfg(target_os = "windows")]
                            {
                                this.resize = None;
                                this.drag = cursor_position().zip(window_origin(window));
                            }
                            #[cfg(not(target_os = "windows"))]
                            window.start_window_move();
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(SEMIBOLD)
                            .child(heading),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(CHAT_SECONDARY))
                            .child(self.tr(
                                "Drag title to move · Drag edges to resize",
                                "拖动标题移动 · 拖动边缘调整大小",
                            )),
                    ),
            )
            .child(close);
        let history = column()
            .id("chat-history-scroll")
            .flex_1()
            .min_h_0()
            .px(px(20.))
            .py(px(8.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .on_hover(cx.listener(|this, hovered, _, cx| {
                this.reading = *hovered;
                if !this.reading && this.follow_latest {
                    this.scroll.scroll_to_bottom();
                }
                cx.notify();
            }))
            .on_scroll(cx.listener(|this, _, _, cx| {
                // Reading history is explicit; new messages no longer steal the scroll position.
                this.follow_latest = false;
                cx.notify();
            }))
            .when(self.history.is_empty(), |d| {
                d.child(
                    div()
                        .py(px(28.))
                        .text_color(rgb(CHAT_SECONDARY))
                        .child(empty),
                )
            })
            .children(
                self.history
                    .iter()
                    .enumerate()
                    .map(|(i, message)| self.message(i, message)),
            );
        let history = column().relative().flex_1().min_h_0().child(history).child(
            div()
                .id("chat-scrollbar")
                .absolute()
                .right(px(12.))
                .top(px(8.))
                .bottom(px(8.))
                .w(px(5.))
                .capture_any_mouse_down(cx.listener(|this, _, _, cx| {
                    this.follow_latest = false;
                    cx.notify();
                }))
                .child(
                    Scrollbar::vertical(&self.scroll)
                        .mode(ScrollbarMode::Always)
                        .styles(|styles| {
                            styles.thumb(|style| style.bg(Hsla::from(rgba(0xffffff40))))
                        }),
                ),
        );
        let footer = column()
            .w_full()
            .flex_shrink_0()
            .pl(px(20.))
            .pr(px(32.))
            .py(px(10.))
            .gap(px(6.))
            .border_t_1()
            .border_color(rgba(0xffffff12))
            .child(
                row().w_full().gap(px(6.)).child(
                    div()
                        .w_full()
                        .min_w_0()
                        .text_ellipsis()
                        .text_size(px(12.))
                        .text_color(rgb(CHAT_SECONDARY))
                        .child(if self.status.is_empty() {
                            self.tr("Not connected", "未连接").to_owned()
                        } else {
                            self.status.clone()
                        }),
                ),
            )
            .child(
                row()
                    .w_full()
                    .justify_between()
                    .gap(px(8.))
                    .child(self.font_controls(cx))
                    .child(
                        button(
                            "chat-latest",
                            if self.follow_latest {
                                if self.reading {
                                    self.tr("Paused", "阅读中")
                                } else {
                                    self.tr("Following latest", "跟随最新")
                                }
                            } else {
                                self.tr("↓ Latest", "↓ 回到最新")
                            },
                            true,
                            1.,
                        )
                        .h(px(30.))
                        .text_size(px(12.))
                        .text_color(rgb(CHAT_ACCENT))
                        .bg(rgba(0x91d8c216))
                        .hover(|style| style.bg(rgba(0x91d8c22a)).text_color(rgb(CHAT_TEXT)))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.follow_latest = true;
                            this.scroll.scroll_to_bottom();
                            cx.notify();
                        })),
                    ),
            );
        let footer = footer.child(self.composer_view(cx));
        let signing_in = matches!(
            self.account.state,
            AccountState::Loading | AccountState::Qr { .. }
        );
        column()
            .relative()
            .size_full()
            .rounded(px(RADIUS))
            .overflow_hidden()
            .bg(rgb(SURFACE_RAISED))
            .text_color(rgb(CHAT_TEXT))
            .text_size(px(14.))
            .line_height(relative(1.45))
            .font(self.font.clone())
            .child(header)
            .child(self.account_panel(window, cx))
            .when(!signing_in, |d| {
                d.when_some(self.spotlight.as_ref(), |d, (message, _)| {
                    d.child(
                        column()
                            .id("paid-spotlight")
                            .flex_shrink_0()
                            .max_h(px(140.))
                            .overflow_y_scroll()
                            .px(px(14.))
                            .child(self.message(usize::MAX, message)),
                    )
                })
                .child(history)
                .child(footer)
            })
            .children(self.resize_handles(cx))
    }
}
