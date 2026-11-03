//! QR sign-in, account identity, avatars and the native message composer.
use super::*;

impl ChatIsland {
    pub(super) fn send_message(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.account.state, AccountState::SignedIn(_))
            || matches!(self.account.send_state, SendState::Sending)
        {
            return;
        }
        let Some(room) = self.room else {
            return;
        };
        let draft = self.composer.read(cx).value().to_string();
        if draft.trim().is_empty() {
            return;
        }
        self.in_flight_draft = Some(draft.clone());
        self.account.send(&self.proxy, room, draft);
        cx.notify();
    }

    pub(super) fn clear_login_image(&self, cx: &mut App) {
        if let AccountState::Qr { image, .. } = &self.account.state {
            ImageSource::from(image.clone()).remove_asset(cx);
        }
    }
    pub(super) fn apply_identity_change(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.account.identity_changed) {
            let _ = self
                .parent
                .update(cx, |main, cx| main.refresh_chat_identity(cx));
        }
    }
    pub(super) fn cancel_sign_in(&mut self, cx: &mut Context<Self>) {
        self.clear_login_image(cx);
        if crate::live::bilibili_live::account::has_session() {
            if let Err(error) = self.account.logout() {
                self.account.state = AccountState::Failed(error);
            }
        } else {
            self.account.cancel();
        }
        self.apply_identity_change(cx);
        cx.notify();
    }

    pub(super) fn account_panel(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let qr_side = (window.viewport_size().height.as_f32() - 170.).clamp(90., 180.);
        let sign_in = || {
            button(
                "chat-sign-in",
                self.tr("Sign in with QR", "扫码登录 B 站"),
                true,
                1.,
            )
            .h(px(34.))
            .text_color(rgb(CHAT_ACCENT))
            .on_click(cx.listener(|this, _, _, cx| {
                this.account.begin(&this.proxy);
                this.apply_identity_change(cx);
                cx.notify();
            }))
        };
        let panel = column()
            .w_full()
            .flex_shrink_0()
            .px(px(20.))
            .py(px(10.))
            .gap(px(8.))
            .border_b_1()
            .border_color(rgba(0xffffff12));
        match &self.account.state {
            AccountState::SignedIn(profile) => panel
                .child(
                    row()
                        .w_full()
                        .gap(px(8.))
                        .child(self.avatar(profile.avatar.as_deref(), &profile.name, 28.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_ellipsis()
                                .text_size(px(12.))
                                .text_color(rgb(CHAT_ACCENT))
                                .child(format!(
                                    "{} · {}",
                                    profile.name,
                                    self.tr("Signed in", "已登录")
                                )),
                        )
                        .child(
                            button("chat-sign-out", self.tr("Sign out", "退出登录"), false, 1.)
                                .h(px(28.))
                                .text_size(px(12.))
                                .text_color(rgb(CHAT_SECONDARY))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Err(error) = this.account.logout() {
                                        this.account.send_state = SendState::Failed(error);
                                    }
                                    this.apply_identity_change(cx);
                                    cx.notify();
                                })),
                        ),
                )
                .into_any_element(),
            AccountState::Qr { image, scanned } => panel
                .id("chat-qr-panel")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(
                    row()
                        .w_full()
                        .justify_between()
                        .child(
                            div()
                                .font_weight(SEMIBOLD)
                                .child(self.tr("Sign in to Bilibili", "登录哔哩哔哩")),
                        )
                        .child(
                            button("chat-cancel-login", self.tr("Cancel", "取消"), false, 1.)
                                .h(px(28.))
                                .text_color(rgb(CHAT_SECONDARY))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.cancel_sign_in(cx);
                                })),
                        ),
                )
                .child(
                    div().flex().justify_center().child(
                        div().rounded(px(12.)).p(px(6.)).bg(rgb(0xffffff)).child(
                            img(image.clone())
                                .w(px(qr_side))
                                .h(px(qr_side))
                                .object_fit(ObjectFit::Contain),
                        ),
                    ),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(CHAT_SECONDARY))
                        .child(if *scanned {
                            self.tr(
                                "Scanned. Confirm sign-in in the Bilibili app.",
                                "已扫码，请在 B 站 App 中确认登录。",
                            )
                        } else {
                            self.tr(
                                "Scan with the Bilibili app. Expires in 3 minutes.",
                                "使用 B 站 App 扫码，二维码 3 分钟内有效。",
                            )
                        }),
                )
                .into_any_element(),
            AccountState::Loading => panel
                .child(
                    row()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(CHAT_SECONDARY))
                                .child(self.tr("Connecting to Bilibili…", "正在连接 B 站…")),
                        )
                        .child(
                            button(
                                "chat-cancel-loading",
                                if crate::live::bilibili_live::account::has_session() {
                                    self.tr("Sign out", "退出登录")
                                } else {
                                    self.tr("Cancel", "取消")
                                },
                                false,
                                1.,
                            )
                            .text_color(rgb(CHAT_SECONDARY))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.cancel_sign_in(cx);
                            })),
                        ),
                )
                .into_any_element(),
            state => panel
                .child(
                    row()
                        .w_full()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(12.))
                                .text_color(rgb(CHAT_SECONDARY))
                                .child(self.tr(
                                    "Read freely. Sign in to send here.",
                                    "可直接阅读，登录后即可在这里发弹幕。",
                                )),
                        )
                        .child(sign_in()),
                )
                .when(matches!(state, AccountState::Expired), |d| {
                    d.child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(0xf6d48b))
                            .child(self.tr(
                                "QR expired. Request a new code.",
                                "二维码已失效，请重新获取。",
                            )),
                    )
                })
                .when_some(
                    if let AccountState::Failed(error) = state {
                        Some(error.clone())
                    } else {
                        None
                    },
                    |d, error| {
                        d.child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(0xf6d48b))
                                .child(error),
                        )
                    },
                )
                .into_any_element(),
        }
    }

    pub(super) fn composer_view(&self, cx: &Context<Self>) -> AnyElement {
        let signed_in = matches!(self.account.state, AccountState::SignedIn(_));
        let sending = matches!(self.account.send_state, SendState::Sending);
        column()
            .w_full()
            .flex_shrink_0()
            .gap(px(5.))
            .when(signed_in, |d| {
                d.child(
                    row()
                        .w_full()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(Input::new(&self.composer).disabled(self.room.is_none())),
                        )
                        .child(
                            button(
                                "chat-send",
                                if sending {
                                    self.tr("Sending…", "发送中…")
                                } else {
                                    self.tr("Send", "发送")
                                },
                                true,
                                1.,
                            )
                            .h(px(34.))
                            .text_color(rgb(CHAT_ACCENT))
                            .when(sending || self.room.is_none(), |d| d.opacity(0.45))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.send_message(window, cx)),
                            ),
                        ),
                )
            })
            .when_some(
                match &self.account.send_state {
                    SendState::Failed(error) => Some(error.clone()),
                    SendState::Sent => Some(self.tr("Sent", "已发送").to_owned()),
                    _ => None,
                },
                |d, message| {
                    d.child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(CHAT_SECONDARY))
                            .child(message),
                    )
                },
            )
            .into_any_element()
    }

    pub(super) fn avatar(&self, url: Option<&str>, name: &str, diameter: f32) -> AnyElement {
        let mut avatar = div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .size(px(diameter))
            .rounded_full()
            .bg(rgba(0x91d8c21c))
            .text_color(rgb(CHAT_ACCENT))
            .font_weight(SEMIBOLD)
            .text_size(px(diameter * 0.42));
        if let Some(url) = url
            && let ImageState::Ready(image) = self.cache.state(url)
        {
            avatar = avatar.child(
                img(image)
                    .size(px(diameter))
                    .rounded_full()
                    .object_fit(ObjectFit::Cover),
            );
        } else {
            avatar = avatar.child(name.chars().next().unwrap_or('·').to_string());
        }
        avatar.into_any_element()
    }
}
