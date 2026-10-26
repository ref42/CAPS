//! Range-backed media, decoded off the audio thread into a bounded PCM queue.
#[path = "streaming/decoder.rs"]
mod decoder;
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    time::Duration,
};

use rodio::Source;

const BLOCK: u64 = 512 * 1024;
const QUEUE: usize = 50; // Two seconds of decoded audio, independent of media size.
static TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct Cache {
    file: Option<File>,
    ready: Vec<bool>,
    wanted: usize,
    downloaded: u64,
    complete: bool,
    sequential: bool,
    valid: bool,
    published: bool,
    error: Option<String>,
}

#[derive(Debug)]
struct Download {
    cache: Mutex<Cache>,
    changed: Condvar,
    cancelled: AtomicBool,
    task: Mutex<Option<tokio::task::AbortHandle>>,
    partial: PathBuf,
    target: PathBuf,
    size: Option<u64>,
}

impl Download {
    fn cancel(&self) {
        // Take the same lock as the reader so a wakeup cannot be lost.
        let mut cache = self.cache.lock().unwrap();
        self.cancelled.store(true, Ordering::Release);
        self.changed.notify_all();
        if let Some(task) = self.task.lock().unwrap().take() {
            task.abort();
        }
        // Close/remove synchronously: process shutdown need not wait for a worker.
        cache.file.take();
        if !cache.published {
            let _ = std::fs::remove_file(&self.partial);
        }
    }

    fn fail(&self, error: String) {
        self.cache.lock().unwrap().error = Some(error);
        self.changed.notify_all();
    }

    fn publish(&self, cache: &mut Cache) -> io::Result<()> {
        if cache.valid && cache.complete && !cache.published {
            cache.file.as_mut().unwrap().flush()?;
            std::fs::rename(&self.partial, &self.target)?;
            cache.published = true;
        }
        Ok(())
    }
}

impl Drop for Download {
    fn drop(&mut self) {
        let cache = self.cache.get_mut().unwrap();
        cache.file.take();
        if !cache.published {
            let _ = std::fs::remove_file(&self.partial);
        }
    }
}

// Cancels even if the async preparation future is aborted before it returns.
struct Preparation(Option<Arc<Download>>);
impl Drop for Preparation {
    fn drop(&mut self) {
        if let Some(download) = &self.0 {
            download.cancel();
        }
    }
}

struct Reader {
    download: Arc<Download>,
    position: u64,
}

impl symphonia::core::io::MediaSource for Reader {
    fn is_seekable(&self) -> bool {
        self.download.size.is_some()
    }
    fn byte_len(&self) -> Option<u64> {
        self.download.size
    }
}

impl Read for Reader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let mut cache = self.download.cache.lock().unwrap();
        loop {
            if self.download.cancelled.load(Ordering::Acquire) {
                return Err(io::Error::other("Playback stopped"));
            }
            if let Some(error) = &cache.error {
                return Err(io::Error::other(error.clone()));
            }
            let length = self
                .download
                .size
                .or_else(|| cache.complete.then_some(cache.downloaded));
            if length.is_some_and(|size| self.position >= size) {
                return Ok(0);
            }
            let index = (self.position / BLOCK) as usize;
            let available = if cache.sequential {
                cache.downloaded.saturating_sub(self.position)
            } else if cache.ready.get(index) == Some(&true) {
                (BLOCK - self.position % BLOCK).min(length.unwrap() - self.position)
            } else {
                0
            };
            if available > 0 {
                let count = buffer.len().min(available as usize);
                let file = cache.file.as_mut().unwrap();
                file.seek(SeekFrom::Start(self.position))?;
                file.read_exact(&mut buffer[..count])?;
                self.position += count as u64;
                return Ok(count);
            }
            cache.wanted = index;
            cache = self.download.changed.wait(cache).unwrap();
        }
    }
}

impl Seek for Reader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(value) => value as i128,
            SeekFrom::Current(value) => self.position as i128 + value as i128,
            SeekFrom::End(value) => {
                let cache = self.download.cache.lock().unwrap();
                let size = self
                    .download
                    .size
                    .or_else(|| cache.complete.then_some(cache.downloaded))
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::Unsupported, "Stream length is not known yet")
                    })?;
                size as i128 + value as i128
            }
        };
        if !(0..=u64::MAX as i128).contains(&position) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid seek"));
        }
        self.position = position as u64;
        Ok(self.position)
    }
}

fn range_header(response: &reqwest::Response) -> Result<(u64, u64, u64), String> {
    if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err("Server does not support buffered playback (HTTP byte ranges).".into());
    }
    let parsed = response
        .headers()
        .get("content-range")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("bytes "))
        .and_then(|v| v.split_once('/'))
        .and_then(|(range, total)| {
            let (start, end) = range.split_once('-')?;
            Some((start.parse().ok()?, end.parse().ok()?, total.parse().ok()?))
        });
    parsed.ok_or_else(|| "Invalid media Content-Range header.".into())
}

async fn fetch(
    client: &reqwest::Client,
    url: &str,
    start: u64,
    end: u64,
    validator: Option<&str>,
) -> Result<reqwest::Response, String> {
    let mut request = client
        .get(url)
        .header("Accept-Encoding", "identity")
        .header("Range", format!("bytes={start}-{end}"));
    if let Some(validator) = validator {
        request = request.header("If-Range", validator);
    }
    request
        .send()
        .await
        .map_err(|e| format!("Media download failed: {e}"))
}

async fn body(mut response: reqwest::Response, expected: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity(expected);
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > expected {
            return Err("Media server returned an oversized range.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.len() != expected {
        return Err("Media download was truncated.".into());
    }
    Ok(bytes)
}

async fn download_ranges(
    download: &Download,
    client: reqwest::Client,
    url: String,
    validator: Option<String>,
) -> Result<(), String> {
    let size = download.size.expect("range responses have a length");
    loop {
        let index = {
            let cache = download.cache.lock().unwrap();
            if download.cancelled.load(Ordering::Acquire) {
                return Ok(());
            }
            if !cache.ready[cache.wanted] {
                cache.wanted
            } else if let Some(index) = cache.ready.iter().position(|ready| !ready) {
                index
            } else {
                return Ok(());
            }
        };
        let start = index as u64 * BLOCK;
        let end = (start + BLOCK).min(size) - 1;
        // Retry transient failures, but never mix versions or accept an ignored Range.
        let mut result = Err("Media download failed.".to_string());
        for _ in 0..3 {
            result = async {
                let response = fetch(&client, &url, start, end, validator.as_deref()).await?;
                if range_header(&response)? != (start, end, size) {
                    return Err("Media changed or the server returned the wrong range.".into());
                }
                body(response, (end - start + 1) as usize).await
            }
            .await;
            if result.is_ok() {
                break;
            }
        }
        let bytes = result?;
        let mut cache = download.cache.lock().unwrap();
        if download.cancelled.load(Ordering::Acquire) {
            return Ok(());
        }
        let file = cache.file.as_mut().unwrap();
        file.seek(SeekFrom::Start(start))
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        cache.ready[index] = true;
        cache.downloaded += bytes.len() as u64;
        cache.complete = cache.downloaded == size;
        download.publish(&mut cache).map_err(|e| e.to_string())?;
        download.changed.notify_all();
    }
}

async fn download_sequential(
    download: &Download,
    mut response: reqwest::Response,
) -> Result<(), String> {
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Media download failed: {e}"))?
    {
        let mut cache = download.cache.lock().unwrap();
        if download.cancelled.load(Ordering::Acquire) {
            return Ok(());
        }
        let offset = cache.downloaded;
        if offset + chunk.len() as u64 > download.size.unwrap_or(64 * 1024 * 1024 * 1024) {
            return Err("Media response exceeds its expected length.".into());
        }
        let file = cache.file.as_mut().unwrap();
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        cache.downloaded += chunk.len() as u64;
        download.changed.notify_all();
    }
    let mut cache = download.cache.lock().unwrap();
    if download.cancelled.load(Ordering::Acquire) {
        return Ok(());
    }
    if cache.downloaded == 0 || download.size.is_some_and(|size| cache.downloaded != size) {
        return Err("Media download was empty or truncated.".into());
    }
    cache.complete = true;
    download.publish(&mut cache).map_err(|e| e.to_string())?;
    download.changed.notify_all();
    Ok(())
}

/// Resolved URLs are short lived; callers store the provider's stable track ID.
pub struct RemoteMedia {
    pub client: reqwest::Client,
    pub urls: Vec<String>,
}

impl RemoteMedia {
    pub async fn prepare(self, path: &Path) -> Result<BufferedSource, String> {
        let mut last = "No playable media URL was provided.".to_string();
        for url in self.urls {
            match prepare(self.client.clone(), &url, path).await {
                Ok(source) => return Ok(source),
                Err(error) => last = error,
            }
        }
        Err(last)
    }
}

pub fn media_client(referer: &str) -> Result<reqwest::Client, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    if !referer.is_empty() {
        headers.insert(
            reqwest::header::REFERER,
            referer
                .parse()
                .map_err(|e| format!("Invalid media referer: {e}"))?,
        );
    }
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/124.0 Safari/537.36")
        .default_headers(headers)
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .build().map_err(|e| e.to_string())
}

#[derive(Debug)]
struct Engine {
    download: Arc<Download>,
    epoch: AtomicU64,
    seek: Mutex<Option<(u64, Duration)>>,
    position: AtomicU64,
    buffering: AtomicBool,
    error: Mutex<Option<String>>,
    rate: u64,
}

#[derive(Debug, Clone)]
pub struct Control(Arc<Engine>);

impl Control {
    pub fn position(&self) -> f64 {
        self.0.position.load(Ordering::Relaxed) as f64 / self.0.rate as f64
    }
    pub fn buffering(&self) -> bool {
        self.0.buffering.load(Ordering::Relaxed)
    }
    pub fn error(&self) -> Option<String> {
        self.0
            .error
            .lock()
            .unwrap()
            .clone()
            .or_else(|| self.0.download.cache.lock().unwrap().error.clone())
    }
    pub fn downloaded(&self) -> (u64, u64) {
        (
            self.0.download.cache.lock().unwrap().downloaded,
            self.0.download.size.unwrap_or(0),
        )
    }
    pub fn cancel(&self) {
        self.0.download.cancel();
    }
    pub fn seek(&self, position: Duration) {
        let mut seek = self.0.seek.lock().unwrap();
        let epoch = self.0.epoch.fetch_add(1, Ordering::AcqRel) + 1;
        self.0.position.store(
            (position.as_secs_f64() * self.0.rate as f64) as u64,
            Ordering::Relaxed,
        );
        self.0.buffering.store(true, Ordering::Relaxed);
        *seek = Some((epoch, position));
    }
}

#[derive(Debug)]
struct Packet {
    epoch: u64,
    samples: Vec<f32>,
}

#[derive(Debug)]
pub struct BufferedSource {
    control: Control,
    receiver: Receiver<Packet>,
    samples: std::vec::IntoIter<f32>,
    epoch: u64,
    channels: rodio::ChannelCount,
    sample_rate: rodio::SampleRate,
    duration: Option<Duration>,
    silence_channel: usize,
    ended: bool,
}

impl BufferedSource {
    pub fn control(&self) -> Control {
        self.control.clone()
    }
}

impl Iterator for BufferedSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let requested = self.control.0.epoch.load(Ordering::Acquire);
        // Change interleaved PCM only at a frame boundary, preserving L/R order.
        if self.epoch != requested
            && self
                .samples
                .len()
                .is_multiple_of(self.channels.get() as usize)
            && self.silence_channel == 0
        {
            self.samples = Vec::new().into_iter();
            self.epoch = requested;
            self.ended = false;
        }
        let epoch = self.epoch;
        // Finish a silent frame before resuming interleaved PCM.
        if self.silence_channel != 0 {
            self.silence_channel = (self.silence_channel + 1) % self.channels.get() as usize;
            return Some(0.0);
        }
        if self.samples.len() == 0 && !self.ended {
            // The producer is bounded, so stale data after a seek is bounded too.
            for _ in 0..=QUEUE {
                match self.receiver.try_recv() {
                    Ok(packet) if packet.epoch == epoch => {
                        self.ended = packet.samples.is_empty();
                        self.samples = packet.samples.into_iter();
                        break;
                    }
                    Ok(_) => continue,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.ended = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        if let Some(sample) = self.samples.next() {
            self.control.0.buffering.store(false, Ordering::Relaxed);
            self.control.0.position.fetch_add(1, Ordering::Relaxed);
            Some(sample)
        } else if self.ended {
            None
        } else {
            self.control.0.buffering.store(true, Ordering::Relaxed);
            self.silence_channel = 1 % self.channels.get() as usize;
            Some(0.0)
        }
    }
}

impl Source for BufferedSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> rodio::ChannelCount {
        self.channels
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        self.duration
    }
    fn try_seek(&mut self, position: Duration) -> Result<(), rodio::source::SeekError> {
        self.control.seek(position);
        Ok(())
    }
}

impl Drop for BufferedSource {
    fn drop(&mut self) {
        self.control.cancel();
    }
}

type Ready = (
    Control,
    rodio::ChannelCount,
    rodio::SampleRate,
    Option<Duration>,
);

fn decode(
    download: Arc<Download>,
    sender: SyncSender<Packet>,
    ready: tokio::sync::oneshot::Sender<Result<Ready, String>>,
) {
    let result = (|| {
        let reader = Reader {
            download: download.clone(),
            position: 0,
        };
        let mut decoder = decoder::Decoder::new(reader)?;
        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();
        let duration = decoder.duration();
        let rate = channels.get() as u64 * sample_rate.get() as u64;
        let count = (sample_rate.get() as usize / 25) * channels.get() as usize;
        let first = decoder.chunk(count)?;
        if first.is_empty() {
            return Err("Video has no decodable audio.".into());
        }
        {
            let mut cache = download.cache.lock().unwrap();
            if download.cancelled.load(Ordering::Acquire) {
                return Err("Playback stopped".into());
            }
            cache.valid = true;
            download.publish(&mut cache).map_err(|e| e.to_string())?;
        }
        let control = Control(Arc::new(Engine {
            download: download.clone(),
            epoch: AtomicU64::new(0),
            seek: Mutex::new(None),
            position: AtomicU64::new(0),
            buffering: AtomicBool::new(false),
            error: Mutex::new(None),
            rate,
        }));
        Ok((
            decoder,
            control,
            channels,
            sample_rate,
            duration,
            count,
            first,
        ))
    })();
    let (mut decoder, control, channels, sample_rate, duration, count, first) = match result {
        Ok(value) => value,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let mut packet = Packet {
        epoch: 0,
        samples: first,
    };
    // Queue the first audio before announcing readiness.
    if sender.send(packet).is_err() {
        return;
    }
    if ready
        .send(Ok((control.clone(), channels, sample_rate, duration)))
        .is_err()
    {
        return;
    }
    let mut epoch = 0;
    loop {
        if download.cancelled.load(Ordering::Acquire) {
            return;
        }
        let request = control.0.seek.lock().unwrap().take();
        if let Some((generation, position)) = request {
            epoch = generation;
            if let Err(error) = decoder.try_seek(position) {
                *control.0.error.lock().unwrap() = Some(format!("Could not seek audio: {error}"));
                return;
            }
        }
        let samples = match decoder.chunk(count) {
            Ok(samples) => samples,
            Err(error) => {
                *control.0.error.lock().unwrap() = Some(error);
                return;
            }
        };
        let ended = samples.is_empty();
        packet = Packet { epoch, samples };
        loop {
            if download.cancelled.load(Ordering::Acquire) {
                return;
            }
            if control.0.epoch.load(Ordering::Acquire) != epoch {
                break;
            }
            match sender.try_send(packet) {
                Ok(()) => {
                    if ended {
                        // Keep seeking available while the last buffered PCM is playing.
                        while !download.cancelled.load(Ordering::Acquire)
                            && control.0.epoch.load(Ordering::Acquire) == epoch
                        {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                    }
                    break;
                }
                Err(TrySendError::Disconnected(_)) => return,
                Err(TrySendError::Full(returned)) => packet = returned,
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Prepare any HTTP audio/video stream using the provider's client and headers.
/// Ranges support immediate seeking; ordinary responses are read progressively.
pub async fn prepare(
    client: reqwest::Client,
    url: &str,
    target: &Path,
) -> Result<BufferedSource, String> {
    let response = fetch(&client, url, 0, 0, None).await?;
    let sequential = response.status() == reqwest::StatusCode::OK;
    let size = if sequential {
        response.content_length()
    } else {
        let (start, end, size) = range_header(&response)?;
        if start != 0 || end != 0 {
            return Err("Invalid initial media range.".into());
        }
        Some(size)
    };
    if size.is_some_and(|size| size == 0 || size > 64 * 1024 * 1024 * 1024) {
        return Err("Invalid media length.".into());
    }
    let url = response.url().to_string();
    let validator = response
        .headers()
        .get("etag")
        .filter(|v| !v.as_bytes().starts_with(b"W/"))
        .or_else(|| response.headers().get("last-modified"))
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let response = if sequential {
        Some(response)
    } else {
        body(response, 1).await?;
        None
    };
    let partial = target.with_extension(format!(
        "{}-{}.partial",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&partial)
        .map_err(|e| e.to_string())?;
    let download = Arc::new(Download {
        cache: Mutex::new(Cache {
            file: Some(file),
            ready: if sequential {
                Vec::new()
            } else {
                vec![false; size.unwrap().div_ceil(BLOCK) as usize]
            },
            complete: false,
            sequential,
            wanted: 0,
            downloaded: 0,
            valid: false,
            published: false,
            error: None,
        }),
        changed: Condvar::new(),
        cancelled: AtomicBool::new(false),
        task: Mutex::new(None),
        partial,
        target: target.into(),
        size,
    });
    let mut preparation = Preparation(Some(download.clone()));
    let downloading = download.clone();
    let task = tokio::spawn(async move {
        let result = match response {
            Some(response) => download_sequential(&downloading, response).await,
            None => download_ranges(&downloading, client, url, validator).await,
        };
        if let Err(error) = result {
            downloading.fail(error);
        }
    });
    *download.task.lock().unwrap() = Some(task.abort_handle());
    let (sender, receiver) = mpsc::sync_channel(QUEUE);
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("stream-decoder".into())
        .spawn(move || decode(download, sender, ready_tx))
        .map_err(|e| e.to_string())?;
    let (control, channels, sample_rate, duration) =
        ready_rx.await.map_err(|e| e.to_string())??;
    preparation.0 = None;
    Ok(BufferedSource {
        control,
        receiver,
        samples: Vec::new().into_iter(),
        epoch: 0,
        channels,
        sample_rate,
        duration,
        silence_channel: 0,
        ended: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // A real, seekable PCM WAV. The second half has a different sample value,
    // allowing tests to detect stale audio after seeking, not just a moved clock.
    fn wav() -> Vec<u8> {
        let frames = 48_000 * 20;
        let bytes = frames * 2;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36_u32 + bytes).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
        wav.extend_from_slice(&48_000_u32.to_le_bytes());
        wav.extend_from_slice(&96_000_u32.to_le_bytes());
        wav.extend_from_slice(b"\x02\0\x10\0data");
        wav.extend_from_slice(&bytes.to_le_bytes());
        for index in 0..frames {
            let value: i16 = if index < frames / 2 { 4096 } else { -8192 };
            wav.extend_from_slice(&value.to_le_bytes());
        }
        wav
    }

    #[derive(Clone, Copy)]
    enum Behavior {
        Normal,
        NoRanges,
        WrongRange,
        Truncated,
        FailLater,
        Slow,
        Sequential,
        UnknownLength,
    }

    struct Server {
        url: String,
        task: tokio::task::JoinHandle<()>,
    }
    impl Drop for Server {
        fn drop(&mut self) {
            self.task.abort();
        }
    }
    async fn server(bytes: Vec<u8>, behavior: Behavior) -> Server {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/media", listener.local_addr().unwrap());
        let bytes = Arc::new(bytes);
        let task = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let bytes = bytes.clone();
                tokio::spawn(async move {
                    let mut request = vec![0; 8192];
                    let n = socket.read(&mut request).await.unwrap();
                    let request = String::from_utf8_lossy(&request[..n]).to_lowercase();
                    if !request.contains("referer: https://provider.example/") {
                        let _ = socket
                            .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
                            .await;
                        return;
                    }
                    let range = request
                        .lines()
                        .find_map(|l| l.strip_prefix("range: bytes="))
                        .unwrap();
                    let (start, end) = range.split_once('-').unwrap();
                    let start: usize = start.parse().unwrap();
                    let end: usize = end.parse::<usize>().unwrap().min(bytes.len() - 1);
                    if matches!(behavior, Behavior::Sequential | Behavior::UnknownLength) {
                        let length = if matches!(behavior, Behavior::Sequential) {
                            format!("Content-Length: {}\r\n", bytes.len())
                        } else {
                            String::new()
                        };
                        let _ = socket
                            .write_all(
                                format!("HTTP/1.1 200 OK\r\n{length}Connection: close\r\n\r\n")
                                    .as_bytes(),
                            )
                            .await;
                        for chunk in bytes.chunks(32 * 1024) {
                            if socket.write_all(chunk).await.is_err() {
                                return;
                            }
                            tokio::time::sleep(Duration::from_millis(20)).await;
                        }
                        return;
                    }
                    if matches!(behavior, Behavior::NoRanges) {
                        let _ = socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                        return;
                    }
                    if end > 0 {
                        let delay = if matches!(behavior, Behavior::Slow) {
                            2000
                        } else {
                            120
                        };
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                    }
                    if start > 0 && matches!(behavior, Behavior::FailLater) {
                        let _ = socket.write_all(b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                        return;
                    }
                    let header_start = if end > 0 && matches!(behavior, Behavior::WrongRange) {
                        start + 1
                    } else {
                        start
                    };
                    let header = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {header_start}-{end}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        bytes.len(),
                        end - start + 1
                    );
                    let _ = socket.write_all(header.as_bytes()).await;
                    let sent_end = if end > 0 && matches!(behavior, Behavior::Truncated) {
                        end - 1
                    } else {
                        end
                    };
                    let _ = socket.write_all(&bytes[start..=sent_end]).await;
                });
            }
        });
        Server { url, task }
    }

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "caps-stream-test-{}-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                TEMP_ID.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn target(&self) -> PathBuf {
            self.0.join("audio.cache")
        }
        async fn wait_empty(&self) {
            tokio::time::timeout(Duration::from_secs(3), async {
                while std::fs::read_dir(&self.0).unwrap().next().is_some() {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("partial download was not cleaned up");
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn client() -> reqwest::Client {
        reqwest::Client::builder()
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    reqwest::header::REFERER,
                    "https://provider.example/".parse().unwrap(),
                );
                headers
            })
            .no_proxy()
            .timeout(Duration::from_secs(4))
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn starts_before_download_finishes_and_seek_discards_old_pcm() {
        let server = server(wav(), Behavior::Normal).await;
        let temp = Temp::new();
        let mut source = prepare(client(), &server.url, &temp.target())
            .await
            .unwrap();
        let control = source.control();
        assert!(control.downloaded().0 < control.downloaded().1);
        assert!(!temp.target().exists());
        assert_eq!(source.next(), Some(0.125));
        // Fill the PCM queue while playback is paused, then seek beyond downloaded bytes.
        tokio::time::sleep(Duration::from_millis(40)).await;
        control.seek(Duration::from_secs(16));
        let starvation = std::time::Instant::now();
        for _ in 0..1000 {
            assert_eq!(source.next(), Some(0.0));
        }
        assert!(starvation.elapsed() < Duration::from_millis(100));
        assert_eq!(
            control.position(),
            16.0,
            "silence must not advance media time"
        );
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let sample = source.next().expect("buffering must not report EOF");
                if sample != 0.0 {
                    assert_eq!(sample, -0.25);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert!(control.position() >= 16.0);
        drop(source);
        drop(control);
        temp.wait_empty().await;
    }

    #[tokio::test]
    async fn complete_download_is_published_byte_for_byte() {
        let bytes = wav();
        let server = server(bytes.clone(), Behavior::Normal).await;
        let temp = Temp::new();
        let source = prepare(client(), &server.url, &temp.target())
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while !temp.target().exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(std::fs::read(temp.target()).unwrap(), bytes);
        drop(source);
    }

    #[tokio::test]
    async fn empty_and_bad_responses_never_become_cache_files() {
        for behavior in [
            Behavior::NoRanges,
            Behavior::WrongRange,
            Behavior::Truncated,
        ] {
            let server = server(wav(), behavior).await;
            let temp = Temp::new();
            let result = prepare(client(), &server.url, &temp.target()).await;
            assert!(result.is_err());
            temp.wait_empty().await;
        }
    }

    #[tokio::test]
    async fn ordinary_and_unknown_length_responses_play_before_completion() {
        for behavior in [Behavior::Sequential, Behavior::UnknownLength] {
            let bytes = wav();
            let server = server(bytes.clone(), behavior).await;
            let temp = Temp::new();
            let mut source = prepare(client(), &server.url, &temp.target())
                .await
                .unwrap();
            let control = source.control();
            assert!(control.downloaded().0 < bytes.len() as u64);
            assert_eq!(source.next(), Some(0.125));
            assert!(!temp.target().exists());
            tokio::time::timeout(Duration::from_secs(4), async {
                while !temp.target().exists() {
                    assert!(control.error().is_none(), "{:?}", control.error());
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(std::fs::read(temp.target()).unwrap(), bytes);
            drop(source);
        }
    }

    #[tokio::test]
    async fn network_failure_wakes_decoder_and_remains_an_error() {
        let server = server(wav(), Behavior::FailLater).await;
        let temp = Temp::new();
        let mut source = prepare(client(), &server.url, &temp.target())
            .await
            .unwrap();
        let control = source.control();
        control.seek(Duration::from_secs(16));
        tokio::time::timeout(Duration::from_secs(3), async {
            while control.error().is_none() {
                source.next();
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert!(!temp.target().exists());
        drop(source);
        drop(control);
        temp.wait_empty().await;
    }

    #[tokio::test]
    async fn cancelled_startup_wakes_reader_and_removes_partial_file() {
        let server = server(wav(), Behavior::Slow).await;
        let temp = Temp::new();
        let target = temp.target();
        let url = server.url.clone();
        let job = tokio::spawn(async move { prepare(client(), &url, &target).await });
        tokio::time::timeout(Duration::from_secs(3), async {
            while std::fs::read_dir(&temp.0).unwrap().next().is_none() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        job.abort();
        let _ = job.await;
        temp.wait_empty().await;
    }
}
