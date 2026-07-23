use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::Mutex;
use std::thread;

static SPECTRUM: Mutex<[f32; 11]> = Mutex::new([0.24; 11]);

pub fn get_audio_spectrum() -> [f32; 11] {
    *SPECTRUM.lock().unwrap()
}

pub fn start_monitor() {
    thread::spawn(|| {
        let host = cpal::default_host();

        let device = match host.default_output_device() {
            Some(d) => d,
            None => return,
        };

        let config = match device.default_output_config() {
            Ok(c) => c,
            Err(_) => return,
        };

        let err_fn = |err| eprintln!("Audio capture error: {}", err);
        let sample_format = config.sample_format();
        let config: cpal::StreamConfig = config.into();
        let channels = config.channels;

        let stream = match sample_format {
            cpal::SampleFormat::F32 => device.build_input_stream(
                config.clone(),
                move |data: &[f32], _: &_| process_data(data, channels),
                err_fn,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_input_stream(
                config.clone(),
                move |data: &[i16], _: &_| {
                    let f32_data: Vec<f32> =
                        data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                    process_data(&f32_data, channels);
                },
                err_fn,
                None,
            ),
            _ => return,
        };

        if let Ok(stream) = stream {
            let _ = stream.play();
            loop {
                thread::sleep(std::time::Duration::from_secs(3600));
            }
        }
    });
}

fn process_data(data: &[f32], channels: u16) {
    if data.is_empty() {
        return;
    }

    let mut mono = Vec::with_capacity(data.len() / channels as usize);
    for chunk in data.chunks(channels as usize) {
        let sum: f32 = chunk.iter().sum();
        mono.push(sum / channels as f32);
    }

    let n = mono.len();
    if n < 128 {
        return;
    }

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n);

    let mut buffer: Vec<Complex<f32>> = mono
        .iter()
        .enumerate()
        .map(|(i, &val)| {
            let multiplier =
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos());
            Complex {
                re: val * multiplier,
                im: 0.0,
            }
        })
        .collect();

    fft.process(&mut buffer);

    let mut bins = [0.0_f32; 11];
    let half_n = n / 2;

    for i in 1..half_n {
        let mag = (buffer[i].re.powi(2) + buffer[i].im.powi(2)).sqrt();
        let position = ((i as f32).ln() / (half_n as f32).ln()).clamp(0.0, 0.999);
        let bin_idx = (position * bins.len() as f32) as usize;

        if mag > bins[bin_idx] {
            bins[bin_idx] = mag;
        }
    }

    let mut final_spectrum = [0.24_f32; 11];
    let eq_weights = [1.8, 1.65, 1.5, 1.45, 1.55, 1.8, 2.15, 2.75, 3.5, 4.35, 5.15];
    let base_gain = 9.2;

    for i in 0..11 {
        let energy = bins[i] * eq_weights[i] * base_gain;

        let scaled = ((energy + 1.0).log10() * 0.42) + 0.24;

        final_spectrum[i] = scaled.clamp(0.18, 1.42);
    }

    if let Ok(mut spec) = SPECTRUM.lock() {
        for i in 0..11 {
            let rise = final_spectrum[i] > spec[i];
            let old_weight = if rise { 0.18 } else { 0.62 };
            spec[i] = spec[i] * old_weight + final_spectrum[i] * (1.0 - old_weight);
        }
    }
}
