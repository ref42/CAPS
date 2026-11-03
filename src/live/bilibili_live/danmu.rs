//! Bilibili's 16-byte, big-endian packet framing, including nested compressed packets.
use serde_json::Value;
use std::io::Read;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Emote {
    pub text: String,
    pub url: String,
    pub width: u32,
    pub height: u32,
}

/// Only Bilibili's public image CDN is used for message-supplied images.
pub fn image_url(raw: &str) -> Option<String> {
    let raw = if raw.starts_with("//") {
        format!("https:{raw}")
    } else {
        raw.to_owned()
    };
    let mut url = reqwest::Url::parse(&raw).ok()?;
    let host = url.host_str()?;
    if !matches!(url.scheme(), "http" | "https")
        || !(host == "hdslb.com" || host.ends_with(".hdslb.com"))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return None;
    }
    url.set_scheme("https").ok()?;
    Some(url.into())
}

fn json_object(value: &Value) -> Value {
    if let Some(text) = value.as_str() {
        serde_json::from_str(text).unwrap_or(Value::Null)
    } else {
        value.clone()
    }
}

fn emote(text: &str, data: &Value) -> Option<Emote> {
    if text.is_empty() {
        return None;
    }
    Some(Emote {
        text: text.into(),
        url: image_url(data["url"].as_str()?)?,
        width: data["width"].as_u64().unwrap_or(1).clamp(1, 4096) as u32,
        height: data["height"].as_u64().unwrap_or(1).clamp(1, 4096) as u32,
    })
}

fn emotes(command: &Value, text: &str) -> Vec<Emote> {
    let options = json_object(&command["info"][0][13]);
    if let Some(sticker) = emote(text, &options) {
        return vec![sticker];
    }
    let extra = json_object(&command["info"][0][15]["extra"]);
    extra["emots"]
        .as_object()
        .map(|map| {
            map.iter()
                .filter(|(label, _)| text.contains(label.as_str()))
                .filter_map(|(label, data)| emote(label, data))
                .take(32)
                .collect()
        })
        .unwrap_or_default()
}

pub const LIMIT: usize = 8 * 1024 * 1024;

#[derive(Debug, PartialEq)]
pub enum Event {
    Auth(i64),
    Heartbeat(u32),
    Command(Value),
}

pub fn packet(operation: u32, body: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16 + body.len());
    bytes.extend_from_slice(&((16 + body.len()) as u32).to_be_bytes());
    bytes.extend_from_slice(&16u16.to_be_bytes());
    bytes.extend_from_slice(&1u16.to_be_bytes());
    bytes.extend_from_slice(&operation.to_be_bytes());
    bytes.extend_from_slice(&1u32.to_be_bytes());
    bytes.extend_from_slice(body);
    bytes
}

pub fn decode(bytes: &[u8]) -> Result<Vec<Event>, String> {
    let mut events = Vec::new();
    let mut budget = LIMIT;
    decode_inner(bytes, 0, &mut budget, &mut events)?;
    Ok(events)
}

fn decode_inner(
    mut bytes: &[u8],
    depth: usize,
    budget: &mut usize,
    events: &mut Vec<Event>,
) -> Result<(), String> {
    if depth > 4 || bytes.len() > *budget {
        return Err("Danmu packet exceeds decoding limits".into());
    }
    *budget -= bytes.len();
    while !bytes.is_empty() {
        if bytes.len() < 16 {
            return Err("Truncated danmu header".into());
        }
        let length = u32::from_be_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let header = u16::from_be_bytes(bytes[4..6].try_into().unwrap()) as usize;
        let version = u16::from_be_bytes(bytes[6..8].try_into().unwrap());
        let operation = u32::from_be_bytes(bytes[8..12].try_into().unwrap());
        if header < 16 || length < header || length > bytes.len() {
            return Err("Invalid danmu packet length".into());
        }
        let body = &bytes[header..length];
        match (operation, version) {
            (5, 2 | 3) => {
                let reader: Box<dyn Read + '_> = if version == 2 {
                    Box::new(flate2::read::ZlibDecoder::new(body))
                } else {
                    Box::new(brotli::Decompressor::new(body, 4096))
                };
                let mut expanded = Vec::new();
                reader
                    .take(*budget as u64 + 1)
                    .read_to_end(&mut expanded)
                    .map_err(|err| format!("Invalid compressed danmu: {err}"))?;
                decode_inner(&expanded, depth + 1, budget, events)?;
            }
            (5, 0 | 1) if !body.is_empty() => {
                events.push(Event::Command(
                    serde_json::from_slice(body)
                        .map_err(|err| format!("Invalid danmu JSON: {err}"))?,
                ));
            }
            (8, _) => {
                let auth: Value = serde_json::from_slice(body).map_err(|err| err.to_string())?;
                let code = auth["code"]
                    .as_i64()
                    .ok_or("Missing authentication result")?;
                events.push(Event::Auth(code));
            }
            (3, _) if body.len() >= 4 => {
                events.push(Event::Heartbeat(u32::from_be_bytes(
                    body[..4].try_into().unwrap(),
                )));
            }
            (5, _) => return Err(format!("Unsupported danmu protocol version {version}")),
            _ => {}
        }
        bytes = &bytes[length..];
    }
    Ok(())
}

/// The web protocol can append a version suffix to DANMU_MSG.
pub fn chat(command: &Value, room: u64) -> Option<Value> {
    let kind = command["cmd"].as_str()?.split(':').next()?;
    let (text, user, uid, timestamp) = match kind {
        "DANMU_MSG" => (
            command.pointer("/info/1")?.as_str()?,
            command
                .pointer("/info/2/1")
                .and_then(Value::as_str)
                .unwrap_or(""),
            command.pointer("/info/2/0").cloned().unwrap_or(Value::Null),
            command.pointer("/info/0/4").cloned().unwrap_or(Value::Null),
        ),
        "SUPER_CHAT_MESSAGE" => (
            command.pointer("/data/message")?.as_str()?,
            command
                .pointer("/data/user_info/uname")
                .and_then(Value::as_str)
                .unwrap_or(""),
            command.pointer("/data/uid").cloned().unwrap_or(Value::Null),
            command
                .pointer("/data/start_time")
                .cloned()
                .unwrap_or(Value::Null),
        ),
        _ => return None,
    };
    Some(
        serde_json::json!({"room_id": room, "kind": kind, "text": text,
        "user": user, "uid": uid, "timestamp": timestamp,
        "emotes": if kind == "DANMU_MSG" { emotes(command, text) } else { Vec::new() }}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn message(text: &str) -> Value {
        let mut meta = vec![Value::Null; 16];
        meta[4] = 123.into();
        serde_json::json!({"cmd": "DANMU_MSG", "info": [meta, text, [42, "viewer😀"]]})
    }

    #[test]
    fn extracts_standalone_sticker_object_or_json_string() {
        // Shape/URL from blivedm's documented web DanmakuMessage fixture.
        let sticker = serde_json::json!({"url": "https://i0.hdslb.com/bfs/live/a98e35996545509188fe4d24bd1a56518ea5af48.png",
            "width": 183, "height": 60, "is_dynamic": 1});
        for data in [sticker.clone(), Value::String(sticker.to_string())] {
            let mut command = message("[表情]");
            command["info"][0][13] = data;
            let result = chat(&command, 1).unwrap();
            assert_eq!(result["text"], "[表情]");
            assert_eq!(result["emotes"][0]["url"], sticker["url"]);
            assert_eq!(result["emotes"][0]["width"], 183);
        }
    }

    #[test]
    fn extracts_inline_emotes_without_changing_unicode_or_raw_request_text() {
        let extra = serde_json::json!({"emots": {"[doge]": {
            "url": "//i0.hdslb.com/bfs/emote/doge.png", "width": 60, "height": 60},
            "[unused]": {"url": "https://i0.hdslb.com/unused.png"}}});
        for data in [extra.clone(), Value::String(extra.to_string())] {
            let mut command = message("点歌 晴天 😀[doge][doge]");
            command["info"][0][15] = serde_json::json!({"extra": data});
            let result = chat(&command, 1).unwrap();
            assert_eq!(result["text"], command["info"][1]);
            assert_eq!(result["user"], "viewer😀");
            assert_eq!(result["emotes"].as_array().unwrap().len(), 1);
            assert_eq!(result["emotes"][0]["text"], "[doge]");
            assert!(
                result["emotes"][0]["url"]
                    .as_str()
                    .unwrap()
                    .starts_with("https://")
            );
        }
    }

    #[test]
    fn malformed_or_untrusted_images_leave_plain_text_intact() {
        for url in [
            "file:///C:/test.png",
            "https://hdslb.com.evil.example/a.png",
            "http://127.0.0.1/a",
            "https://user@i0.hdslb.com/a",
            "https://i0.hdslb.com:8443/a",
        ] {
            assert!(image_url(url).is_none());
        }
        let mut command = message("😀[unknown]");
        command["info"][0][13] = "invalid json".into();
        command["info"][0][15] = serde_json::json!({"extra": "invalid json"});
        let result = chat(&command, 1).unwrap();
        assert_eq!(result["text"], "😀[unknown]");
        assert!(result["emotes"].as_array().unwrap().is_empty());
    }

    #[test]
    fn decodes_concatenated_auth_heartbeat_and_chat() {
        let mut data = packet(8, br#"{"code":0}"#);
        data.extend(packet(3, &42u32.to_be_bytes()));
        data.extend(packet(5, br#"{"cmd":"DANMU_MSG","info":[]}"#));
        let events = decode(&data).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0], Event::Auth(0));
        assert_eq!(events[1], Event::Heartbeat(42));
    }

    #[test]
    fn decodes_zlib_and_brotli_batches() {
        let mut inner = packet(5, br#"{"cmd":"ONE"}"#);
        inner.extend(packet(5, br#"{"cmd":"TWO"}"#));
        let mut zlib = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        zlib.write_all(&inner).unwrap();
        let mut brotli = Vec::new();
        {
            let mut writer = brotli::CompressorWriter::new(&mut brotli, 4096, 4, 20);
            writer.write_all(&inner).unwrap();
        }
        for (version, body) in [(2u16, zlib.finish().unwrap()), (3, brotli)] {
            let mut outer = packet(5, &body);
            outer[6..8].copy_from_slice(&version.to_be_bytes());
            assert_eq!(decode(&outer).unwrap(), decode(&inner).unwrap());
        }
    }

    #[test]
    fn rejects_truncation_invalid_lengths_and_expansion_bombs() {
        assert!(decode(&[0; 15]).is_err());
        let mut data = packet(5, b"{}");
        data[0..4].copy_from_slice(&0u32.to_be_bytes());
        assert!(decode(&data).is_err());
        let mut data = packet(5, b"{}");
        data[4..6].copy_from_slice(&15u16.to_be_bytes());
        assert!(decode(&data).is_err());
        let mut zlib = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        zlib.write_all(&vec![0; LIMIT + 1]).unwrap();
        let mut data = packet(5, &zlib.finish().unwrap());
        data[6..8].copy_from_slice(&2u16.to_be_bytes());
        assert!(decode(&data).is_err());
    }

    #[test]
    fn extracts_text_without_requiring_unmasked_identity() {
        let command = serde_json::json!({"cmd":"DANMU_MSG:4:0:2:2:2:0",
            "info":[[0,1,25,0,123], "点歌 晴天 周杰伦", [0,"用**"]]});
        let result = chat(&command, 1234).unwrap();
        assert_eq!(result["text"], "点歌 晴天 周杰伦");
        assert_eq!(result["uid"], 0);
        assert!(chat(&serde_json::json!({"cmd":"WATCHED_CHANGE"}), 1234).is_none());
        assert!(chat(&serde_json::json!({"cmd":"DANMU_MSG"}), 1234).is_none());
    }
}
