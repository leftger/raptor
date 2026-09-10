//! Glicol-backed audio rendering and the realtime audio thread.
//!
//! Glicol is compiled and rendered on a dedicated thread, which also owns the
//! cpal output stream. The Bevy side never touches Glicol or cpal directly; it
//! holds an [`AudioHandle`] that sends [`EngineCommand`]s and updates a few
//! atomics. This keeps the audio graph on one thread regardless of whether
//! `Engine` is `Send`, and keeps the Bevy schedule free of blocking work.
//!
//! Rendered blocks are handed to the cpal callback through a bounded channel;
//! the callback drains them and writes interleaved samples to the device. If no
//! output device exists the handle simply reports why and the rest of the app
//! runs silently.

use super::score::accent_message;
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

/// Block size handed to Glicol. The cpal output callback drains from blocks of
/// this size, so it also bounds the latency of a parameter change.
pub const BLOCK_SIZE: usize = 256;

/// Rendered interleaved blocks buffered ahead of the device callback. Larger is
/// more robust against scheduling jitter but adds latency.
const SAMPLE_QUEUE_BLOCKS: usize = 12;

/// How long the render thread waits while the output queue is full before
/// assuming the device stalled.
const SEND_TIMEOUT: Duration = Duration::from_millis(500);

/// Per-block one-pole coefficient for master gain changes (~115 ms at 256
/// samples / 44.1 kHz).
const GAIN_SMOOTHING: f32 = 0.05;

/// Per-block decay of an accent one-shot; reaches near silence in about a third
/// of a second at 256 samples / 44.1 kHz.
const ACCENT_DECAY: f32 = 0.86;

/// Messages from the Bevy control side to the audio thread.
#[derive(Debug, Clone)]
enum EngineCommand {
    /// Replace the graph and tempo. Used on directory and profile changes.
    SetCode { code: String, bpm: f32 },
    /// A `send_msg` payload updating per-voice parameters.
    Params(String),
    /// Open the shared accent chain briefly.
    Accent { cutoff: f32, gain: f32 },
}

/// What the audio thread is doing, for UI and diagnostics.
#[derive(Debug, Clone)]
pub struct AudioStatus {
    pub available: bool,
    pub sample_rate: u32,
    pub channels: u16,
    pub message: Option<String>,
}

impl AudioStatus {
    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            available: false,
            sample_rate: 0,
            channels: 0,
            message: Some(message.into()),
        }
    }
}

/// Control handle for the music audio thread. Cheap to clone; every clone talks
/// to the same thread.
#[derive(Clone)]
pub struct AudioHandle {
    commands: Option<Sender<EngineCommand>>,
    master_gain: Arc<AtomicU32>,
    enabled: Arc<AtomicBool>,
    status: Arc<Mutex<AudioStatus>>,
}

impl AudioHandle {
    /// Starts the audio thread. Never fails: if there is no usable output device
    /// the returned handle reports it through [`AudioHandle::status`].
    pub fn start() -> Self {
        let master_gain = Arc::new(AtomicU32::new(
            crate::config::MUSIC_DEFAULT_VOLUME.to_bits(),
        ));
        let enabled = Arc::new(AtomicBool::new(true));
        let status = Arc::new(Mutex::new(AudioStatus {
            available: false,
            sample_rate: 0,
            channels: 0,
            message: Some("starting".to_string()),
        }));

        let (commands_tx, commands_rx) = unbounded::<EngineCommand>();
        let (samples_tx, samples_rx) = bounded::<Vec<f32>>(SAMPLE_QUEUE_BLOCKS);

        let gain = master_gain.clone();
        let enabled_flag = enabled.clone();
        let thread_status = status.clone();
        let spawned = thread::Builder::new()
            .name("raptor-music".to_string())
            .spawn(move || {
                audio_thread(
                    commands_rx,
                    samples_tx,
                    samples_rx,
                    gain,
                    enabled_flag,
                    thread_status,
                )
            });

        if let Err(error) = spawned {
            *status.lock().expect("audio status mutex") =
                AudioStatus::unavailable(format!("could not start audio thread: {error}"));
        }

        Self {
            commands: Some(commands_tx),
            master_gain,
            enabled,
            status,
        }
    }

    pub fn status(&self) -> AudioStatus {
        self.status.lock().expect("audio status mutex").clone()
    }

    /// Replaces the Glicol graph and tempo.
    pub fn set_code(&self, code: &str, bpm: f32) {
        if let Some(commands) = &self.commands {
            let _ = commands.send(EngineCommand::SetCode {
                code: code.to_string(),
                bpm,
            });
        }
    }

    /// Sends a prebuilt `send_msg` payload to update voice parameters.
    pub fn set_voice_params(&self, message: &str) {
        if let Some(commands) = &self.commands {
            let _ = commands.send(EngineCommand::Params(message.to_string()));
        }
    }

    /// Triggers a short accent hit on the shared accent chain.
    pub fn accent(&self, cutoff: f32, gain: f32) {
        if let Some(commands) = &self.commands {
            let _ = commands.send(EngineCommand::Accent { cutoff, gain });
        }
    }

    pub fn set_volume(&self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        self.master_gain.store(volume.to_bits(), Ordering::Relaxed);
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}

/// Owns the Glicol engine and the cpal stream for the lifetime of the app.
fn audio_thread(
    commands: Receiver<EngineCommand>,
    samples_tx: Sender<Vec<f32>>,
    samples_rx: Receiver<Vec<f32>>,
    master_gain: Arc<AtomicU32>,
    enabled: Arc<AtomicBool>,
    status: Arc<Mutex<AudioStatus>>,
) {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let Some(device) = host.default_output_device() else {
        *status.lock().expect("audio status mutex") = AudioStatus::unavailable("no output device");
        return;
    };

    let Some(supported) = pick_output_config(&device) else {
        *status.lock().expect("audio status mutex") =
            AudioStatus::unavailable("device has no f32 output configuration");
        return;
    };

    let sample_rate = supported.sample_rate();
    let channels = supported.channels();
    let stream_config: cpal::StreamConfig = supported.into();

    let mut engine = glicol::Engine::<BLOCK_SIZE>::new();
    engine.set_sr(sample_rate as usize);

    let mut queue: Vec<f32> = Vec::new();
    let mut cursor = 0usize;
    let data_callback = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        let mut index = 0;
        while index < data.len() {
            if cursor >= queue.len() {
                match samples_rx.try_recv() {
                    Ok(block) => {
                        queue = block;
                        cursor = 0;
                    }
                    Err(_) => {
                        for sample in &mut data[index..] {
                            *sample = 0.0;
                        }
                        break;
                    }
                }
            }
            data[index] = queue[cursor];
            cursor += 1;
            index += 1;
        }
    };

    let stream = match device.build_output_stream(
        stream_config,
        data_callback,
        |error| eprintln!("raptor music: output stream error: {error}"),
        None,
    ) {
        Ok(stream) => stream,
        Err(error) => {
            *status.lock().expect("audio status mutex") =
                AudioStatus::unavailable(format!("could not open output stream: {error}"));
            return;
        }
    };

    if let Err(error) = stream.play() {
        *status.lock().expect("audio status mutex") =
            AudioStatus::unavailable(format!("could not start output stream: {error}"));
        return;
    }

    *status.lock().expect("audio status mutex") = AudioStatus {
        available: true,
        sample_rate,
        channels,
        message: None,
    };

    let mut smoothed_gain = 0.0_f32;
    // A room or profile change compiles a second graph and equal-power
    // crossfades to it, so the swap blends instead of dipping to silence.
    let crossfade_blocks = ((crate::config::MUSIC_CROSSFADE_SECONDS * sample_rate as f32)
        / BLOCK_SIZE as f32)
        .round()
        .max(1.0) as u32;
    let mut incoming: Option<glicol::Engine<BLOCK_SIZE>> = None;
    let mut crossfade_age: u32 = 0;
    let mut last_params = String::new();

    // Accent one-shots decay on the audio thread.
    let mut accent_gain = 0.0_f32;
    let mut accent_cutoff = 1800.0_f32;

    loop {
        while let Ok(command) = commands.try_recv() {
            match command {
                EngineCommand::SetCode { code, bpm } => {
                    if let Some(next) = compile_engine(&code, bpm, sample_rate, &last_params) {
                        incoming = Some(next);
                        crossfade_age = 0;
                    }
                }
                EngineCommand::Params(message) => {
                    engine.send_msg(&message);
                    if let Some(next) = incoming.as_mut() {
                        next.send_msg(&message);
                    }
                    if last_params != message {
                        last_params.clear();
                        last_params.push_str(&message);
                    }
                }
                EngineCommand::Accent { cutoff, gain } => {
                    accent_cutoff = cutoff;
                    accent_gain = gain;
                }
            }
        }

        if accent_gain > 0.0005 {
            let message = accent_message(accent_cutoff, accent_gain);
            engine.send_msg(&message);
            if let Some(next) = incoming.as_mut() {
                next.send_msg(&message);
            }
            accent_gain *= ACCENT_DECAY;
        }

        let target_gain = if enabled.load(Ordering::Relaxed) {
            f32::from_bits(master_gain.load(Ordering::Relaxed))
        } else {
            0.0
        };
        smoothed_gain += (target_gain - smoothed_gain) * GAIN_SMOOTHING;

        let mut interleaved = vec![0.0_f32; BLOCK_SIZE * channels as usize];
        {
            let block = engine.next_block(Vec::new());
            write_interleaved(block, channels as usize, smoothed_gain, &mut interleaved);
        }

        let mut promote = false;
        if let Some(next) = incoming.as_mut() {
            let progress = (crossfade_age as f32 / crossfade_blocks as f32).clamp(0.0, 1.0);
            let (fade_in, fade_out) = (progress * std::f32::consts::FRAC_PI_2).sin_cos();

            for sample in interleaved.iter_mut() {
                *sample *= fade_out;
            }

            let mut incoming_block = vec![0.0_f32; interleaved.len()];
            {
                let block = next.next_block(Vec::new());
                write_interleaved(
                    block,
                    channels as usize,
                    smoothed_gain * fade_in,
                    &mut incoming_block,
                );
            }
            for (sample, add) in interleaved.iter_mut().zip(incoming_block) {
                *sample += add;
            }

            crossfade_age += 1;
            promote = crossfade_age >= crossfade_blocks;
        }
        if promote && let Some(done) = incoming.take() {
            engine = done;
            crossfade_age = 0;
        }

        if samples_tx.send_timeout(interleaved, SEND_TIMEOUT).is_err() {
            break;
        }
    }
}

/// Builds a Glicol graph for a code string, applying the last known voice
/// parameters so a freshly promoted graph is immediately in tune.
fn compile_engine(
    code: &str,
    bpm: f32,
    sample_rate: u32,
    params: &str,
) -> Option<glicol::Engine<BLOCK_SIZE>> {
    let mut engine = glicol::Engine::<BLOCK_SIZE>::new();
    engine.set_sr(sample_rate as usize);
    engine.set_bpm(bpm);
    if let Err(error) = engine.update_with_code(code) {
        eprintln!("raptor music: could not compile graph: {error:?}");
        return None;
    }
    if !params.is_empty() {
        engine.send_msg(params);
    }
    Some(engine)
}

/// Picks a stereo/mono f32 output configuration, preferring 48 kHz.
fn pick_output_config(
    device: &impl cpal::traits::DeviceTrait,
) -> Option<cpal::SupportedStreamConfig> {
    if let Ok(configs) = device.supported_output_configs() {
        for config in configs {
            if config.sample_format() == cpal::SampleFormat::F32 {
                return Some(
                    config
                        .try_with_sample_rate(48_000)
                        .unwrap_or_else(|| config.with_max_sample_rate()),
                );
            }
        }
    }

    device
        .default_output_config()
        .ok()
        .filter(|config| config.sample_format() == cpal::SampleFormat::F32)
}

/// Interleaves Glicol's stereo buffers into the device's channel layout and
/// applies the master gain.
///
/// Generic over the buffer type (`glicol::Buffer` is not publicly nameable),
/// which derefs to `[f32]`.
fn write_interleaved<B>(block: &[B], channels: usize, gain: f32, out: &mut [f32])
where
    B: std::ops::Deref<Target = [f32]>,
{
    let left = block.first().map(|buffer| &buffer[..]).unwrap_or(&[]);
    let right = block.get(1).map(|buffer| &buffer[..]).unwrap_or(left);
    for frame in 0..BLOCK_SIZE {
        let l = left.get(frame).copied().unwrap_or(0.0) * gain;
        let r = right.get(frame).copied().unwrap_or(0.0) * gain;
        let base = frame * channels;
        if channels == 1 {
            out[base] = (l + r) * 0.5;
        } else {
            out[base] = l;
            out[base + 1] = r;
            for sample in out[base + 2..base + channels].iter_mut() {
                *sample = 0.0;
            }
        }
    }
}

/// Renders `blocks` audio blocks of Glicol `code` and returns one `Vec` per
/// output channel, each holding `blocks * BLOCK_SIZE` samples.
///
/// Glicol's destination is stereo, so the usual result is two channels.
#[cfg(test)]
pub fn render_offline(code: &str, blocks: usize) -> Result<Vec<Vec<f32>>, String> {
    let mut engine = glicol::Engine::<BLOCK_SIZE>::new();
    engine
        .update_with_code(code)
        .map_err(|error| format!("{error:?}"))?;

    let mut channels: Vec<Vec<f32>> = Vec::new();
    for _ in 0..blocks {
        let block = engine.next_block(Vec::new());
        if channels.len() != block.len() {
            channels = (0..block.len())
                .map(|_| Vec::with_capacity(blocks * BLOCK_SIZE))
                .collect();
        }
        for (channel, buffer) in block.iter().enumerate() {
            channels[channel].extend_from_slice(buffer);
        }
    }

    Ok(channels)
}

/// Peak absolute sample across every channel.
#[cfg(test)]
pub fn peak(channels: &[Vec<f32>]) -> f32 {
    channels
        .iter()
        .flatten()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_stereo_blocks_of_the_requested_length() {
        let channels = render_offline("o: saw 440 >> mul 0.2", 4).unwrap();
        assert_eq!(channels.len(), 2, "glicol destination should be stereo");
        for channel in &channels {
            assert_eq!(channel.len(), 4 * BLOCK_SIZE);
        }
    }

    #[test]
    fn a_saw_oscillator_is_audible_and_bounded() {
        let channels = render_offline("o: saw 440 >> mul 0.2", 4).unwrap();
        let peak = peak(&channels);
        assert!(peak > 0.01, "expected audible output, peak was {peak}");
        assert!(peak <= 1.0, "unclipped saw should stay within [-1, 1]");
    }

    #[test]
    fn stereo_mixdown_respects_the_device_channel_count() {
        let block = [vec![1.0_f32; BLOCK_SIZE], vec![0.5_f32; BLOCK_SIZE]];

        let mut stereo = vec![0.0; BLOCK_SIZE * 2];
        write_interleaved(&block, 2, 1.0, &mut stereo);
        assert_eq!(stereo[0], 1.0);
        assert_eq!(stereo[1], 0.5);

        let mut mono = vec![0.0; BLOCK_SIZE];
        write_interleaved(&block, 1, 1.0, &mut mono);
        assert_eq!(mono[0], 0.75);
    }

    #[test]
    fn startup_degrades_gracefully_without_a_device() {
        let handle = AudioHandle::start();
        let mut status = handle.status();
        for _ in 0..100 {
            status = handle.status();
            if status.available || status.message.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            status.available || status.message.is_some(),
            "audio should either play or explain why it cannot"
        );
    }
}
