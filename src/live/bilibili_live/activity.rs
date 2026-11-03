//! Events for the chat history, including paid messages and gifts.
//! Web packet fields and the V2 protobuf tags are documented in:
//! https://github.com/xfgryujk/blivedm/blob/master/blivedm/models/web.py
//! https://github.com/xfgryujk/blivedm/blob/master/blivedm/models/pb.py
use super::danmu::{self, Emote};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use prost::Message as _;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Danmu,
    SuperChat {
        id: Option<String>,
        yuan: Option<u64>,
    },
    Gift {
        id: Option<String>,
        name: String,
        quantity: u64,
    },
}

impl Kind {
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Danmu => None,
            Self::SuperChat { id, .. } | Self::Gift { id, .. } => id.as_deref(),
        }
    }

    pub fn allows_song_request(&self) -> bool {
        !matches!(self, Self::Gift { .. })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Author {
    pub uid: u64,
    pub avatar: Option<String>,
    pub medal: Option<(String, u64)>,
    pub guard: u64,
}

fn author(command: &Value) -> Author {
    let info = &command["info"];
    let data = &command["data"];
    let avatar = info
        .pointer("/0/15/user/base/face")
        .and_then(Value::as_str)
        .or_else(|| data.pointer("/user_info/face").and_then(Value::as_str))
        .or_else(|| data["face"].as_str())
        .and_then(danmu::image_url);
    let medal_name = info.pointer("/3/1").and_then(Value::as_str).or_else(|| {
        data.pointer("/medal_info/medal_name")
            .and_then(Value::as_str)
    });
    let level = info
        .pointer("/3/0")
        .and_then(number)
        .or_else(|| number(&data["medal_info"]["medal_level"]))
        .unwrap_or(0);
    Author {
        uid: info
            .pointer("/2/0")
            .and_then(number)
            .or_else(|| number(&data["uid"]))
            .unwrap_or(0),
        avatar,
        medal: medal_name
            .filter(|name| !name.is_empty())
            .map(|name| (name.chars().take(16).collect(), level)),
        guard: info
            .get(7)
            .and_then(number)
            .or_else(|| number(&data["guard_level"]))
            .or_else(|| number(&data["user_info"]["guard_level"]))
            .unwrap_or(0),
    }
}

pub struct Message {
    pub user: String,
    pub text: String,
    pub emotes: Vec<Emote>,
    pub kind: Kind,
    pub author: Author,
}

pub enum Event {
    Message(Message),
    DeleteSuperChats(Vec<String>),
}

fn number(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_str()?.parse().ok())
}

fn id(value: &Value) -> Option<String> {
    let value = value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|n| n.to_string()))?;
    (!value.is_empty()).then_some(value)
}

fn bounded(text: &str) -> String {
    text.chars().take(500).collect()
}

fn gift(
    user: &str,
    name: &str,
    quantity: u64,
    transaction: Option<String>,
    gift_id: u64,
    image: Option<&str>,
) -> Option<Event> {
    if name.is_empty() || quantity == 0 {
        return None;
    }
    let name = bounded(name);
    let mut text = format!("{name} ×{quantity}");
    let mut emotes = Vec::new();
    if let Some(url) = image.and_then(danmu::image_url) {
        // This placeholder belongs to a validated gift image, never user text.
        text.insert_str(0, "\u{fffc} ");
        emotes.push(Emote {
            text: "\u{fffc}".into(),
            url,
            width: 32,
            height: 32,
        });
    }
    Some(Event::Message(Message {
        user: bounded(user),
        text,
        emotes,
        author: Author::default(),
        kind: Kind::Gift {
            id: transaction.map(|id| format!("gift:{id}:{gift_id}")),
            name,
            quantity,
        },
    }))
}

/// Ignore combo summaries and translated SC copies: their underlying events
/// already appear in this history. Stable IDs also suppress retransmissions.
pub fn events(command: &Value, room: u64) -> Vec<Event> {
    let kind = command["cmd"]
        .as_str()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    let data = &command["data"];
    match kind {
        "DANMU_MSG" | "SUPER_CHAT_MESSAGE" => danmu::chat(command, room)
            .map(|chat| {
                Event::Message(Message {
                    author: author(command),
                    user: bounded(chat["user"].as_str().unwrap_or("")),
                    text: bounded(chat["text"].as_str().unwrap_or("")),
                    emotes: serde_json::from_value(chat["emotes"].clone()).unwrap_or_default(),
                    kind: if kind == "SUPER_CHAT_MESSAGE" {
                        Kind::SuperChat {
                            id: id(&data["id"]).map(|id| format!("sc:{id}")),
                            yuan: number(&data["price"]),
                        }
                    } else {
                        Kind::Danmu
                    },
                })
            })
            .into_iter()
            .collect(),
        "SEND_GIFT" => gift(
            data["uname"].as_str().unwrap_or(""),
            data["giftName"].as_str().unwrap_or(""),
            number(&data["num"]).unwrap_or(1),
            id(&data["tid"]).or_else(|| id(&data["rnd"])),
            number(&data["giftId"]).unwrap_or(0),
            data["gift_info"]["img_basic"].as_str(),
        )
        .map(|mut event| {
            if let Event::Message(message) = &mut event {
                message.author = author(command);
            }
            event
        })
        .into_iter()
        .collect(),
        "SEND_GIFT_V2" => gifts_v2(data),
        "GUARD_BUY" => gift(
            data["username"].as_str().unwrap_or(""),
            data["gift_name"].as_str().unwrap_or(""),
            number(&data["num"]).unwrap_or(1),
            id(&data["start_time"])
                .zip(id(&data["uid"]))
                .map(|(time, uid)| format!("guard:{uid}:{time}")),
            number(&data["gift_id"]).unwrap_or(0),
            None,
        )
        .into_iter()
        .collect(),
        "SUPER_CHAT_MESSAGE_DELETE" => {
            let ids: Vec<_> = data["ids"]
                .as_array()
                .into_iter()
                .flatten()
                .take(200)
                .filter_map(id)
                .map(|id| format!("sc:{id}"))
                .collect();
            if ids.is_empty() {
                Vec::new()
            } else {
                vec![Event::DeleteSuperChats(ids)]
            }
        }
        _ => Vec::new(),
    }
}

// Partial protobuf definitions intentionally skip fields not displayed by CAPS.
#[derive(Clone, PartialEq, prost::Message)]
struct GiftBroadcast {
    #[prost(uint64, tag = "1")]
    uid: u64,
    #[prost(string, tag = "2")]
    user: String,
    #[prost(string, tag = "3")]
    avatar: String,
    #[prost(uint64, tag = "5")]
    guard: u64,
    #[prost(message, repeated, tag = "10")]
    gifts: Vec<GiftItem>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct GiftItem {
    #[prost(uint64, tag = "1")]
    gift_id: u64,
    #[prost(string, tag = "2")]
    name: String,
    #[prost(uint64, tag = "3")]
    quantity: u64,
    #[prost(string, tag = "9")]
    transaction: String,
    #[prost(string, tag = "12")]
    random_id: String,
    #[prost(message, optional, tag = "35")]
    material: Option<GiftMaterial>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct GiftMaterial {
    #[prost(string, tag = "1")]
    image: String,
}

fn gifts_v2(data: &Value) -> Vec<Event> {
    let Some(encoded) = data["pb"].as_str().filter(|text| text.len() <= 256 * 1024) else {
        return Vec::new();
    };
    let Ok(bytes) = STANDARD.decode(encoded) else {
        return Vec::new();
    };
    let Ok(packet) = GiftBroadcast::decode(bytes.as_slice()) else {
        return Vec::new();
    };
    packet
        .gifts
        .into_iter()
        .take(64)
        .filter_map(|item| {
            let transaction = if item.transaction.is_empty() {
                item.random_id
            } else {
                item.transaction
            };
            gift(
                &packet.user,
                &item.name,
                item.quantity,
                (!transaction.is_empty()).then_some(transaction),
                item.gift_id,
                item.material
                    .as_ref()
                    .map(|material| material.image.as_str()),
            )
            .map(|mut event| {
                if let Event::Message(message) = &mut event {
                    message.author.avatar = danmu::image_url(&packet.avatar);
                    message.author.uid = packet.uid;
                    message.author.guard = packet.guard;
                }
                event
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn message(command: Value) -> Message {
        let Event::Message(message) = events(&command, 1).remove(0) else {
            panic!("expected a message");
        };
        message
    }

    #[test]
    fn super_chat_preserves_amount_sender_text_and_deletion_id() {
        let chat = message(json!({"cmd":"SUPER_CHAT_MESSAGE", "data": {
            "id":42, "price":30, "message":"支持你😀", "user_info":{"uname":"观众"}}}));
        assert_eq!(chat.user, "观众");
        assert_eq!(chat.text, "支持你😀");
        assert_eq!(
            chat.kind,
            Kind::SuperChat {
                id: Some("sc:42".into()),
                yuan: Some(30)
            }
        );
        assert!(chat.kind.allows_song_request());
        let deleted = events(
            &json!({"cmd":"SUPER_CHAT_MESSAGE_DELETE", "data":{"ids":[42,"43"]}}),
            1,
        );
        assert!(matches!(&deleted[0], Event::DeleteSuperChats(ids) if ids == &["sc:42", "sc:43"]));
    }

    #[test]
    fn gifts_keep_quantity_and_icon_and_never_become_song_requests() {
        let chat = message(json!({"cmd":"SEND_GIFT", "data": {
            "uname":"观众", "giftName":"点歌 晴天", "num":3, "giftId":7, "tid":"abc",
            "gift_info":{"img_basic":"//i0.hdslb.com/gift.png"}}}));
        assert_eq!(chat.kind.id(), Some("gift:abc:7"));
        assert!(!chat.kind.allows_song_request());
        assert!(chat.text.ends_with("点歌 晴天 ×3"));
        assert_eq!(chat.emotes[0].url, "https://i0.hdslb.com/gift.png");
        assert!(events(&json!({"cmd":"COMBO_SEND", "data":{"combo_num":3}}), 1).is_empty());
        assert!(
            events(
                &json!({"cmd":"SUPER_CHAT_MESSAGE_JPN", "data":{"id":42}}),
                1
            )
            .is_empty()
        );
    }

    #[test]
    fn decodes_batched_v2_gifts_and_matches_legacy_transaction_keys() {
        // Wire fixture encoded independently from the structs: sender field 2,
        // repeated gift field 10; item fields 1=id, 2=name, 3=num, 9=tid.
        let bytes = [
            0x12, 3, b'A', b'n', b'n', 0x52, 14, 0x08, 7, 0x12, 4, b'R', b'o', b's', b'e', 0x18, 2,
            0x4a, 2, b'i', b'd', 0x52, 14, 0x08, 8, 0x12, 4, b'S', b't', b'a', b'r', 0x18, 5, 0x4a,
            2, b'i', b'd',
        ];
        let batch = events(
            &json!({"cmd":"SEND_GIFT_V2", "data":{"pb":STANDARD.encode(bytes)}}),
            1,
        );
        assert_eq!(batch.len(), 2);
        let Event::Message(first) = &batch[0] else {
            panic!();
        };
        assert_eq!(first.user, "Ann");
        assert_eq!(first.text, "Rose ×2");
        assert_eq!(first.kind.id(), Some("gift:id:7"));
        let Event::Message(second) = &batch[1] else {
            panic!();
        };
        assert_eq!(second.text, "Star ×5");
        assert_eq!(second.kind.id(), Some("gift:id:8"));
    }

    #[test]
    fn malformed_events_are_ignored_without_dropping_the_connection() {
        for pb in [
            "invalid!".to_owned(),
            STANDARD.encode([0x52, 100, 1]),
            "A".repeat(256 * 1024 + 1),
        ] {
            assert!(events(&json!({"cmd":"SEND_GIFT_V2", "data":{"pb":pb}}), 1).is_empty());
        }
        assert!(
            events(
                &json!({"cmd":"SEND_GIFT", "data":{"giftName":"Rose", "num":0}}),
                1
            )
            .is_empty()
        );
        assert!(events(&json!({"cmd":"SUPER_CHAT_MESSAGE", "data":{}}), 1).is_empty());
        let chat = message(json!({"cmd":"SUPER_CHAT_MESSAGE", "data":{"message":"hi"}}));
        assert!(matches!(chat.kind, Kind::SuperChat { yuan: None, .. }));
    }

    #[test]
    fn keeps_available_identity_and_fan_medal_without_guessing_missing_values() {
        let mut options = vec![Value::Null; 16];
        options[15] = json!({"user":{"base":{"face":"//i0.hdslb.com/avatar.png"}}});
        let chat = message(
            json!({"cmd":"DANMU_MSG","info":[options,"hi",[42,"观众"],[7,"粉丝牌"],[],[],[],3]}),
        );
        assert_eq!(chat.author.uid, 42);
        assert_eq!(
            chat.author.avatar.as_deref(),
            Some("https://i0.hdslb.com/avatar.png")
        );
        assert_eq!(chat.author.medal, Some(("粉丝牌".into(), 7)));
        assert_eq!(chat.author.guard, 3);
        assert_eq!(
            message(json!({"cmd":"DANMU_MSG","info":[[],"hi",[0,"用**"]]})).author,
            Author::default()
        );
    }
}
