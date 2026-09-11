//! Bevy wiring for the procedural music.
//!
//! This plugin owns the [`MusicState`] resource and the control-side systems.
//! Synthesis runs on the audio thread inside [`crate::music::engine`]; here we
//! only decide *what* it should play:
//!
//! - a directory change rebuilds the path-seeded theme and the entry list,
//! - the interaction mode selects the calm or action profile,
//! - every frame, the listener position (camera target or lightcycle cell) is
//!   turned into per-entry voice parameters by the proximity mixer.

use crate::config;
use crate::lightcycle::logic::stable_path_seed;
use crate::lightcycle::{LightcycleState, RunEnvironment};
use crate::load::DirectoryLoaded;
use crate::music::engine::AudioHandle;
use crate::music::proximity::{Listener, NodePoint};
use crate::music::score::{arp_message, base_filter_message, voice_message};
use crate::music::{ArpState, ModeProfile, MusicSfx, MusicTheme, VoiceMixer, full_code};
use crate::state::{InteractionMode, OrbitCameraResource};
use bevy::prelude::*;
use std::f32::consts::TAU;
use std::fmt::Write as _;

pub struct MusicPlugin;

/// Music state shared by the control systems.
#[derive(Resource)]
pub struct MusicState {
    pub handle: AudioHandle,
    pub enabled: bool,
    pub volume: f32,
    pub theme: Option<MusicTheme>,
    pub profile: ModeProfile,
    pub mixer: VoiceMixer,
    /// The evolving melody layer.
    pub arp: ArpState,
    /// The current directory's entries, as musical points on the ground plane.
    pub nodes: Vec<NodePoint>,
    /// Reused buffer for the per-frame `send_msg` payload.
    params: String,
    /// Last payload sent, so an idle listener does not re-send every frame.
    last_params: String,
    /// Time accumulated toward the next parameter publish.
    param_clock: f32,
}

impl MusicState {
    /// Starts the audio thread with the requested initial settings.
    pub fn with_settings(enabled: bool, volume: f32) -> Self {
        let handle = AudioHandle::start();
        let volume = volume.clamp(config::MUSIC_MIN_VOLUME, config::MUSIC_MAX_VOLUME);
        handle.set_volume(volume);
        handle.set_enabled(enabled);
        Self {
            handle,
            enabled,
            volume,
            theme: None,
            profile: ModeProfile::Calm,
            mixer: VoiceMixer::new(),
            arp: ArpState::new(),
            nodes: Vec::new(),
            params: String::new(),
            last_params: String::new(),
            param_clock: 0.0,
        }
    }
}

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        // `MusicState` is inserted by `main` so the CLI options can seed it.
        app.add_message::<MusicSfx>().add_systems(
            Update,
            (
                report_audio_status,
                play_sfx,
                sync_profile_with_mode,
                rebuild_for_directory,
                update_proximity,
                read_music_keys,
                apply_master_gain,
            )
                .chain(),
        );
    }
}

/// Plays the one-shot gameplay effects (crash, turn, beam, portal). The audio
/// thread owns each effect's envelope, so this only forwards the trigger.
fn play_sfx(mut effects: MessageReader<MusicSfx>, music: Res<MusicState>) {
    for sfx in effects.read() {
        music.handle.sfx(*sfx);
    }
}

/// Logs the audio thread's outcome once it settles, so a missing device is
/// explained instead of silently swallowing the music.
fn report_audio_status(mut reported: Local<bool>, music: Res<MusicState>) {
    if *reported {
        return;
    }
    let status = music.handle.status();
    if status.available {
        eprintln!(
            "raptor music: {} at {} Hz, {} channels",
            status.device.as_deref().unwrap_or("unknown device"),
            status.sample_rate,
            status.channels
        );
        *reported = true;
    } else if let Some(message) = &status.message
        && message != "starting"
    {
        eprintln!("raptor music disabled: {message}");
        *reported = true;
    }
}

/// Explorer is calm, Lightcycle is action. Recompiling the base graph is done
/// by the audio thread behind a short fade.
fn sync_profile_with_mode(mode: Res<InteractionMode>, mut music: ResMut<MusicState>) {
    let profile = ModeProfile::from_mode(*mode);
    if profile == music.profile {
        return;
    }
    music.profile = profile;
    music.mixer.clear();
    music.arp.reset();
    if let Some(theme) = &music.theme {
        music
            .handle
            .set_code(&full_code(theme, profile), theme.bpm(profile));
    }
}

/// A loaded directory becomes a theme plus an entry list. Because the theme is
/// seeded from the path, revisiting a folder restores the same piece.
fn rebuild_for_directory(
    mut loaded: MessageReader<DirectoryLoaded>,
    mode: Res<InteractionMode>,
    mut music: ResMut<MusicState>,
) {
    for event in loaded.read() {
        let theme = MusicTheme::from_path(&event.path);
        let profile = ModeProfile::from_mode(*mode);

        music.nodes = event
            .contents
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let position = config::ground_position(node.grid_pos.0, node.grid_pos.1);
                NodePoint {
                    index,
                    x: position.x,
                    z: position.z,
                    node_seed: stable_path_seed(&node.path),
                    is_dir: node.is_dir,
                }
            })
            .collect();

        music.mixer.clear();
        music.arp.reset();
        music
            .handle
            .set_code(&full_code(&theme, profile), theme.bpm(profile));
        music.theme = Some(theme);
        music.profile = profile;
    }
}

/// The listener is the point the camera is looking at in Explorer, and the bike
/// in Lightcycle. Nearby entries swell their voices.
fn update_proximity(
    time: Res<Time>,
    mode: Res<InteractionMode>,
    orbit: Res<OrbitCameraResource>,
    lightcycle: Res<LightcycleState>,
    mut music: ResMut<MusicState>,
) {
    let listener = match *mode {
        InteractionMode::Explorer => Listener {
            x: orbit.target.x,
            z: orbit.target.z,
        },
        InteractionMode::Lightcycle => {
            let Some(run) = &lightcycle.run else {
                return;
            };
            let position = config::ground_position(run.sim.cell.0, run.sim.cell.1);
            Listener {
                x: position.x,
                z: position.z,
            }
        }
    };

    let MusicState {
        handle,
        theme,
        profile,
        mixer,
        arp,
        nodes,
        params,
        last_params,
        param_clock,
        ..
    } = &mut *music;
    let Some(theme) = theme.as_ref() else {
        return;
    };

    let dt = time.delta_secs();
    let targets = mixer.update(listener, nodes, theme, *profile, dt);

    // While a disc-wars ring is open the folder theme stays the seed, but the
    // language tints the arpeggiator: Rust steps harder, Python pumps slower.
    let (arp_rate, arp_gain) = match lightcycle.run.as_ref().map(|run| &run.environment) {
        Some(RunEnvironment::Source { language, .. }) => {
            (language.arp_rate_scale(), language.arp_gain_scale())
        }
        _ => (1.0, 1.0),
    };

    // The evolving melody plus a slow filter sweep on the base voices. The
    // mixer and arp advance every frame, but the payload is only published at a
    // fixed rate so a fast frame loop cannot starve the audio thread.
    let arp_voice = arp.update(dt * arp_rate, theme, *profile);
    let sweep = 0.5 + 0.5 * (time.elapsed_secs() * profile.sweep_rate() * TAU).sin();

    let publish_interval = 1.0 / config::MUSIC_PARAMS_HZ.max(1.0);
    *param_clock += dt;
    if *param_clock < publish_interval {
        return;
    }
    *param_clock = (*param_clock % publish_interval).min(publish_interval);

    params.clear();
    for target in targets {
        let _ = write!(
            params,
            "{}",
            voice_message(
                target.slot,
                target.freq,
                target.cutoff,
                target.gain,
                target.pan
            )
        );
    }
    // A new note opens the filter briefly, so the attack is brighter than the
    // tail.
    let arp_cutoff = if arp_voice.triggered {
        (arp_voice.cutoff * 1.35).min(12_000.0)
    } else {
        arp_voice.cutoff
    };
    let _ = write!(
        params,
        "{}{}",
        arp_message(
            arp_voice.freq,
            arp_cutoff,
            arp_voice.gain * arp_gain,
            arp_voice.pan
        ),
        base_filter_message(theme, *profile, sweep)
    );
    if params != last_params {
        handle.set_voice_params(params);
        last_params.clear();
        last_params.push_str(params);
    }
}

/// Toggle (`N`) and volume (`[` / `]`). Runs in both Explorer and Lightcycle so
/// the music can always be silenced.
fn read_music_keys(keys: Res<ButtonInput<KeyCode>>, mut music: ResMut<MusicState>) {
    if keys.just_pressed(KeyCode::KeyN) {
        music.enabled = !music.enabled;
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        music.volume = (music.volume - config::MUSIC_VOLUME_STEP)
            .clamp(config::MUSIC_MIN_VOLUME, config::MUSIC_MAX_VOLUME);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        music.volume = (music.volume + config::MUSIC_VOLUME_STEP)
            .clamp(config::MUSIC_MIN_VOLUME, config::MUSIC_MAX_VOLUME);
    }
}

/// Pushes the current volume/enabled state to the audio thread. The thread
/// smooths the change, so this is safe to run every frame.
fn apply_master_gain(music: Res<MusicState>) {
    music.handle.set_volume(music.volume);
    music.handle.set_enabled(music.enabled);
}
