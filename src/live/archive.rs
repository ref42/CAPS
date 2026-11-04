//! Append-only received-event journal. Disk I/O never runs on the websocket or
//! UI thread. The live history is a display window, not the only copy of chat.
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    io::{self, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

enum Job {
    Record(u64, Value),
    Stop,
}

struct Archive {
    sender: mpsc::Sender<Job>,
    error: Arc<Mutex<Option<String>>>,
    stopping: Arc<AtomicBool>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

static ARCHIVE: OnceLock<Archive> = OnceLock::new();

pub fn directory() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("LOCALAPPDATA"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("CAPS").join("chat-history")
}

fn supported(command: &Value) -> bool {
    matches!(
        command["cmd"].as_str().unwrap_or("").split(':').next(),
        Some(
            "DANMU_MSG"
                | "SUPER_CHAT_MESSAGE"
                | "SUPER_CHAT_MESSAGE_DELETE"
                | "SEND_GIFT"
                | "SEND_GIFT_V2"
                | "GUARD_BUY"
                | "INTERACT_WORD"
                | "INTERACT_WORD_V2"
        )
    )
}

pub fn record(room: u64, command: &Value) {
    if !supported(command) {
        return;
    }
    let archive = ARCHIVE.get_or_init(|| {
        let started = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        // Separate runs/instances never seek or truncate each other's files.
        Archive::start(directory().join(format!("session-{}-{started}", std::process::id())))
    });
    let received = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    if archive
        .sender
        .send(Job::Record(
            room,
            json!({
                "room_id": room, "received_unix_ms": received, "command": command,
            }),
        ))
        .is_err()
    {
        *archive.error.lock().unwrap_or_else(|e| e.into_inner()) =
            Some("Chat archive worker stopped; received messages are not being saved.".into());
    }
}

pub fn error() -> Option<String> {
    ARCHIVE.get().and_then(|archive| {
        archive
            .error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    })
}

/// Called after the async runtime has stopped receiving. The stop marker is
/// ordered after all accepted records; join drains the writer on normal exit.
pub fn shutdown() {
    if let Some(archive) = ARCHIVE.get() {
        archive.stop();
    }
}

impl Archive {
    fn start(directory: PathBuf) -> Self {
        let (sender, receiver) = mpsc::channel();
        let error = Arc::new(Mutex::new(None));
        let worker_error = error.clone();
        let stopping = Arc::new(AtomicBool::new(false));
        let worker_stopping = stopping.clone();
        let worker = std::thread::Builder::new().name("chat-archive".into()).spawn(move || {
            let mut writer = Writer { directory, current: None, rollback: None };
            loop {
                let first = match receiver.recv() { Ok(job) => job, Err(_) => break };
                let mut records = Vec::new();
                let mut stop = false;
                for job in std::iter::once(first).chain(receiver.try_iter().take(255)) {
                    match job {
                        Job::Record(room, value) => records.push((room, value)),
                        Job::Stop => { stop = true; break; }
                    }
                }
                // Preserve receipt order, including room handoffs. Retry the
                // failed batch rather than silently throwing it away.
                let mut start = 0;
                while start < records.len() {
                    let room = records[start].0;
                    let end = records[start..].iter().position(|(id, _)| *id != room)
                        .map(|offset| start + offset).unwrap_or(records.len());
                    let mut bytes = Vec::new();
                    for (_, value) in &records[start..end] {
                        // serde_json::Value always serializes to a Vec writer.
                        if let Err(err) = serde_json::to_writer(&mut bytes, value) {
                            *worker_error.lock().unwrap_or_else(|e| e.into_inner()) = Some(err.to_string());
                            return;
                        }
                        bytes.push(b'\n');
                    }
                    loop {
                        match writer.append(room, &bytes) {
                            Ok(()) => {
                                *worker_error.lock().unwrap_or_else(|e| e.into_inner()) = None;
                                break;
                            }
                            Err(err) => {
                                let message = format!("Chat archive cannot save messages: {err}. Retrying; check free disk space.");
                                log::error!("{message}");
                                *worker_error.lock().unwrap_or_else(|e| e.into_inner()) = Some(message);
                                if worker_stopping.load(Ordering::Acquire) { return; }
                                std::thread::sleep(Duration::from_millis(250));
                            }
                        }
                    }
                    start = end;
                }
                if stop { break; }
            }
        });
        let worker = match worker {
            Ok(worker) => Some(worker),
            Err(err) => {
                *error.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(format!("Chat archive could not start: {err}"));
                None
            }
        };
        Self {
            sender,
            error,
            stopping,
            worker: Mutex::new(worker),
        }
    }

    fn stop(&self) {
        self.stopping.store(true, Ordering::Release);
        let _ = self.sender.send(Job::Stop);
        if let Some(worker) = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take()
            && worker.join().is_err()
        {
            *self.error.lock().unwrap_or_else(|e| e.into_inner()) =
                Some("Chat archive worker panicked.".into());
        }
    }
}

struct Writer {
    directory: PathBuf,
    current: Option<(u64, File)>,
    rollback: Option<u64>,
}

impl Writer {
    fn append(&mut self, room: u64, bytes: &[u8]) -> io::Result<()> {
        if self.current.as_ref().is_none_or(|(id, _)| *id != room) {
            std::fs::create_dir_all(&self.directory)?;
            let file = OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .truncate(false)
                .open(self.directory.join(format!("room-{room}.jsonl")))?;
            self.current = Some((room, file));
        }
        let file = &mut self.current.as_mut().unwrap().1;
        if let Some(start) = self.rollback {
            // Keep this marker until rollback succeeds, including when the
            // original error also prevented truncating the partial write.
            file.set_len(start)?;
            file.seek(SeekFrom::Start(start))?;
            self.rollback = None;
        }
        let start = file.seek(SeekFrom::End(0))?;
        self.rollback = Some(start);
        file.write_all(bytes)?;
        file.sync_data()?;
        self.rollback = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archives_bursts_in_order_and_drains_on_shutdown() {
        let root = std::env::temp_dir().join(format!(
            "caps-chat-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let archive = Archive::start(root.clone());
        for index in 0..10_000 {
            archive
                .sender
                .send(Job::Record(42, json!({"index":index, "text":"完整弹幕😀"})))
                .unwrap();
        }
        archive
            .sender
            .send(Job::Record(7, json!({"index":0})))
            .unwrap();
        archive.stop();
        assert!(archive.error.lock().unwrap().is_none());
        let text = std::fs::read_to_string(root.join("room-42.jsonl")).unwrap();
        let records: Vec<Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(records.len(), 10_000);
        for (index, record) in records.iter().enumerate() {
            assert_eq!(record["index"], index);
            assert_eq!(record["text"], "完整弹幕😀");
        }
        assert_eq!(
            std::fs::read_to_string(root.join("room-7.jsonl"))
                .unwrap()
                .lines()
                .count(),
            1
        );
        std::fs::remove_file(root.join("room-42.jsonl")).unwrap();
        std::fs::remove_file(root.join("room-7.jsonl")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn reports_disk_failure_and_retries_without_losing_the_batch() {
        let root = std::env::temp_dir().join(format!(
            "caps-chat-retry-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&root, b"file blocks directory creation").unwrap();
        let archive = Archive::start(root.clone());
        archive
            .sender
            .send(Job::Record(1, json!({"text":"kept"})))
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while archive.error.lock().unwrap().is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(archive.error.lock().unwrap().is_some());
        std::fs::remove_file(&root).unwrap();
        // Stop drains the queued record and allows one final write attempt.
        archive.stop();
        assert!(archive.error.lock().unwrap().is_none());
        assert!(
            std::fs::read_to_string(root.join("room-1.jsonl"))
                .unwrap()
                .contains("kept")
        );
        std::fs::remove_file(root.join("room-1.jsonl")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
