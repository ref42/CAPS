//! Read-only live capture using the same connection as CAPS.
#[path = "../src/live/bilibili_live/mod.rs"]
mod live;
use std::{error::Error, io::Write, time::Duration};
type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
struct Options {
    room: u64,
    seconds: u64,
    proxy: Option<String>,
    raw: bool,
}
#[derive(Default)]
struct Stats {
    sessions: usize,
    heartbeats: usize,
    commands: usize,
    chats: usize,
}

fn options() -> Result<Options> {
    let mut args = std::env::args().skip(1);
    let room = args.next().ok_or("Usage: cargo run --example bilibili_danmu -- ROOM_ID [--seconds 60] [--proxy http://127.0.0.1:10808] [--raw]")?.parse::<u64>()?;
    if room == 0 {
        return Err("Room ID must be positive".into());
    }
    let mut options = Options {
        room,
        seconds: 60,
        proxy: None,
        raw: false,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seconds" => options.seconds = args.next().ok_or("Missing seconds")?.parse()?,
            "--proxy" => options.proxy = Some(args.next().ok_or("Missing proxy URL")?),
            "--raw" => options.raw = true,
            _ => return Err(format!("Unknown option: {arg}").into()),
        }
    }
    if options.seconds == 0 {
        return Err("Seconds must be positive".into());
    }
    Ok(options)
}

#[tokio::main]
async fn main() -> Result<()> {
    let options = options()?;
    let mut stats = Stats::default();
    let mut output = std::io::stdout();
    let mut room = options.room;
    let mut write_error = None;
    let mut receive = |event| match event {
        live::LiveEvent::Status(text) => eprintln!("{text}"),
        live::LiveEvent::Connected(id) => {
            room = id;
            stats.sessions += 1;
            eprintln!("Authenticated in room {id} (no login).");
        }
        live::LiveEvent::Heartbeat => {
            stats.heartbeats += 1;
            eprintln!("Heartbeat received.");
        }
        live::LiveEvent::Command(command) => {
            stats.commands += 1;
            let chat = live::danmu::chat(&command, room);
            if chat.is_some() {
                stats.chats += 1;
            }
            let record = if options.raw { Some(command) } else { chat };
            if let Some(record) = record
                && let Err(err) = (|| -> Result<()> {
                    serde_json::to_writer(&mut output, &record)?;
                    writeln!(output)?;
                    output.flush()?;
                    Ok(())
                })()
            {
                write_error = Some(err);
            }
        }
    };
    tokio::select! {
        result = live::listen(options.room, options.proxy.clone(), &mut receive) => result?,
        _ = tokio::time::sleep(Duration::from_secs(options.seconds)) => eprintln!("Capture time elapsed."),
        result = tokio::signal::ctrl_c() => { result?; eprintln!("Stopped."); }
    }
    if let Some(err) = write_error {
        return Err(err);
    }
    eprintln!(
        "Summary: {} authenticated sessions, {} heartbeats, {} events, {} chat messages.",
        stats.sessions, stats.heartbeats, stats.commands, stats.chats
    );
    if stats.sessions == 0 {
        return Err("No authenticated danmu connection established".into());
    }
    Ok(())
}
