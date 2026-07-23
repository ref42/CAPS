use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq)]
pub struct MessageNotification {
    pub key: String,
    pub app: String,
    pub mark: String,
    pub title: String,
    pub body: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationAccess {
    Allowed,
    Denied,
    Unavailable,
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{MessageNotification, NotificationAccess};
    use std::collections::HashSet;
    use windows::UI::Notifications::Management::{
        UserNotificationListener, UserNotificationListenerAccessStatus,
    };
    use windows::UI::Notifications::{NotificationKinds, UserNotification};

    pub async fn request_access() -> NotificationAccess {
        let Ok(listener) = UserNotificationListener::Current() else {
            return NotificationAccess::Unavailable;
        };
        let status = listener
            .GetAccessStatus()
            .unwrap_or(UserNotificationListenerAccessStatus::Unspecified);
        let status = if status == UserNotificationListenerAccessStatus::Unspecified {
            match listener.RequestAccessAsync() {
                Ok(operation) => operation
                    .get()
                    .unwrap_or(UserNotificationListenerAccessStatus::Denied),
                Err(_) => UserNotificationListenerAccessStatus::Denied,
            }
        } else {
            status
        };
        if status == UserNotificationListenerAccessStatus::Allowed {
            NotificationAccess::Allowed
        } else if status == UserNotificationListenerAccessStatus::Denied {
            NotificationAccess::Denied
        } else {
            NotificationAccess::Unavailable
        }
    }

    pub async fn collect_messages(seen: &mut HashSet<String>) -> Vec<MessageNotification> {
        let Ok(listener) = UserNotificationListener::Current() else {
            return Vec::new();
        };
        let Ok(operation) = listener.GetNotificationsAsync(NotificationKinds::Toast) else {
            return Vec::new();
        };
        let Ok(notifications) = operation.get() else {
            return Vec::new();
        };
        let Ok(size) = notifications.Size() else {
            return Vec::new();
        };
        let mut messages = Vec::new();
        for index in 0..size {
            let Ok(notification) = notifications.GetAt(index) else {
                continue;
            };
            let Some(message) = message_from_notification(notification, seen) else {
                continue;
            };
            messages.push(message);
        }
        messages
    }

    fn message_from_notification(
        notification: UserNotification,
        seen: &mut HashSet<String>,
    ) -> Option<MessageNotification> {
        let app_info = notification.AppInfo().ok()?;
        let app_id = app_info
            .AppUserModelId()
            .ok()
            .map(|value| value.to_string())
            .or_else(|| app_info.Id().ok().map(|value| value.to_string()))
            .unwrap_or_default();
        let app_name = app_info
            .DisplayInfo()
            .ok()
            .and_then(|info| info.DisplayName().ok())
            .map(|value| value.to_string())
            .unwrap_or_else(|| app_id.clone());
        let source = normalize_source(&app_name, &app_id)?;
        let id = notification.Id().unwrap_or_default();
        let key = format!("{app_id}:{id}");
        if !seen.insert(key.clone()) {
            return None;
        }
        let texts = notification_texts(&notification);
        let sender = texts
            .iter()
            .find(|text| likely_sender(text, &source.app))
            .cloned()
            .unwrap_or_default();
        let body = if sender.is_empty() {
            "New message".to_string()
        } else {
            format!("{sender} sent a message")
        };
        Some(MessageNotification {
            key,
            app: source.app,
            mark: source.mark,
            title: "Message".to_string(),
            body,
        })
    }

    fn notification_texts(notification: &UserNotification) -> Vec<String> {
        let Ok(notification) = notification.Notification() else {
            return Vec::new();
        };
        let Ok(visual) = notification.Visual() else {
            return Vec::new();
        };
        let Ok(bindings) = visual.Bindings() else {
            return Vec::new();
        };
        let Ok(binding_count) = bindings.Size() else {
            return Vec::new();
        };
        let mut result = Vec::new();
        for binding_index in 0..binding_count {
            let Ok(binding) = bindings.GetAt(binding_index) else {
                continue;
            };
            let Ok(texts) = binding.GetTextElements() else {
                continue;
            };
            let Ok(text_count) = texts.Size() else {
                continue;
            };
            for text_index in 0..text_count {
                let Ok(text) = texts.GetAt(text_index) else {
                    continue;
                };
                let Ok(value) = text.Text() else {
                    continue;
                };
                let value = value.to_string();
                let value = value.trim();
                if !value.is_empty() {
                    result.push(value.to_string());
                }
            }
        }
        result
    }

    fn likely_sender(text: &str, app: &str) -> bool {
        let normalized = text.trim().to_lowercase();
        !normalized.is_empty()
            && !normalized.contains(&app.to_lowercase())
            && !normalized.contains("new message")
            && !normalized.contains("sent a message")
            && !normalized.contains("message")
            && normalized.chars().count() <= 32
    }

    fn normalize_source(app_name: &str, app_id: &str) -> Option<MessageSource> {
        let haystack = format!("{app_name} {app_id}").to_lowercase();
        if haystack.contains("wechat") || haystack.contains("weixin") || haystack.contains("微信")
        {
            Some(MessageSource {
                app: "WeChat".to_string(),
                mark: "W".to_string(),
            })
        } else if haystack.contains("qq") || haystack.contains("tim") {
            Some(MessageSource {
                app: "QQ".to_string(),
                mark: "Q".to_string(),
            })
        } else {
            None
        }
    }

    struct MessageSource {
        app: String,
        mark: String,
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::{MessageNotification, NotificationAccess};
    use std::collections::HashSet;

    pub async fn request_access() -> NotificationAccess {
        NotificationAccess::Unavailable
    }

    pub async fn collect_messages(_: &mut HashSet<String>) -> Vec<MessageNotification> {
        Vec::new()
    }
}

pub async fn request_access() -> NotificationAccess {
    platform::request_access().await
}

pub async fn collect_messages(seen: &mut HashSet<String>) -> Vec<MessageNotification> {
    platform::collect_messages(seen).await
}
