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

use super::score::{sfx_message, sfx_silence_message};
use super::sfx::MusicSfx;
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
/// more robust against scheduling jitter but adds latency. At 256 samples this
/// is roughly a fifth of a second of headroom.
const SAMPLE_QUEUE_BLOCKS: usize = 32;

/// Silent blocks pushed into the queue before the stream starts, so the first
/// device callbacks always have data.
const PREBUFFER_BLOCKS: usize = 8;

/// How long the render thread waits while the output queue is full before
/// assuming the device stalled.
const SEND_TIMEOUT: Duration = Duration::from_millis(500);

/// Sample rate asked of the output device when it supports one.
const PREFERRED_SAMPLE_RATE: u32 = 48_000;

/// Below this the graph's filter cutoffs start crowding Nyquist and the mix
/// sounds like a phone call, so such a configuration is only a last resort.
const MIN_MUSIC_SAMPLE_RATE: u32 = 44_100;

/// Per-block one-pole coefficient for master gain changes (~115 ms at 256
/// samples / 44.1 kHz).
const GAIN_SMOOTHING: f32 = 0.05;

/// Rare control messages. Per-frame voice parameters do **not** go through this
/// channel: they use a coalescing mailbox so a fast frame loop cannot flood the
/// audio thread.
#[derive(Debug, Clone)]
enum EngineCommand {
    /// Replace the graph and tempo. Used on directory and profile changes.
    SetCode { code: String, bpm: f32 },
    /// Trigger a one-shot sound effect.
    Sfx(MusicSfx),
}

/// What the audio thread is doing, for UI and diagnostics.
#[derive(Debug, Clone)]
pub struct AudioStatus {
    pub available: bool,
    /// Name of the output device in use. Worth reporting: the music goes to the
    /// system default, which is not always the speakers the listener expects.
    pub device: Option<String>,
    pub sample_rate: u32,
    pub channels: u16,
    pub message: Option<String>,
}

impl AudioStatus {
    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            available: false,
            device: None,
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
    /// Coalescing mailbox for per-frame voice parameters: only the newest
    /// payload is kept, so a fast frame loop can never backlog the audio thread.
    params: Arc<Mutex<Option<String>>>,
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
            device: None,
            sample_rate: 0,
            channels: 0,
            message: Some("starting".to_string()),
        }));

        let (commands_tx, commands_rx) = unbounded::<EngineCommand>();
        let (samples_tx, samples_rx) = bounded::<Vec<f32>>(SAMPLE_QUEUE_BLOCKS);
        let params = Arc::new(Mutex::new(None::<String>));

        let gain = master_gain.clone();
        let enabled_flag = enabled.clone();
        let thread_status = status.clone();
        let thread_params = params.clone();
        let spawned = thread::Builder::new()
            .name("raptor-music".to_string())
            .spawn(move || {
                audio_thread(
                    commands_rx,
                    samples_tx,
                    samples_rx,
                    thread_params,
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
            params,
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

    /// Publishes the latest voice-parameter payload. Replaces any unpublished
    /// payload, so the audio thread always applies one coalesced update per
    /// block no matter how fast frames are produced.
    pub fn set_voice_params(&self, message: &str) {
        store_latest(&self.params, message);
    }

    /// Triggers a one-shot sound effect. Retriggering an effect that is still
    /// sounding restarts it from the top.
    pub fn sfx(&self, sfx: MusicSfx) {
        if let Some(commands) = &self.commands {
            let _ = commands.send(EngineCommand::Sfx(sfx));
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

/// Stores the newest parameter payload, reusing the existing allocation. Only
/// the latest value matters, so older unpublished payloads are simply replaced.
fn store_latest(slot: &Mutex<Option<String>>, message: &str) {
    if let Ok(mut slot) = slot.lock() {
        match slot.as_mut() {
            Some(existing) => {
                existing.clear();
                existing.push_str(message);
            }
            None => *slot = Some(message.to_string()),
        }
    }
}

/// A sound effect currently sounding, with how far it has run.
struct ActiveSfx {
    sfx: MusicSfx,
    elapsed: f32,
}

/// Owns the Glicol engine and the cpal stream for the lifetime of the app.
fn audio_thread(
    commands: Receiver<EngineCommand>,
    samples_tx: Sender<Vec<f32>>,
    samples_rx: Receiver<Vec<f32>>,
    params: Arc<Mutex<Option<String>>>,
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

    let device_name = device.to_string();

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

    // Cushion the first device callbacks so startup cannot underrun.
    for _ in 0..PREBUFFER_BLOCKS {
        let _ = samples_tx.try_send(vec![0.0_f32; BLOCK_SIZE * channels as usize]);
    }

    if let Err(error) = stream.play() {
        *status.lock().expect("audio status mutex") =
            AudioStatus::unavailable(format!("could not start output stream: {error}"));
        return;
    }

    *status.lock().expect("audio status mutex") = AudioStatus {
        available: true,
        device: Some(device_name),
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

    // One-shot effects advance on the audio thread, so their envelopes are
    // sample-accurate and independent of the frame rate.
    let block_seconds = BLOCK_SIZE as f32 / sample_rate as f32;
    let mut active_sfx: Vec<ActiveSfx> = Vec::new();
    // Reused scratch for the crossfade's incoming graph.
    let mut incoming_block = vec![0.0_f32; BLOCK_SIZE * channels as usize];

    loop {
        while let Ok(command) = commands.try_recv() {
            match command {
                EngineCommand::SetCode { code, bpm } => {
                    if let Some(next) = compile_engine(&code, bpm, sample_rate, &last_params) {
                        incoming = Some(next);
                        crossfade_age = 0;
                    }
                }
                EngineCommand::Sfx(sfx) => {
                    match active_sfx.iter_mut().find(|active| active.sfx == sfx) {
                        Some(active) => active.elapsed = 0.0,
                        None => active_sfx.push(ActiveSfx { sfx, elapsed: 0.0 }),
                    }
                }
            }
        }

        // Apply at most one parameter update per block. The mailbox holds only
        // the newest payload, so frame rate cannot backlog this thread.
        let latest = params.lock().ok().and_then(|mut slot| slot.take());
        if let Some(message) = latest {
            engine.send_msg(&message);
            if let Some(next) = incoming.as_mut() {
                next.send_msg(&message);
            }
            last_params = message;
        }

        active_sfx.retain_mut(|active| {
            let progress = active.elapsed / active.sfx.duration();
            let message = if progress >= 1.0 {
                sfx_silence_message(active.sfx)
            } else {
                sfx_message(active.sfx, active.sfx.voice(progress))
            };
            engine.send_msg(&message);
            if let Some(next) = incoming.as_mut() {
                next.send_msg(&message);
            }
            active.elapsed += block_seconds;
            progress < 1.0
        });

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

            {
                let block = next.next_block(Vec::new());
                write_interleaved(
                    block,
                    channels as usize,
                    smoothed_gain * fade_in,
                    &mut incoming_block,
                );
            }
            for (sample, add) in interleaved.iter_mut().zip(incoming_block.iter()) {
                *sample += *add;
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

/// Picks the best f32 output configuration the device offers.
///
/// Every supported range is considered, not just the first: a device can
/// advertise a telephony mode ahead of its music mode (a Bluetooth headset
/// offering 16 kHz mono *and* 44.1 kHz), and taking the first match leaves the
/// music rendering at phone quality.
fn pick_output_config(
    device: &impl cpal::traits::DeviceTrait,
) -> Option<cpal::SupportedStreamConfig> {
    let mut best: Option<cpal::SupportedStreamConfig> = None;
    if let Ok(ranges) = device.supported_output_configs() {
        for range in ranges {
            if range.sample_format() != cpal::SampleFormat::F32 {
                continue;
            }
            let config = range
                .try_with_sample_rate(PREFERRED_SAMPLE_RATE)
                .unwrap_or_else(|| range.with_max_sample_rate());
            if best
                .as_ref()
                .is_none_or(|current| config_rank(&config) > config_rank(current))
            {
                best = Some(config);
            }
        }
    }

    best.or_else(|| {
        device
            .default_output_config()
            .ok()
            .filter(|config| config.sample_format() == cpal::SampleFormat::F32)
    })
}

/// Ranks an output configuration for music, highest wins: a music-grade sample
/// rate first, then stereo over mono, then the rate nearest
/// [`PREFERRED_SAMPLE_RATE`]. Rate leads because 44.1 kHz mono carries the piece
/// better than 16 kHz stereo, and a low rate also drags the filter cutoffs down
/// toward Nyquist.
fn config_rank(config: &cpal::SupportedStreamConfig) -> (bool, u16, i64) {
    let rate = config.sample_rate();
    (
        rate >= MIN_MUSIC_SAMPLE_RATE,
        config.channels().min(2),
        -i64::from(rate.abs_diff(PREFERRED_SAMPLE_RATE)),
    )
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
    fn voice_params_coalesce_to_the_newest_payload() {
        let slot = Mutex::new(None::<String>);
        store_latest(&slot, "first");
        store_latest(&slot, "second");
        let slot = slot.lock().expect("params mutex");
        assert_eq!(
            slot.as_deref(),
            Some("second"),
            "the mailbox must keep only the newest payload"
        );
    }

    /// A Bluetooth headset can advertise its 16 kHz mono call mode before its
    /// music mode, which is how the mix ended up at phone quality.
    #[test]
    fn a_music_rate_outranks_an_earlier_telephony_config() {
        let telephony = cpal::SupportedStreamConfig::new(
            1,
            16_000,
            cpal::SupportedBufferSize::Unknown,
            cpal::SampleFormat::F32,
        );
        let music = cpal::SupportedStreamConfig::new(
            1,
            44_100,
            cpal::SupportedBufferSize::Unknown,
            cpal::SampleFormat::F32,
        );
        let stereo = cpal::SupportedStreamConfig::new(
            2,
            48_000,
            cpal::SupportedBufferSize::Unknown,
            cpal::SampleFormat::F32,
        );

        assert!(config_rank(&music) > config_rank(&telephony));
        assert!(config_rank(&stereo) > config_rank(&music));
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
