//! Glicol source generation.
//!
//! The graph has two parts:
//!
//! - **Base voices** derived from the directory theme (a calm pad pair or an
//!   action bass/lead pair).
//! - **A fixed bank of per-entry voices**, compiled once at [`MAX_VOICES`] width
//!   and left silent. The proximity mixer then only changes their numeric
//!   parameters through `send_msg`, so the graph is never rebuilt while the
//!   listener moves.
//!
//! The output chain mixes the base voices with the whole bank:
//!
//! ```text
//! ~v0: saw 220.00 >> lpf 500.0 0.7 >> mul 0.0 >> pan 0.0;
//! ...
//! o: mix ~pad0 ~pad1 ~v0 ~v1 ...;
//! ```
//!
//! Voice chain node positions are fixed: `0` = oscillator, `1` = low-pass
//! (param 0 cutoff), `2` = gain, `3` = pan. See [`voice_message`].

use super::sfx::{MusicSfx, SfxVoice};
use super::theme::{ModeProfile, MusicTheme};
use crate::config;
use std::fmt::Write;

/// Width of the compiled voice bank. Always the hard cap so a profile switch
/// never needs to rebuild the graph.
pub const MAX_VOICES: usize = config::MUSIC_MAX_VOICES;

/// Chain name of one proximity voice as it appears in the Glicol graph. The
/// leading `~` marks it as a reference chain (not sent to the DAC on its own)
/// and is part of the key used by `send_msg`.
pub fn voice_chain_name(slot: usize) -> String {
    format!("~v{slot}")
}

/// Base chain names mixed into the output for a profile.
pub fn base_refs(profile: ModeProfile) -> &'static [&'static str] {
    match profile {
        ModeProfile::Calm => &["~pad0", "~pad1"],
        ModeProfile::Action => &["~bass", "~lead"],
    }
}

/// Base cutoffs for the calm pads, shared by the graph and the filter sweep.
fn calm_pad_cutoffs(theme: &MusicTheme) -> (f32, f32) {
    let cutoff = (theme.root_hz() * 6.0).clamp(400.0, 2400.0);
    (cutoff, (cutoff * 1.2).clamp(400.0, 3000.0))
}

/// Base cutoffs for the action bass and lead.
fn action_base_cutoffs(theme: &MusicTheme) -> (f32, f32) {
    let bass = theme.degree_hz(0, -1);
    let lead = theme.degree_hz(4, 1);
    (
        (bass * 8.0).clamp(500.0, 2600.0),
        (lead * 6.0).clamp(700.0, 3200.0),
    )
}

/// Definitions for the profile's base voices plus the shared arpeggiator and
/// sound-effect chains.
pub fn base_voices(theme: &MusicTheme, profile: ModeProfile) -> String {
    let wave = theme.family.waveform();
    let mut code = String::new();

    // Shared effects, all silent until triggered. See [`MusicSfx::declaration`].
    for sfx in MusicSfx::ALL {
        let _ = writeln!(code, "{}", sfx.declaration());
    }

    // Shared evolving melody, retriggered from the control side. Layout:
    // 0 osc, 1 low-pass, 2 gain, 3 pan. See [`arp_message`].
    let _ = writeln!(
        code,
        "~arp: {wave} {:.2} >> lpf {:.1} 0.6 >> mul 0.0 >> pan 0.0;",
        theme.degree_hz(0, 0),
        theme.node_cutoff(theme.seed)
    );

    match profile {
        ModeProfile::Calm => {
            let root = theme.root_hz();
            let fifth = theme.degree_hz(4, 0);
            let (c0, c1) = calm_pad_cutoffs(theme);
            let _ = writeln!(
                code,
                "~pad0: {wave} {root:.2} >> lpf {c0:.1} 0.7 >> mul {:.3} >> pan -0.25;",
                config::MUSIC_CALM_PAD_GAIN
            );
            let _ = writeln!(
                code,
                "~pad1: {wave} {fifth:.2} >> lpf {c1:.1} 0.7 >> mul {:.3} >> pan 0.25;",
                config::MUSIC_CALM_PAD_GAIN * 0.8
            );
        }
        ModeProfile::Action => {
            let bass = theme.degree_hz(0, -1);
            let lead = theme.degree_hz(4, 1);
            let (c0, c1) = action_base_cutoffs(theme);
            // Tempo-synced tremolo in 0..1, so the arrangement pumps.
            let pump_hz = (theme.bpm(ModeProfile::Action) / 60.0) * config::MUSIC_ACTION_PUMP_RATE;
            let half_depth = config::MUSIC_ACTION_PUMP_DEPTH / 2.0;
            let _ = writeln!(
                code,
                "~pump: sin {pump_hz:.3} >> mul {half_depth:.3} >> add {:.3};",
                1.0 - half_depth
            );
            let _ = writeln!(
                code,
                "~bass: saw {bass:.2} >> lpf {c0:.1} 0.8 >> mul {:.3} >> mul ~pump >> pan -0.15;",
                config::MUSIC_ACTION_BASS_GAIN
            );
            let _ = writeln!(
                code,
                "~lead: squ {lead:.2} >> lpf {c1:.1} 0.7 >> mul {:.3} >> mul ~pump >> pan 0.2;",
                config::MUSIC_ACTION_LEAD_GAIN
            );
        }
    }
    code
}

/// Definitions for the silent per-entry voice bank.
pub fn voice_bank(theme: &MusicTheme) -> String {
    let wave = theme.family.waveform();
    let mut code = String::new();
    for slot in 0..MAX_VOICES {
        let freq = theme.degree_hz(slot as u32, -1);
        let cutoff = theme.node_cutoff(theme.seed ^ slot as u64);
        let _ = writeln!(
            code,
            "~v{slot}: {wave} {freq:.2} >> lpf {cutoff:.1} 0.7 >> mul 0.0 >> pan 0.0;"
        );
    }
    code
}

/// The single `o:` chain mixing the profile's base voices, the sound effects,
/// the arpeggiator, and the whole bank.
pub fn output_chain(profile: ModeProfile) -> String {
    let mut code = String::from("o: mix");
    for sfx in MusicSfx::ALL {
        let _ = write!(code, " {}", sfx.chain());
    }
    code.push_str(" ~arp");
    for name in base_refs(profile) {
        let _ = write!(code, " {name}");
    }
    for slot in 0..MAX_VOICES {
        let _ = write!(code, " {}", voice_chain_name(slot));
    }
    code.push_str(";\n");
    code
}

/// Complete Glicol program for one directory and profile.
pub fn full_code(theme: &MusicTheme, profile: ModeProfile) -> String {
    let mut code = String::new();
    code.push_str(&base_voices(theme, profile));
    code.push_str(&voice_bank(theme));
    code.push_str(&output_chain(profile));
    code
}

/// A Glicol `send_msg` payload that sets one voice's frequency, filter cutoff,
/// gain, and pan. Matches the fixed node layout produced by [`voice_bank`].
pub fn voice_message(slot: usize, freq: f32, cutoff: f32, gain: f32, pan: f32) -> String {
    format!(
        "~v{slot},0,0,{freq:.3};~v{slot},1,0,{cutoff:.3};~v{slot},2,0,{gain:.4};~v{slot},3,0,{pan:.3};"
    )
}

/// A Glicol `send_msg` payload that drives one sound effect's chain. Matches the
/// layout produced by [`base_voices`]: 0 = oscillator, 1 = low-pass cutoff,
/// 2 = gain, 3 = pan.
///
/// The noise-based effects have no frequency to set, so their oscillator node is
/// left alone (see [`MusicSfx::uses_pitch`]).
pub fn sfx_message(sfx: MusicSfx, voice: SfxVoice) -> String {
    let chain = sfx.chain();
    let mut message = String::new();
    if sfx.uses_pitch() {
        let _ = write!(message, "{chain},0,0,{:.3};", voice.freq);
    }
    let _ = write!(
        message,
        "{chain},1,0,{:.3};{chain},2,0,{:.4};{chain},3,0,{:.3};",
        voice.cutoff, voice.gain, voice.pan
    );
    message
}

/// A `send_msg` payload that closes a sound effect's chain once it has run out.
pub fn sfx_silence_message(sfx: MusicSfx) -> String {
    format!("{},2,0,0.0000;", sfx.chain())
}

/// A `send_msg` payload that plays one arpeggiator note. Layout: 0 oscillator,
/// 1 low-pass, 2 gain, 3 pan.
pub fn arp_message(freq: f32, cutoff: f32, gain: f32, pan: f32) -> String {
    format!("~arp,0,0,{freq:.3};~arp,1,0,{cutoff:.3};~arp,2,0,{gain:.4};~arp,3,0,{pan:.3};")
}

/// A `send_msg` payload that sweeps the profile's base-voice filters. `sweep`
/// runs 0..1, closing to ~0.65x and opening to ~1.65x of the base cutoff.
pub fn base_filter_message(theme: &MusicTheme, profile: ModeProfile, sweep: f32) -> String {
    let open = 0.65 + sweep.clamp(0.0, 1.0);
    let (c0, c1) = match profile {
        ModeProfile::Calm => calm_pad_cutoffs(theme),
        ModeProfile::Action => action_base_cutoffs(theme),
    };
    let c0 = (c0 * open).clamp(config::MUSIC_VOICE_CUTOFF_MIN, 12_000.0);
    let c1 = (c1 * open).clamp(config::MUSIC_VOICE_CUTOFF_MIN, 12_000.0);
    match profile {
        ModeProfile::Calm => format!("~pad0,1,0,{c0:.1};~pad1,1,0,{c1:.1};"),
        ModeProfile::Action => format!("~bass,1,0,{c0:.1};~lead,1,0,{c1:.1};"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::engine::render_offline;

    fn theme() -> MusicTheme {
        MusicTheme::from_seed(0x0bad_c0de_1234_5678)
    }

    #[test]
    fn the_bank_is_always_compiled_at_full_width() {
        let code = voice_bank(&theme());
        for slot in 0..MAX_VOICES {
            assert!(code.contains(&format!("~v{slot}:")), "missing slot {slot}");
        }
    }

    #[test]
    fn output_mixes_base_and_every_voice() {
        let code = output_chain(ModeProfile::Action);
        assert!(code.starts_with("o: mix"));
        for name in base_refs(ModeProfile::Action) {
            assert!(code.contains(name));
        }
        assert!(code.contains("~v7"));
    }

    #[test]
    fn profiles_render_different_base_voices() {
        let calm = full_code(&theme(), ModeProfile::Calm);
        let action = full_code(&theme(), ModeProfile::Action);
        assert!(calm.contains("~pad0"));
        assert!(action.contains("~bass"));
        assert_ne!(calm, action);
    }

    #[test]
    fn only_action_pumps_and_both_profiles_carry_the_shared_chains() {
        let calm = full_code(&theme(), ModeProfile::Calm);
        let action = full_code(&theme(), ModeProfile::Action);
        assert!(!calm.contains("~pump"));
        assert!(action.contains("~pump"));
        for code in [&calm, &action] {
            for sfx in MusicSfx::ALL {
                let chain = sfx.chain();
                assert!(code.contains(&format!("{chain}:")), "missing {chain}");
            }
            assert!(code.contains("~arp"));
        }
    }

    #[test]
    fn arp_message_addresses_the_fixed_node_layout() {
        let message = arp_message(330.0, 1200.0, 0.08, -0.3);
        assert!(message.contains("~arp,0,0,330.000;"));
        assert!(message.contains("~arp,1,0,1200.000;"));
        assert!(message.contains("~arp,2,0,0.0800;"));
        assert!(message.contains("~arp,3,0,-0.300;"));
    }

    #[test]
    fn base_filter_sweep_targets_the_profile_base() {
        let calm = base_filter_message(&theme(), ModeProfile::Calm, 0.5);
        assert!(calm.contains("~pad0,1,0,"));
        assert!(calm.contains("~pad1,1,0,"));
        let action = base_filter_message(&theme(), ModeProfile::Action, 0.5);
        assert!(action.contains("~bass,1,0,"));
        assert!(action.contains("~lead,1,0,"));
    }

    #[test]
    fn sfx_messages_address_the_fixed_node_layout() {
        let voice = SfxVoice {
            freq: 880.0,
            cutoff: 1400.0,
            gain: 0.375,
            pan: -0.1,
        };

        let turn = sfx_message(MusicSfx::Turn, voice);
        assert!(turn.contains("~sfx_turn,0,0,880.000;"));
        assert!(turn.contains("~sfx_turn,1,0,1400.000;"));
        assert!(turn.contains("~sfx_turn,2,0,0.3750;"));
        assert!(turn.contains("~sfx_turn,3,0,-0.100;"));

        // The crash's first node is noise, which has no frequency to set.
        let crash = sfx_message(MusicSfx::Crash, voice);
        assert!(!crash.contains("~sfx_crash,0,0,"));
        assert!(crash.contains("~sfx_crash,2,0,0.3750;"));

        assert_eq!(
            sfx_silence_message(MusicSfx::Crash),
            "~sfx_crash,2,0,0.0000;"
        );
    }

    #[test]
    fn every_sfx_chain_is_declared_and_mixed() {
        let code = full_code(&theme(), ModeProfile::Calm);
        let output = output_chain(ModeProfile::Calm);
        for sfx in MusicSfx::ALL {
            let chain = sfx.chain();
            assert!(code.contains(&format!("{chain}:")), "{chain} not declared");
            assert!(output.contains(chain), "{chain} not mixed into the output");
        }
    }

    #[test]
    fn code_is_deterministic_for_the_same_theme() {
        assert_eq!(
            full_code(&theme(), ModeProfile::Calm),
            full_code(&theme(), ModeProfile::Calm)
        );
    }

    #[test]
    fn voice_message_addresses_the_fixed_node_layout() {
        let message = voice_message(3, 220.0, 900.0, 0.4, -0.2);
        assert!(message.contains("~v3,0,0,220.000;"));
        assert!(message.contains("~v3,1,0,900.000;"));
        assert!(message.contains("~v3,2,0,0.4000;"));
        assert!(message.contains("~v3,3,0,-0.200;"));
    }

    #[test]
    fn both_profiles_compile_in_glicol() {
        for profile in ModeProfile::ALL {
            let code = full_code(&theme(), profile);
            let rendered = render_offline(&code, 2);
            assert!(
                rendered.is_ok(),
                "{profile:?} graph failed to compile: {:?}\n{code}",
                rendered.err()
            );
        }
    }

    /// The base voices carry the piece on their own, so a compiled graph that
    /// renders silence means the mix is broken even though nothing errored.
    #[test]
    fn both_profiles_are_audible_before_any_parameter_is_sent() {
        for profile in ModeProfile::ALL {
            let code = full_code(&theme(), profile);
            let channels = render_offline(&code, 64).expect("graph should compile");
            let peak = crate::music::engine::peak(&channels);
            assert!(
                peak > 0.01,
                "{profile:?} rendered near silence (peak {peak})\n{code}"
            );
            assert!(peak <= 1.0, "{profile:?} clipped (peak {peak})");
        }
    }
}
