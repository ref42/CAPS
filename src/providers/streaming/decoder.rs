//! Symphonia 0.6 stops MP4 probing at the first playable fragment. Rodio's
//! bundled 0.5 demuxer scans every fragment first, defeating progressive playback.
use rodio::Source;
use std::time::Duration;
use symphonia::core::{
    codecs::audio::AudioDecoder,
    errors::Error,
    formats::{FormatReader, SeekMode, SeekTo, TrackType, probe::Hint},
    io::{MediaSource, MediaSourceStream},
    units::{Time, TimeBase},
};

pub(super) enum Decoder {
    Mp4(Mp4Decoder),
    Other(rodio::Decoder<Box<dyn MediaSource>>),
}

impl Decoder {
    pub fn new(mut source: impl MediaSource + 'static) -> Result<Self, String> {
        // Keep the established codec paths for MP3/FLAC/WAV/Ogg. The MP4 path
        // specifically needs the newer demuxer's incremental fragment discovery.
        let mut header = [0; 12];
        let mut count = 0;
        while count < header.len() {
            let n = source
                .read(&mut header[count..])
                .map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            count += n;
        }
        source
            .seek(std::io::SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
        if count == 12 && &header[4..8] == b"ftyp" {
            return Mp4Decoder::new(source).map(Self::Mp4);
        }
        let len = source.byte_len();
        let seekable = source.is_seekable();
        let mut builder = rodio::Decoder::builder()
            .with_data(Box::new(source) as Box<dyn MediaSource>)
            .with_seekable(seekable);
        if let Some(len) = len {
            builder = builder.with_byte_len(len);
        }
        builder.build().map(Self::Other).map_err(|e| e.to_string())
    }
    pub fn channels(&self) -> rodio::ChannelCount {
        match self {
            Self::Mp4(d) => d.channels,
            Self::Other(d) => d.channels(),
        }
    }
    pub fn sample_rate(&self) -> rodio::SampleRate {
        match self {
            Self::Mp4(d) => d.sample_rate,
            Self::Other(d) => d.sample_rate(),
        }
    }
    pub fn duration(&self) -> Option<Duration> {
        match self {
            Self::Mp4(d) => d.duration,
            Self::Other(d) => d.total_duration(),
        }
    }
    pub fn chunk(&mut self, count: usize) -> Result<Vec<f32>, String> {
        match self {
            Self::Mp4(d) => d.chunk(count),
            Self::Other(d) => Ok(d.by_ref().take(count).collect()),
        }
    }
    pub fn try_seek(&mut self, position: Duration) -> Result<(), String> {
        match self {
            Self::Mp4(d) => d.try_seek(position),
            Self::Other(d) => d.try_seek(position).map_err(|e| e.to_string()),
        }
    }
}

pub(super) struct Mp4Decoder {
    format: Box<dyn FormatReader>,
    codec: Box<dyn AudioDecoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    pub channels: rodio::ChannelCount,
    pub sample_rate: rodio::SampleRate,
    pub duration: Option<Duration>,
    samples: std::vec::IntoIter<f32>,
    seek_target: Option<f64>,
}

impl Mp4Decoder {
    pub fn new(source: impl MediaSource + 'static) -> Result<Self, String> {
        let stream = MediaSourceStream::new(Box::new(source), Default::default());
        let format = symphonia::default::get_probe()
            .probe(&Hint::new(), stream, Default::default(), Default::default())
            .map_err(|e| e.to_string())?;
        let track = format
            .default_track(TrackType::Audio)
            .ok_or("No audio track in media.")?;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .ok_or("Missing audio codec parameters.")?;
        let codec = symphonia::default::get_codecs()
            .make_audio_decoder(params, &Default::default())
            .map_err(|e| e.to_string())?;
        let track_id = track.id;
        let time_base = track.time_base;
        let duration = track
            .duration
            .zip(time_base)
            .and_then(|(duration, base)| base.calc_duration(duration))
            .map(|time| time.as_secs_f64())
            .filter(|seconds| *seconds > 0.0)
            .map(Duration::from_secs_f64);
        let mut decoder = Self {
            format,
            codec,
            track_id,
            time_base,
            duration,
            channels: rodio::ChannelCount::new(1).unwrap(),
            sample_rate: rodio::SampleRate::new(48_000).unwrap(),
            samples: Vec::new().into_iter(),
            seek_target: None,
        };
        if !decoder.refill(true)? {
            return Err("Media has no decodable audio.".into());
        }
        Ok(decoder)
    }

    fn refill(&mut self, initial: bool) -> Result<bool, String> {
        let mut bad_packets = 0;
        loop {
            let Some(packet) = self.format.next_packet().map_err(|e| e.to_string())? else {
                return Ok(false);
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let buffer = match self.codec.decode(&packet) {
                Ok(buffer) => buffer,
                Err(Error::DecodeError(_) | Error::IoError(_)) if bad_packets < 16 => {
                    bad_packets += 1;
                    continue;
                }
                Err(error) => return Err(format!("Audio decode failed: {error}")),
            };
            let channels = rodio::ChannelCount::new(
                buffer
                    .spec()
                    .channels()
                    .count()
                    .try_into()
                    .map_err(|_| "Too many audio channels")?,
            )
            .ok_or("Audio has no channels")?;
            let rate = rodio::SampleRate::new(buffer.spec().rate()).ok_or("Invalid sample rate")?;
            if !initial && (channels != self.channels || rate != self.sample_rate) {
                return Err("Audio format changed during playback.".into());
            }
            self.channels = channels;
            self.sample_rate = rate;
            let mut samples = vec![0.0; buffer.samples_interleaved()];
            buffer.copy_to_slice_interleaved(&mut samples);
            let mut skip = 0;
            if let Some(target) = self.seek_target {
                let time = self
                    .time_base
                    .and_then(|base| base.calc_time(packet.pts))
                    .ok_or("Audio seek timestamps are unavailable")?
                    .as_secs_f64();
                skip = (((target - time).max(0.0) * rate.get() as f64).round() as usize)
                    .saturating_mul(channels.get() as usize)
                    .min(samples.len());
                if skip < samples.len() {
                    self.seek_target = None;
                }
            }
            self.samples = samples.into_iter();
            if skip > 0 {
                self.samples.nth(skip - 1);
            }
            if self.samples.len() > 0 {
                return Ok(true);
            }
        }
    }

    pub fn chunk(&mut self, count: usize) -> Result<Vec<f32>, String> {
        let mut output = Vec::with_capacity(count);
        while output.len() < count {
            output.extend(self.samples.by_ref().take(count - output.len()));
            if output.len() == count || !self.refill(false)? {
                break;
            }
        }
        Ok(output)
    }

    pub fn try_seek(&mut self, position: Duration) -> Result<(), String> {
        let time = Time::try_from_nanos_u128(position.as_nanos()).ok_or("Invalid seek time")?;
        self.format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time,
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| e.to_string())?;
        self.codec.reset();
        self.samples = Vec::new().into_iter();
        self.seek_target = Some(position.as_secs_f64());
        Ok(())
    }
}
