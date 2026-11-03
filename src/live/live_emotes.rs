//! Inline live-chat images. Network and decoding never block chat reception or UI ticks.
use crate::live::bilibili_live::activity::Kind;
use crate::live::bilibili_live::danmu::{Emote, image_url};
use gpui_kit::{App, Image, ImageFormat, ImageSource};
use std::{collections::HashMap, sync::Arc, time::Duration};

#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    Text(String),
    Emote(Emote),
}

/// History keeps the original text for requests/fallbacks and the image metadata
/// for rendering. Reducing events to (user, text) loses all sticker information.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatMessage {
    pub user: String,
    pub body: Vec<Segment>,
    pub kind: Kind,
    pub author: crate::live::bilibili_live::activity::Author,
}

impl ChatMessage {
    pub fn new(user: &str, text: &str, emotes: &[Emote]) -> Self {
        let mut body = segments("", text, emotes);
        if body.is_empty() {
            body.push(Segment::Text(text.to_owned()));
        }
        Self {
            user: user.to_owned(),
            body,
            kind: Kind::Danmu,
            author: crate::live::bilibili_live::activity::Author::default(),
        }
    }

    pub fn media(&self) -> Vec<Segment> {
        let mut segments = self.body.clone();
        if let Some(url) = &self.author.avatar {
            segments.push(Segment::Emote(Emote {
                text: self.user.clone(),
                url: url.clone(),
                width: 32,
                height: 32,
            }));
        }
        segments
    }
}

/// A retransmission (or the old and new gift formats arriving together) updates
/// one row rather than displaying the same transaction twice.
pub fn push_history(history: &mut Vec<ChatMessage>, message: ChatMessage) -> bool {
    if let Some(id) = message.kind.id()
        && let Some(existing) = history.iter_mut().find(|old| old.kind.id() == Some(id))
    {
        *existing = message;
        return false;
    }
    history.push(message);
    if history.len() > 200 {
        history.drain(..history.len() - 200);
    }
    true
}

pub fn delete_super_chats(history: &mut Vec<ChatMessage>, ids: &[String]) {
    history.retain(|message| !matches!(&message.kind, Kind::SuperChat { id: Some(id), .. } if ids.contains(id)));
}

/// Match only in the message body, never in a viewer's name. Keep Unicode intact.
pub fn segments(user: &str, text: &str, emotes: &[Emote]) -> Vec<Segment> {
    if emotes.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::new();
    if !user.is_empty() {
        result.push(Segment::Text(format!("{user}：")));
    }
    let mut rest = text;
    while !rest.is_empty() {
        let next = emotes
            .iter()
            .filter(|e| !e.text.is_empty())
            .filter_map(|e| rest.find(&e.text).map(|index| (index, e)))
            .min_by_key(|(index, e)| (*index, std::cmp::Reverse(e.text.len())));
        let Some((index, emote)) = next else {
            result.push(Segment::Text(rest.into()));
            break;
        };
        if index > 0 {
            result.push(Segment::Text(rest[..index].into()));
        }
        result.push(Segment::Emote(emote.clone()));
        rest = &rest[index + emote.text.len()..];
    }
    result
}

struct Entry {
    // None caches a failed download too, avoiding retries every 8 ms.
    image: Option<Arc<Image>>,
    used: u64,
}

pub enum ImageState {
    Loading,
    Ready(Arc<Image>),
    Failed,
}

#[derive(Default)]
pub struct Cache {
    entries: HashMap<String, Entry>,
    pending: HashMap<String, tokio::task::JoinHandle<Option<Arc<Image>>>>,
    clock: u64,
}

impl Cache {
    pub fn state(&self, url: &str) -> ImageState {
        match self.entries.get(url) {
            Some(Entry {
                image: Some(image), ..
            }) => ImageState::Ready(image.clone()),
            Some(Entry { image: None, .. }) => ImageState::Failed,
            // Includes images waiting for one of the bounded download slots.
            None => ImageState::Loading,
        }
    }

    /// Release decoded assets and cancel outstanding downloads on room/proxy changes.
    pub fn reset(&mut self, cx: &mut App) {
        for (_, entry) in self.entries.drain() {
            if let Some(image) = entry.image {
                ImageSource::from(image).remove_asset(cx);
            }
        }
        for (_, task) in self.pending.drain() {
            task.abort();
        }
    }

    /// At most four downloads and 64 cached images, including failure entries.
    /// Callers supply only visible messages, not a busy room's entire backlog.
    pub fn update(&mut self, segments: &[Segment], proxy: &str, cx: &mut App) -> bool {
        self.clock = self.clock.wrapping_add(1);
        let mut changed = false;
        let finished: Vec<_> = self
            .pending
            .iter()
            .filter(|(_, task)| task.is_finished())
            .map(|(url, _)| url.clone())
            .collect();
        for url in finished {
            let task = self.pending.remove(&url).unwrap();
            use futures_util::FutureExt;
            let image = task.now_or_never().and_then(Result::ok).flatten();
            if self.entries.len() >= 64
                && let Some(old) = self
                    .entries
                    .iter()
                    .min_by_key(|(_, e)| e.used)
                    .map(|(key, _)| key.clone())
                && let Some(image) = self.entries.remove(&old).and_then(|e| e.image)
            {
                ImageSource::from(image).remove_asset(cx);
            }
            self.entries.insert(
                url,
                Entry {
                    image,
                    used: self.clock,
                },
            );
            changed = true;
        }
        for segment in segments {
            let Segment::Emote(emote) = segment else {
                continue;
            };
            if let Some(entry) = self.entries.get_mut(&emote.url) {
                entry.used = self.clock;
            } else if self.pending.len() < 4 && !self.pending.contains_key(&emote.url) {
                let url = emote.url.clone();
                let proxy = proxy.to_owned();
                self.pending.insert(
                    url.clone(),
                    tokio::spawn(async move { load(&url, &proxy).await }),
                );
            }
        }
        changed
    }
}

impl Drop for Cache {
    fn drop(&mut self) {
        for task in self.pending.values() {
            task.abort();
        }
    }
}

async fn load(url: &str, proxy: &str) -> Option<Arc<Image>> {
    let url = image_url(url)?;
    let mut builder = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 CAPS")
        .timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none());
    if proxy.is_empty() {
        builder = builder.no_proxy();
    } else {
        builder = builder.proxy(reqwest::Proxy::all(proxy).ok()?);
    }
    let mut response = builder
        .build()
        .ok()?
        .get(url)
        .header("Referer", "https://live.bilibili.com/")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    const LIMIT: usize = 2 * 1024 * 1024;
    if response
        .content_length()
        .is_some_and(|length| length > LIMIT as u64)
    {
        return None;
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if bytes.len() + chunk.len() > LIMIT {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    tokio::task::spawn_blocking(move || {
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .ok()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(2048);
        limits.max_image_height = Some(2048);
        limits.max_alloc = Some(32 * 1024 * 1024);
        reader.limits(limits);
        // Preserve transparency/aspect ratio. Animated assets currently use their first frame.
        let decoded = reader.decode().ok()?.thumbnail(256, 256);
        let mut png = std::io::Cursor::new(Vec::new());
        decoded.write_to(&mut png, image::ImageFormat::Png).ok()?;
        Some(Arc::new(Image::from_bytes(
            ImageFormat::Png,
            png.into_inner(),
        )))
    })
    .await
    .ok()
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_deduplicates_paid_events_and_honors_sc_removal() {
        let mut history = Vec::new();
        let mut sc = ChatMessage::new("viewer", "hello", &[]);
        sc.kind = Kind::SuperChat {
            id: Some("sc:42".into()),
            yuan: Some(30),
        };
        assert!(push_history(&mut history, sc.clone()));
        assert!(!push_history(&mut history, sc));
        let mut gift = ChatMessage::new("viewer", "Rose ×2", &[]);
        gift.kind = Kind::Gift {
            id: Some("gift:abc:7".into()),
            name: "Rose".into(),
            quantity: 2,
        };
        assert!(push_history(&mut history, gift.clone()));
        assert!(!push_history(&mut history, gift));
        assert_eq!(history.len(), 2);
        delete_super_chats(&mut history, &["sc:42".into()]);
        assert_eq!(history.len(), 1);
        assert!(matches!(history[0].kind, Kind::Gift { .. }));
        for _ in 0..250 {
            push_history(
                &mut history,
                ChatMessage::new("viewer", "repeated plain message", &[]),
            );
        }
        assert_eq!(history.len(), 200);
    }

    #[test]
    fn history_retains_stickers_and_plain_unicode_messages() {
        let emote = Emote {
            text: "[微笑]".into(),
            url: "https://i0.hdslb.com/smile.png".into(),
            width: 180,
            height: 60,
        };
        let message = ChatMessage::new("观众", "你好[微笑]", &[emote.clone()]);
        assert_eq!(message.user, "观众");
        assert_eq!(
            message.body,
            vec![Segment::Text("你好".into()), Segment::Emote(emote.clone())]
        );
        assert_eq!(
            ChatMessage::new("观众", "[微笑]", &[emote.clone()]).body,
            vec![Segment::Emote(emote)]
        );
        assert_eq!(
            ChatMessage::new("观众", "晚安👩‍💻[未提供图片]", &[]).body,
            vec![Segment::Text("晚安👩‍💻[未提供图片]".into())]
        );
    }

    #[test]
    fn text_fallback_is_only_used_after_failure_not_while_waiting_for_images() {
        let mut cache = Cache::default();
        let url = "https://i0.hdslb.com/test.png";
        assert!(matches!(cache.state(url), ImageState::Loading));
        cache.entries.insert(
            url.into(),
            Entry {
                image: None,
                used: 0,
            },
        );
        assert!(matches!(cache.state(url), ImageState::Failed));
        let image = Arc::new(Image::from_bytes(ImageFormat::Png, Vec::new()));
        cache.entries.insert(
            url.into(),
            Entry {
                image: Some(image.clone()),
                used: 1,
            },
        );
        let ImageState::Ready(cached) = cache.state(url) else {
            panic!("Image should be ready")
        };
        assert!(Arc::ptr_eq(&cached, &image));
    }

    #[test]
    fn keeps_unicode_and_names_while_replacing_repeated_inline_emotes() {
        let emote = Emote {
            text: "[doge]".into(),
            url: "https://i0.hdslb.com/doge.png".into(),
            width: 1,
            height: 1,
        };
        let parts = segments("viewer[doge]", "你好😀[doge][doge]尾巴👩‍💻", &[emote.clone()]);
        assert_eq!(
            parts,
            vec![
                Segment::Text("viewer[doge]：".into()),
                Segment::Text("你好😀".into()),
                Segment::Emote(emote.clone()),
                Segment::Emote(emote),
                Segment::Text("尾巴👩‍💻".into())
            ]
        );
    }

    #[test]
    fn unknown_labels_and_empty_metadata_stay_readable() {
        assert!(segments("viewer", "😀[unknown]", &[]).is_empty());
        let emote = Emote {
            text: "".into(),
            url: "".into(),
            width: 1,
            height: 1,
        };
        assert_eq!(
            segments("", "😀[unknown]", &[emote]),
            vec![Segment::Text("😀[unknown]".into())]
        );
    }
}
