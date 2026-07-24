use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub enum RemoteCommand {
    Previous,
    Next,
    PlayPause,
    Stop,
    Volume(u32),
}

#[derive(Clone)]
pub struct RemoteServer {
    pub url: String,
    commands: Arc<Mutex<mpsc::Receiver<RemoteCommand>>>,
}

impl RemoteServer {
    pub fn try_recv(&self) -> Option<RemoteCommand> {
        self.commands.lock().ok()?.try_recv().ok()
    }
}

pub fn start() -> RemoteServer {
    let (command_tx, command_rx) = mpsc::channel();
    let token = session_token();
    let listener = bind_listener();
    let url = listener
        .as_ref()
        .map(|listener| {
            let port = listener
                .local_addr()
                .map(|addr| addr.port())
                .unwrap_or(49321);
            format!("http://{}:{port}/?t={token}", lan_ip())
        })
        .unwrap_or_else(|| "Phone remote unavailable".to_string());

    if let Some(listener) = listener {
        let server_token = token.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let command_tx = command_tx.clone();
                let token = server_token.clone();
                thread::spawn(move || handle_client(stream, command_tx, &token));
            }
        });
    }

    RemoteServer {
        url,
        commands: Arc::new(Mutex::new(command_rx)),
    }
}

fn bind_listener() -> Option<TcpListener> {
    (49321..49341).find_map(|port| {
        TcpListener::bind(("0.0.0.0", port))
            .inspect(|listener| {
                let _ = listener.set_nonblocking(false);
            })
            .ok()
    })
}

fn handle_client(mut stream: TcpStream, command_tx: mpsc::Sender<RemoteCommand>, token: &str) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buffer = [0_u8; 8192];
    let Ok(size) = stream.read(&mut buffer) else {
        return;
    };
    if size == 0 {
        return;
    }

    let request = String::from_utf8_lossy(&buffer[..size]);
    let Some(first_line) = request.lines().next() else {
        respond(&mut stream, 400, "text/plain; charset=utf-8", "Bad request");
        return;
    };
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");
    let (path, query) = split_target(target);

    if method == "GET" && path == "/" {
        respond(&mut stream, 200, "text/html; charset=utf-8", phone_html());
        return;
    }

    if !query_has_token(query, token) {
        respond(&mut stream, 403, "text/plain; charset=utf-8", "Forbidden");
        return;
    }

    if method == "POST" && path.starts_with("/api/control/") {
        let command = path.trim_start_matches("/api/control/");
        let accepted = match command {
            "previous" => command_tx.send(RemoteCommand::Previous).is_ok(),
            "next" => command_tx.send(RemoteCommand::Next).is_ok(),
            "play-pause" => command_tx.send(RemoteCommand::PlayPause).is_ok(),
            "stop" => command_tx.send(RemoteCommand::Stop).is_ok(),
            _ => false,
        };
        if accepted {
            respond(&mut stream, 200, "application/json", "{\"ok\":true}");
        } else {
            respond(&mut stream, 404, "application/json", "{\"ok\":false}");
        }
        return;
    }

    if method == "POST" && path == "/api/volume" {
        if let Some(value) = query_value(query, "value").and_then(|value| value.parse::<u32>().ok())
        {
            let _ = command_tx.send(RemoteCommand::Volume(value.clamp(0, 100)));
            respond(&mut stream, 200, "application/json", "{\"ok\":true}");
        } else {
            respond(&mut stream, 400, "application/json", "{\"ok\":false}");
        }
        return;
    }

    respond(&mut stream, 404, "text/plain; charset=utf-8", "Not found");
}

fn split_target(target: &str) -> (&str, &str) {
    target.split_once('?').unwrap_or((target, ""))
}

fn query_has_token(query: &str, token: &str) -> bool {
    query_value(query, "t").is_some_and(|value| value == token)
}

fn query_value<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find_map(|(candidate, value)| (candidate == key).then_some(value))
}

fn respond(stream: &mut TcpStream, code: u16, content_type: &str, body: &str) {
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        _ => "OK",
    };
    let response = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
        body.as_bytes().len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn lan_ip() -> String {
    UdpSocket::bind(("0.0.0.0", 0))
        .and_then(|socket| {
            let _ = socket.connect(("8.8.8.8", 80));
            socket.local_addr()
        })
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

fn session_token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{:x}{:x}", nanos, std::process::id())
}

fn phone_html() -> &'static str {
    r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>CAPS Fun Mode</title>
<style>
:root{color-scheme:dark;font-family:-apple-system,BlinkMacSystemFont,"SF Pro Text","Segoe UI",sans-serif;background:#050607;color:#f8fffc}
*{box-sizing:border-box}body{margin:0;min-height:100svh;display:grid;place-items:center;padding:22px;background:radial-gradient(circle at 50% 0,#24312d 0,#050607 46%)}
main{width:min(100%,420px);display:grid;gap:16px;text-align:center}
.caps{height:76px;border-radius:999px;background:rgba(10,14,15,.78);border:1px solid rgba(255,255,255,.1);box-shadow:inset 0 1px rgba(255,255,255,.12);display:grid;place-items:center;backdrop-filter:blur(22px) saturate(1.2)}
h1{margin:0;font-size:24px;letter-spacing:0}.hint{margin:0;color:rgba(248,255,252,.62);font-size:14px;line-height:1.45}
.grid{display:grid;grid-template-columns:repeat(2,1fr);gap:10px}
button{height:58px;border:0;border-radius:18px;background:rgba(255,255,255,.12);color:#f8fffc;font-size:17px;font-weight:800;touch-action:manipulation}
button:active{transform:scale(.97);background:rgba(255,255,255,.18)}
.wide{grid-column:1/-1}.live{background:linear-gradient(135deg,#ff5f68,#ffb86b);color:#16090a}.volume{display:grid;gap:8px;padding:14px;border-radius:20px;background:rgba(255,255,255,.08)}
input{width:100%;accent-color:#ff69b4}.state{min-height:22px;color:rgba(248,255,252,.7);font-size:13px}
</style>
</head>
<body>
<main>
<section class="caps"><h1>CAPS Fun Mode</h1></section>
<p class="hint">Tap Start Motion, then shake for next, tilt left/right for previous/next, and tilt forward/back for volume.</p>
<div class="grid">
<button onclick="control('previous')">Previous</button>
<button onclick="control('next')">Next</button>
<button onclick="control('play-pause')">Play / Pause</button>
<button onclick="control('stop')">Stop</button>
<button class="wide live" onclick="startMotion()">Start Motion</button>
</div>
<section class="volume">
<input id="volume" type="range" min="0" max="100" value="100" oninput="setVolume(this.value)">
<div class="state" id="state">Manual controls are ready.</div>
</section>
</main>
<script>
const token=new URLSearchParams(location.search).get('t')||'';
const state=document.getElementById('state');
const slider=document.getElementById('volume');
let lastShake=0,lastTilt=0,lastVolume=0,motionReady=false;
function api(path){return `${path}${path.includes('?')?'&':'?'}t=${encodeURIComponent(token)}`}
async function control(name){await fetch(api(`/api/control/${name}`),{method:'POST'});state.textContent=`Sent ${name}.`}
async function setVolume(value){await fetch(api(`/api/volume?value=${Math.round(value)}`),{method:'POST'});state.textContent=`Volume ${Math.round(value)}%.`}
async function startMotion(){
  try{
    if(typeof DeviceMotionEvent!=='undefined'&&typeof DeviceMotionEvent.requestPermission==='function'){
      const result=await DeviceMotionEvent.requestPermission();
      if(result!=='granted'){state.textContent='Motion permission denied.';return}
    }
    if(typeof DeviceOrientationEvent!=='undefined'&&typeof DeviceOrientationEvent.requestPermission==='function'){
      await DeviceOrientationEvent.requestPermission().catch(()=>null);
    }
    if(motionReady)return;
    motionReady=true;
    window.addEventListener('devicemotion',onMotion);
    window.addEventListener('deviceorientation',onOrientation);
    state.textContent='Motion controls are live.';
  }catch(error){state.textContent='Motion is unavailable in this browser.'}
}
function onMotion(event){
  const a=event.accelerationIncludingGravity||event.acceleration||{};
  const x=a.x||0,y=a.y||0,z=a.z||0;
  const force=Math.sqrt(x*x+y*y+z*z);
  const now=Date.now();
  if(force>24&&now-lastShake>900){lastShake=now;control('next')}
}
function onOrientation(event){
  const now=Date.now();
  const gamma=event.gamma||0;
  const beta=event.beta||0;
  if(now-lastTilt>1200){
    if(gamma>32){lastTilt=now;control('next')}
    if(gamma<-32){lastTilt=now;control('previous')}
  }
  if(now-lastVolume>260&&Math.abs(beta)>12){
    const value=Math.max(0,Math.min(100,Math.round(100-(beta+35)/90*100)));
    if(Math.abs(value-Number(slider.value))>=3){
      lastVolume=now;
      slider.value=value;
      setVolume(value);
    }
  }
}
</script>
</body>
</html>"#
}
