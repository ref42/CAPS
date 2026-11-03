//! Native QR login and explicit, single-attempt sending. Session secrets never
//! enter settings JSON, UI strings, debug formatting, or diagnostic logs.
use super::danmu::image_url;
use gpui_kit::{Image, ImageFormat};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    sync::{Arc, LazyLock, RwLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{sync::mpsc, task::JoinHandle};

#[derive(Clone, Serialize, Deserialize)]
struct Credentials {
    session: String,
    csrf: String,
    uid: u64,
}
static SESSION: LazyLock<RwLock<Option<Credentials>>> = LazyLock::new(|| RwLock::new(None));

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub uid: u64,
    pub name: String,
    pub avatar: Option<String>,
}

pub enum State {
    SignedOut,
    Loading,
    Qr { image: Arc<Image>, scanned: bool },
    SignedIn(Profile),
    Failed(String),
    Expired,
}
pub enum SendState {
    Idle,
    Sending,
    Sent,
    Failed(String),
}
enum Notice {
    State(State),
    Authenticated(Profile, Credentials),
    Scanned,
    Send(SendState),
}

pub struct Account {
    pub state: State,
    pub send_state: SendState,
    pub identity_changed: bool,
    rx: mpsc::UnboundedReceiver<Notice>,
    tx: mpsc::UnboundedSender<Notice>,
    login: Option<JoinHandle<()>>,
    sending: Option<JoinHandle<()>>,
    last_send: Option<Instant>,
}

pub fn restore() {
    if let Some(credentials) = vault::load() {
        if let Ok(mut session) = SESSION.write() {
            *session = Some(credentials);
        }
    }
}

pub fn connection_identity() -> (u64, String) {
    current().map(|c| (c.uid, cookie(&c))).unwrap_or_default()
}
pub fn has_session() -> bool {
    SESSION.read().is_ok_and(|session| session.is_some())
}
fn current() -> Option<Credentials> {
    SESSION.read().ok()?.clone()
}
fn cookie(c: &Credentials) -> String {
    format!(
        "SESSDATA={}; bili_jct={}; DedeUserID={}",
        c.session, c.csrf, c.uid
    )
}

fn client(proxy: &str) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .user_agent(super::UA)
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy();
    if !proxy.is_empty() {
        super::validate_proxy(proxy).map_err(|_| "Invalid HTTP proxy".to_owned())?;
        builder = builder.proxy(reqwest::Proxy::all(proxy).map_err(|_| "Invalid HTTP proxy")?);
    }
    builder
        .build()
        .map_err(|_| "Could not start connection".into())
}
async fn json(response: Result<reqwest::Response, reqwest::Error>) -> Result<Value, String> {
    response
        .map_err(|_| "Could not reach Bilibili. Try again.")?
        .error_for_status()
        .map_err(|_| "Bilibili is unavailable. Try again.")?
        .json()
        .await
        .map_err(|_| "Invalid Bilibili response".into())
}
fn success(value: &Value) -> Result<&Value, String> {
    if value["code"].as_i64() != Some(0) {
        return Err("Bilibili rejected the request. Try signing in again.".into());
    }
    value
        .get("data")
        .ok_or_else(|| "Invalid Bilibili response".into())
}
async fn profile(client: &reqwest::Client, credentials: &Credentials) -> Result<Profile, String> {
    let value = json(
        client
            .get("https://api.bilibili.com/x/web-interface/nav")
            .header("Cookie", cookie(credentials))
            .header("Referer", "https://live.bilibili.com/")
            .send()
            .await,
    )
    .await?;
    let data = success(&value)?;
    if data["isLogin"] != true || data["mid"].as_u64() != Some(credentials.uid) {
        return Err("Session expired. Sign in again.".into());
    }
    Ok(Profile {
        uid: credentials.uid,
        name: data["uname"]
            .as_str()
            .unwrap_or("Bilibili")
            .chars()
            .take(80)
            .collect(),
        avatar: data["face"].as_str().and_then(image_url),
    })
}

fn credentials(headers: &reqwest::header::HeaderMap) -> Option<Credentials> {
    let mut session = None;
    let mut csrf = None;
    let mut uid = None;
    for header in headers.get_all(reqwest::header::SET_COOKIE) {
        let (name, value) = header.to_str().ok()?.split(';').next()?.split_once('=')?;
        if value.is_empty()
            || value.len() > 4096
            || !value
                .bytes()
                .all(|b| b.is_ascii_graphic() && b != b';' && b != b',')
        {
            continue;
        }
        match name.trim() {
            "SESSDATA" => session = Some(value.to_owned()),
            "bili_jct" => csrf = Some(value.to_owned()),
            "DedeUserID" => uid = value.parse().ok(),
            _ => {}
        }
    }
    let c = Credentials {
        session: session?,
        csrf: csrf?,
        uid: uid?,
    };
    (c.uid > 0 && c.csrf.len() == 32 && c.csrf.bytes().all(|b| b.is_ascii_hexdigit())).then_some(c)
}

fn qr_png(url: &str) -> Result<Arc<Image>, String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "Invalid sign-in QR code")?;
    if parsed.scheme() != "https"
        || !matches!(
            parsed.host_str(),
            Some("passport.bilibili.com" | "account.bilibili.com")
        )
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
    {
        return Err("Invalid sign-in QR code".into());
    }
    let qr = qrcode::QrCode::new(url).map_err(|_| "Could not create QR code")?;
    let width = qr.width();
    let scale = 6;
    let side = (width + 8) * scale;
    let mut pixels = image::GrayImage::from_pixel(side as u32, side as u32, image::Luma([255]));
    for y in 0..width {
        for x in 0..width {
            if qr[(x, y)] == qrcode::Color::Dark {
                for dy in 0..scale {
                    for dx in 0..scale {
                        pixels.put_pixel(
                            ((x + 4) * scale + dx) as u32,
                            ((y + 4) * scale + dy) as u32,
                            image::Luma([0]),
                        );
                    }
                }
            }
        }
    }
    let mut output = std::io::Cursor::new(Vec::new());
    pixels
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|_| "Could not draw QR code")?;
    Ok(Arc::new(Image::from_bytes(
        ImageFormat::Png,
        output.into_inner(),
    )))
}

async fn sign_in(proxy: String, tx: mpsc::UnboundedSender<Notice>) -> Result<(), String> {
    let client = client(&proxy)?;
    let value = json(
        client
            .get("https://passport.bilibili.com/x/passport-login/web/qrcode/generate")
            .send()
            .await,
    )
    .await?;
    let data = success(&value)?;
    let key = data["qrcode_key"]
        .as_str()
        .filter(|key| key.len() <= 128)
        .ok_or("Missing QR key")?;
    let image = qr_png(data["url"].as_str().ok_or("Missing QR code")?)?;
    let _ = tx.send(Notice::State(State::Qr {
        image,
        scanned: false,
    }));
    let deadline = Instant::now() + Duration::from_secs(180);
    while Instant::now() < deadline {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let response = client
            .get(format!(
                "https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={}",
                urlencoding::encode(key)
            ))
            .send()
            .await
            .map_err(|_| "Sign-in connection failed. Refresh the QR code.")?;
        let headers = response.headers().clone();
        let value = json(Ok(response)).await?;
        match success(&value)?["code"].as_i64() {
            Some(86101) => {}
            Some(86090) => {
                let _ = tx.send(Notice::Scanned);
            }
            Some(86038) => break,
            Some(0) => {
                let credentials = credentials(&headers)
                    .ok_or("Sign-in returned an incomplete session. Refresh the QR code.")?;
                let profile = profile(&client, &credentials).await?;
                // Installation is handled by the owner when it polls this notice.
                // Cancelling/closing discards it without leaving a signed-in account.
                let _ = tx.send(Notice::Authenticated(profile, credentials));
                return Ok(());
            }
            _ => return Err("Sign-in failed. Refresh the QR code.".into()),
        }
    }
    let _ = tx.send(Notice::State(State::Expired));
    Ok(())
}

pub fn send_fields(
    room: u64,
    message: &str,
    csrf: &str,
    timestamp: u64,
) -> Result<Vec<(&'static str, String)>, String> {
    let message = message.trim();
    if room == 0 {
        return Err("Connect to a live room first.".into());
    }
    if message.is_empty() || message.chars().count() > 200 {
        return Err("Use between 1 and 200 characters. The room may have a lower limit.".into());
    }
    Ok(vec![
        ("roomid", room.to_string()),
        ("msg", message.into()),
        ("csrf", csrf.into()),
        ("csrf_token", csrf.into()),
        ("rnd", timestamp.to_string()),
        ("fontsize", "25".into()),
        ("color", "16777215".into()),
        ("mode", "1".into()),
        ("bubble", "0".into()),
    ])
}
async fn send_message(proxy: String, room: u64, message: String) -> Result<(), String> {
    let credentials = current().ok_or("Sign in to send a message.")?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Check the system clock.")?
        .as_secs();
    let fields = send_fields(room, &message, &credentials.csrf, timestamp)?;
    let response = json(
        client(&proxy)?
            .post("https://api.live.bilibili.com/msg/send")
            .header("Cookie", cookie(&credentials))
            .header("Referer", format!("https://live.bilibili.com/{room}"))
            .form(&fields)
            .send()
            .await,
    )
    .await
    .map_err(|_| "Delivery could not be confirmed. Check chat before retrying.".to_owned())?;
    send_result(&response)
}

fn send_result(response: &Value) -> Result<(), String> {
    let reason = [
        response["message"].as_str(),
        response["msg"].as_str(),
        response["data"]["msg"].as_str(),
    ]
    .into_iter()
    .flatten()
    .find(|message| !message.is_empty() && *message != "0" && *message != "success");
    if response["code"].as_i64() != Some(0) || reason.is_some() {
        return Err(reason
            .map(|message| message.chars().take(200).collect())
            .unwrap_or_else(|| {
                "Message rejected by Bilibili. Check sign-in and room permissions.".into()
            }));
    }
    Ok(())
}

impl Account {
    pub fn new(proxy: &str) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut account = Self {
            state: State::SignedOut,
            send_state: SendState::Idle,
            identity_changed: false,
            tx,
            rx,
            login: None,
            sending: None,
            last_send: None,
        };
        if let Some(credentials) = current() {
            account.state = State::Loading;
            let tx = account.tx.clone();
            let proxy = proxy.to_owned();
            account.login = Some(tokio::spawn(async move {
                let result = match client(&proxy) {
                    Ok(client) => profile(&client, &credentials).await,
                    Err(error) => Err(error),
                };
                let _ = tx.send(Notice::State(match result {
                    Ok(profile) => State::SignedIn(profile),
                    Err(error) => State::Failed(error),
                }));
            }));
        }
        account
    }
    pub fn begin(&mut self, proxy: &str) {
        if has_session()
            && let Err(error) = self.logout()
        {
            self.state = State::Failed(error);
            return;
        }
        self.cancel();
        self.state = State::Loading;
        let tx = self.tx.clone();
        let proxy = proxy.to_owned();
        self.login = Some(tokio::spawn(async move {
            if let Err(error) = sign_in(proxy, tx.clone()).await {
                let _ = tx.send(Notice::State(State::Failed(error)));
            }
        }));
    }
    pub fn cancel(&mut self) {
        if let Some(job) = self.login.take() {
            job.abort();
        }
        while self.rx.try_recv().is_ok() {}
        self.state = State::SignedOut;
    }
    pub fn logout(&mut self) -> Result<(), String> {
        // Removal must succeed before reporting that the remembered session is gone.
        vault::clear()?;
        self.cancel();
        if let Some(job) = self.sending.take() {
            job.abort();
        }
        if let Ok(mut session) = SESSION.write() {
            *session = None;
        }
        self.send_state = SendState::Idle;
        self.identity_changed = true;
        Ok(())
    }
    pub fn send(&mut self, proxy: &str, room: u64, message: String) {
        if matches!(self.send_state, SendState::Sending) {
            return;
        }
        if self
            .last_send
            .is_some_and(|last| last.elapsed() < Duration::from_secs(2))
        {
            self.send_state =
                SendState::Failed("Please wait a moment before sending again.".into());
            return;
        }
        self.last_send = Some(Instant::now());
        self.send_state = SendState::Sending;
        let tx = self.tx.clone();
        let proxy = proxy.to_owned();
        self.sending = Some(tokio::spawn(async move {
            let result = send_message(proxy, room, message).await;
            let _ = tx.send(Notice::Send(match result {
                Ok(()) => SendState::Sent,
                Err(error) => SendState::Failed(error),
            }));
        }));
    }
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(notice) = self.rx.try_recv() {
            changed = true;
            match notice {
                Notice::Authenticated(profile, credentials) => match vault::save(&credentials) {
                    Ok(()) => {
                        if let Ok(mut session) = SESSION.write() {
                            *session = Some(credentials);
                        }
                        self.state = State::SignedIn(profile);
                        self.identity_changed = true;
                    }
                    Err(error) => self.state = State::Failed(error),
                },
                Notice::State(state) => {
                    self.identity_changed |= matches!(state, State::SignedIn(_));
                    self.state = state;
                }
                Notice::Scanned => {
                    if let State::Qr { scanned, .. } = &mut self.state {
                        *scanned = true;
                    }
                }
                Notice::Send(state) => self.send_state = state,
            }
        }
        changed
    }
}
impl Drop for Account {
    fn drop(&mut self) {
        if let Some(job) = self.login.take() {
            job.abort();
        }
        if let Some(job) = self.sending.take() {
            job.abort();
        }
    }
}

#[cfg(target_os = "windows")]
#[path = "account_vault.rs"]
mod vault;
#[cfg(not(target_os = "windows"))]
mod vault {
    use super::Credentials;
    pub fn load() -> Option<Credentials> {
        None
    }
    pub fn save(_: &Credentials) -> Result<(), String> {
        Ok(())
    }
    pub fn clear() -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cookies_require_a_complete_valid_session_and_ignore_other_cookies() {
        use reqwest::header::{HeaderMap, HeaderValue, SET_COOKIE};
        let mut headers = HeaderMap::new();
        for value in [
            "SESSDATA=example%2Fsession; Secure; HttpOnly",
            "bili_jct=0123456789abcdef0123456789abcdef; Path=/",
            "DedeUserID=42; Secure",
            "irrelevant=secret",
        ] {
            headers.append(SET_COOKIE, HeaderValue::from_str(value).unwrap());
        }
        let c = credentials(&headers).unwrap();
        assert_eq!(c.uid, 42);
        assert!(!cookie(&c).contains("irrelevant"));
        headers.remove(SET_COOKIE);
        assert!(credentials(&headers).is_none());
    }
    #[test]
    fn explicit_send_preserves_unicode_and_matches_both_csrf_fields() {
        let fields = send_fields(42, "  晚上好👩‍💻  ", "csrf", 123).unwrap();
        assert!(fields.contains(&("msg", "晚上好👩‍💻".into())));
        assert!(
            fields.contains(&("csrf", "csrf".into()))
                && fields.contains(&("csrf_token", "csrf".into()))
        );
        assert!(send_fields(0, "hi", "x", 0).is_err());
        assert!(send_fields(42, " ", "x", 0).is_err());
    }
    #[test]
    fn qr_accepts_only_the_bilibili_passport_host() {
        assert!(
            qr_png("https://passport.bilibili.com/h5-app/passport/login/scan?qrcode_key=test")
                .is_ok()
        );
        assert!(
            qr_png("https://account.bilibili.com/h5-app/passport/login/scan?qrcode_key=test")
                .is_ok()
        );
        for url in [
            "http://passport.bilibili.com/a",
            "https://passport.bilibili.com.evil.example/a",
            "https://user@passport.bilibili.com/a",
        ] {
            assert!(qr_png(url).is_err());
        }
    }
    #[test]
    fn send_success_does_not_hide_rejections_in_alternate_message_fields() {
        use serde_json::json;
        assert!(send_result(&json!({"code":0,"msg":"","data":{}})).is_ok());
        assert!(send_result(&json!({"code":0,"message":"","msg":"禁言"})).is_err());
        assert!(send_result(&json!({"code":-101,"message":""})).is_err());
        assert!(send_result(&json!({"message":""})).is_err());
    }
    #[tokio::test]
    async fn cancel_discards_an_unconsumed_login_without_installing_it() {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut account = Account {
            state: State::Loading,
            send_state: SendState::Idle,
            identity_changed: false,
            tx,
            rx,
            login: None,
            sending: None,
            last_send: None,
        };
        account
            .tx
            .send(Notice::Authenticated(
                Profile {
                    uid: 42,
                    name: "test".into(),
                    avatar: None,
                },
                Credentials {
                    session: "fake".into(),
                    csrf: "fake".into(),
                    uid: 42,
                },
            ))
            .ok();
        account.cancel();
        assert!(!account.poll());
        assert!(!account.identity_changed);
        assert!(matches!(account.state, State::SignedOut));
    }

    #[tokio::test]
    #[ignore = "read-only live QR endpoint smoke test; requires network"]
    async fn live_qr_endpoint_returns_a_renderable_bilibili_code() {
        let client = client("").unwrap();
        let value = json(
            client
                .get("https://passport.bilibili.com/x/passport-login/web/qrcode/generate")
                .send()
                .await,
        )
        .await
        .unwrap();
        let data = success(&value).expect("QR generation was rejected");
        assert!(
            data["qrcode_key"]
                .as_str()
                .is_some_and(|key| !key.is_empty())
        );
        let url = data["url"].as_str().expect("missing QR URL");
        let result = qr_png(url);
        assert!(
            result.is_ok(),
            "{}; origin={:?}; length={}",
            result.err().unwrap_or_default(),
            reqwest::Url::parse(url).ok().map(|url| format!(
                "{}://{}",
                url.scheme(),
                url.host_str().unwrap_or("")
            )),
            url.len()
        );
        let key = data["qrcode_key"].as_str().unwrap();
        let status = json(
            client
                .get(format!(
                    "https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={}",
                    urlencoding::encode(key)
                ))
                .send()
                .await,
        )
        .await
        .unwrap();
        assert_eq!(success(&status).unwrap()["code"].as_i64(), Some(86101));
        // Read the unscanned status only; never authorize or send from this test.
    }
}
