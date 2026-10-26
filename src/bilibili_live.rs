//! Anonymous Bilibili live connection shared by CAPS and the CLI example.
#[path = "bilibili_live/protocol.rs"]
pub mod danmu;
use futures_util::{SinkExt, StreamExt};
use md5::{Digest, Md5};
use serde_json::{Value, json};
use std::{
    error::Error,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{Instant, timeout},
};
use tokio_tungstenite::{
    client_async_tls_with_config,
    tungstenite::{Message, client::IntoClientRequest, protocol::WebSocketConfig},
};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";
const API: &str = "https://api.live.bilibili.com";
const WBI_ORDER: [usize; 32] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13,
];

pub enum LiveEvent {
    Status(String),
    Connected(u64),
    Heartbeat,
    Command(Value),
}
struct Options {
    room: u64,
    proxy: Option<String>,
}
#[derive(Default)]
struct Stats {
    heartbeats: usize,
}

pub fn validate_proxy(value: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(value)?;
    if url.scheme() != "http"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Use an HTTP proxy URL without credentials, such as http://127.0.0.1:10808".into(),
        );
    }
    Ok(url)
}

async fn get(client: &reqwest::Client, url: &str, cookie: &str) -> Result<Value> {
    Ok(client
        .get(url)
        .header("Referer", "https://live.bilibili.com/")
        .header("Cookie", cookie)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

fn api_data(value: &Value) -> Result<&Value> {
    if value["code"].as_i64() != Some(0) {
        return Err(format!("Bilibili API code {}: {}", value["code"], value["message"]).into());
    }
    value
        .get("data")
        .ok_or_else(|| "Bilibili response has no data".into())
}

fn signed_query(room: u64, timestamp: u64, image: &str, sub: &str) -> Result<String> {
    let key_part = |url: &str| {
        url.rsplit('/')
            .next()
            .unwrap_or("")
            .split('.')
            .next()
            .unwrap_or("")
            .to_owned()
    };
    let key = key_part(image) + &key_part(sub);
    if key.len() != 64 || !key.is_ascii() {
        return Err("Invalid WBI key".into());
    }
    let mix: String = WBI_ORDER
        .iter()
        .map(|index| key.as_bytes()[*index] as char)
        .collect();
    // All values here are decimal integers; their sorted query needs no escaping.
    let query = format!("id={room}&type=0&wts={timestamp}");
    let digest = Md5::digest(format!("{query}{mix}").as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(format!("{query}&w_rid={hex}"))
}

pub async fn listen(
    room: u64,
    proxy: Option<String>,
    mut emit: impl FnMut(LiveEvent) + Send,
) -> Result<()> {
    let options = Options { room, proxy };
    let mut builder = reqwest::Client::builder()
        .user_agent(UA)
        .no_proxy()
        .timeout(Duration::from_secs(20));
    if let Some(proxy) = &options.proxy {
        validate_proxy(proxy)?;
        builder = builder.proxy(reqwest::Proxy::all(proxy)?);
    }
    let client = builder.build()?;
    let mut stats = Stats::default();
    let mut attempts = 0u32;
    loop {
        let previous_heartbeats = stats.heartbeats;
        let result = session(&client, &options, &mut stats, attempts as usize, &mut emit).await;
        if stats.heartbeats > previous_heartbeats {
            attempts = 0;
        }
        let delay = (2u64 << attempts.min(4)).min(30);
        let reason = result
            .err()
            .map(|err| err.to_string())
            .unwrap_or_else(|| "Disconnected".into());
        emit(LiveEvent::Status(format!(
            "{reason}. Reconnecting in {delay}s"
        )));
        tokio::time::sleep(Duration::from_secs(delay)).await;
        attempts = attempts.saturating_add(1);
    }
}

async fn session(
    client: &reqwest::Client,
    options: &Options,
    stats: &mut Stats,
    attempt: usize,
    emit: &mut (impl FnMut(LiveEvent) + Send),
) -> Result<()> {
    // Room resolution and the anonymous fingerprint are independent. Do not add
    // their round-trip times together when joining a new room.
    let init_url = format!("{API}/room/v1/Room/room_init?id={}", options.room);
    let (init, fingerprint) = tokio::try_join!(
        get(client, &init_url, ""),
        get(client, "https://api.bilibili.com/x/frontend/finger/spi", ""),
    )?;
    let data = api_data(&init)?;
    let room = data["room_id"].as_u64().ok_or("Missing room ID")?;
    emit(LiveEvent::Status(format!("Connecting to room {room}…")));
    let buvid = api_data(&fingerprint)?["b_3"]
        .as_str()
        .ok_or("Missing anonymous buvid")?;
    let cookie = format!("buvid3={buvid}");
    let nav = get(
        client,
        "https://api.bilibili.com/x/web-interface/nav",
        &cookie,
    )
    .await?;
    // Anonymous nav returns -101 but still supplies public WBI signing keys.
    let image = nav
        .pointer("/data/wbi_img/img_url")
        .and_then(Value::as_str)
        .ok_or("Missing WBI image key")?;
    let sub = nav
        .pointer("/data/wbi_img/sub_url")
        .and_then(Value::as_str)
        .ok_or("Missing WBI sub key")?;
    let query = signed_query(
        room,
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        image,
        sub,
    )?;
    let info = get(
        client,
        &format!("{API}/xlive/web-room/v1/index/getDanmuInfo?{query}"),
        &cookie,
    )
    .await?;
    let data = api_data(&info)?;
    let token = data["token"]
        .as_str()
        .filter(|token| !token.is_empty())
        .ok_or("Missing danmu token")?;
    let hosts = data["host_list"]
        .as_array()
        .filter(|hosts| !hosts.is_empty())
        .ok_or("Missing danmu hosts")?;
    let server = &hosts[attempt % hosts.len()];
    let host = server["host"].as_str().ok_or("Missing danmu host")?;
    if !host.ends_with(".chat.bilibili.com")
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
    {
        return Err("Unexpected danmu host".into());
    }
    let port = server["wss_port"]
        .as_u64()
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port != 0)
        .ok_or("Invalid WSS port")?;
    let url = format!("wss://{host}:{port}/sub");

    let mut request = url.into_client_request()?;
    request.headers_mut().insert("User-Agent", UA.parse()?);
    request
        .headers_mut()
        .insert("Origin", "https://live.bilibili.com".parse()?);
    let config = WebSocketConfig::default()
        .max_message_size(Some(danmu::LIMIT))
        .max_frame_size(Some(danmu::LIMIT));
    let (mut socket, _) = timeout(Duration::from_secs(20), async {
        let stream = transport(host, port, options.proxy.as_deref()).await?;
        Ok::<_, Box<dyn Error + Send + Sync>>(
            client_async_tls_with_config(request, stream, Some(config), None).await?,
        )
    })
    .await??;
    let auth = json!({"uid":0,"roomid":room,"protover":3,"platform":"web","type":2,"buvid":buvid,"key":token});
    socket
        .send(Message::Binary(
            danmu::packet(7, &serde_json::to_vec(&auth)?).into(),
        ))
        .await?;
    let mut authenticated = false;
    let mut last_heartbeat = Instant::now();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    heartbeat.tick().await;
    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(last_heartbeat + Duration::from_secs(if authenticated {75} else {15})) => {
                return Err("Authentication or heartbeat timed out".into());
            }
            _ = heartbeat.tick(), if authenticated => {
                socket.send(Message::Binary(danmu::packet(2, b"{}").into())).await?;
            }
            message = socket.next() => {
                match message.ok_or("WebSocket stream ended")?? {
                    Message::Binary(bytes) => {
                        for event in danmu::decode(&bytes)? {
                            match event {
                                danmu::Event::Auth(0) => {
                                    authenticated = true;
                                    emit(LiveEvent::Connected(room));
                                    socket.send(Message::Binary(danmu::packet(2, b"{}").into())).await?;
                                }
                                danmu::Event::Auth(code) => return Err(format!("Danmu authentication rejected: {code}").into()),
                                danmu::Event::Heartbeat(_) => {
                                    last_heartbeat = Instant::now();
                                    stats.heartbeats += 1;
                                    emit(LiveEvent::Heartbeat);
                                }
                                danmu::Event::Command(command) => {
                                    emit(LiveEvent::Command(command));
                                }
                            }
                        }
                    }
                    Message::Ping(bytes) => socket.send(Message::Pong(bytes)).await?,
                    Message::Close(_) => return Err("Server closed the WebSocket".into()),
                    _ => {}
                }
            }
        }
    }
}

async fn transport(host: &str, port: u16, proxy: Option<&str>) -> Result<TcpStream> {
    let Some(proxy) = proxy else {
        return Ok(TcpStream::connect((host, port)).await?);
    };
    let proxy = validate_proxy(proxy)?;
    let mut stream = TcpStream::connect((
        proxy.host_str().ok_or("Missing proxy host")?,
        proxy.port().unwrap_or(80),
    ))
    .await?;
    stream
        .write_all(
            format!("CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n\r\n").as_bytes(),
        )
        .await?;
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        if header.len() >= 8192 {
            return Err("Proxy response header too large".into());
        }
        header.push(stream.read_u8().await?);
    }
    if std::str::from_utf8(&header)?
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        != Some("200")
    {
        return Err("HTTP proxy refused the WebSocket tunnel".into());
    }
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signing_matches_independent_fixture_and_validates_keys() {
        let image = "https://example.com/7cd084941338484aae1ad9425b84077c.png";
        let sub = "https://example.com/4932caff0ff746eab6f01bf08b70ac45.png";
        let query = signed_query(1747113738, 1700000000, image, sub).unwrap();
        assert_eq!(
            query,
            "id=1747113738&type=0&wts=1700000000&w_rid=819f724963591d19954cccf6b3c658f6"
        );
        assert!(signed_query(1, 1, "bad", "bad").is_err());
    }
}
