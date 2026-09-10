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

/// Definitions for the profile's base voices and the shared accent chain.
pub fn base_voices(theme: &MusicTheme, profile: ModeProfile) -> String {
    let wave = theme.family.waveform();
    let mut code = String::new();

    // Shared, silent until an accent opens it. Node layout: 0 noise, 1 low-pass
    // (param 0 cutoff), 2 gain, 3 pan. See [`accent_message`]. Glicol's `noise`
    // node takes an integer seed, not a float.
    let _ = writeln!(
        code,
        "~accent: noise 1 >> lpf 1800.0 0.7 >> mul 0.0 >> pan 0.0;"
    );

    match profile {
        ModeProfile::Calm => {
            let root = theme.root_hz();
            let fifth = theme.degree_hz(4, 0);
            let cutoff = (root * 6.0).clamp(400.0, 2400.0);
            let _ = writeln!(
                code,
                "~pad0: {wave} {root:.2} >> lpf {cutoff:.1} 0.7 >> mul {:.3} >> pan -0.25;",
                config::MUSIC_CALM_PAD_GAIN
            );
            let _ = writeln!(
                code,
                "~pad1: {wave} {fifth:.2} >> lpf {:.1} 0.7 >> mul {:.3} >> pan 0.25;",
                (cutoff * 1.2).clamp(400.0, 3000.0),
                config::MUSIC_CALM_PAD_GAIN * 0.8
            );
        }
        ModeProfile::Action => {
            let bass = theme.degree_hz(0, -1);
            let lead = theme.degree_hz(4, 1);
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
                "~bass: saw {bass:.2} >> lpf {:.1} 0.8 >> mul {:.3} >> mul ~pump >> pan -0.15;",
                (bass * 8.0).clamp(500.0, 2600.0),
                config::MUSIC_ACTION_BASS_GAIN
            );
            let _ = writeln!(
                code,
                "~lead: squ {lead:.2} >> lpf {:.1} 0.7 >> mul {:.3} >> mul ~pump >> pan 0.2;",
                (lead * 6.0).clamp(700.0, 3200.0),
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

/// The single `o:` chain mixing the profile's base voices, the accent, and the
/// whole bank.
pub fn output_chain(profile: ModeProfile) -> String {
    let mut code = String::from("o: mix ~accent");
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

/// A Glicol `send_msg` payload that opens the shared accent chain. Matches the
/// layout produced by [`base_voices`]: 1 = low-pass cutoff, 2 = gain.
pub fn accent_message(cutoff: f32, gain: f32) -> String {
    format!("~accent,1,0,{cutoff:.1};~accent,2,0,{gain:.4};")
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
    fn only_action_pumps_and_both_profiles_carry_the_accent() {
        let calm = full_code(&theme(), ModeProfile::Calm);
        let action = full_code(&theme(), ModeProfile::Action);
        assert!(!calm.contains("~pump"));
        assert!(action.contains("~pump"));
        assert!(calm.contains("~accent"));
        assert!(action.contains("~accent"));
    }

    #[test]
    fn accent_message_addresses_the_fixed_node_layout() {
        let message = accent_message(1400.0, 0.375);
        assert!(message.contains("~accent,1,0,1400.0;"));
        assert!(message.contains("~accent,2,0,0.3750;"));
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
}
