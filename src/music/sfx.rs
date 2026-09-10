//! One-shot gameplay sound effects.
//!
//! Each effect is a dedicated (initially silent) Glicol chain whose parameters
//! are animated on the audio thread for the effect's duration, so a crash
//! decays, a turn blips, and a beam-up rises. The shapes live here as pure
//! functions so they can be unit-tested without a device.

use bevy::prelude::Message;

/// A gameplay sound effect. Emitted by the Lightcycle systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Message)]
pub enum MusicSfx {
    /// Dirty, descending noise impact.
    Crash,
    /// Short pitched blip when a turn is queued.
    Turn,
    /// Rising sweep for the Recognizer-style directory transport.
    Beam,
    /// Bright shimmer for the parent portal / leaving a document.
    Portal,
}

/// One frame of effect output. `freq` is ignored by effects whose oscillator is
/// noise (see [`MusicSfx::uses_pitch`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SfxVoice {
    pub freq: f32,
    pub cutoff: f32,
    pub gain: f32,
    pub pan: f32,
}

impl MusicSfx {
    /// Every effect, in graph order. The score walks this to declare and mix the
    /// chains, so a new variant cannot be forgotten in the output.
    pub const ALL: [Self; 4] = [Self::Crash, Self::Turn, Self::Beam, Self::Portal];

    /// Glicol reference chain this effect drives.
    pub fn chain(self) -> &'static str {
        match self {
            Self::Crash => "~sfx_crash",
            Self::Turn => "~sfx_turn",
            Self::Beam => "~sfx_beam",
            Self::Portal => "~sfx_portal",
        }
    }

    /// The effect's Glicol chain definition, silent until it is triggered. Node
    /// layout is fixed: 0 osc/noise, 1 low-pass, 2 gain, 3 pan. Glicol's `noise`
    /// node takes an integer seed, not a float.
    pub fn declaration(self) -> String {
        let chain = self.chain();
        match self {
            Self::Crash => format!("{chain}: noise 1 >> lpf 4000.0 0.7 >> mul 0.0 >> pan 0.0;"),
            Self::Turn => format!("{chain}: squ 1200.0 >> lpf 3200.0 0.7 >> mul 0.0 >> pan -0.15;"),
            Self::Beam => format!("{chain}: saw 180.0 >> lpf 600.0 0.7 >> mul 0.0 >> pan 0.0;"),
            Self::Portal => format!("{chain}: tri 660.0 >> lpf 2500.0 0.7 >> mul 0.0 >> pan 0.1;"),
        }
    }

    pub fn duration(self) -> f32 {
        match self {
            Self::Crash => 0.6,
            Self::Turn => 0.16,
            Self::Beam => 1.15,
            Self::Portal => 0.5,
        }
    }

    /// False for the noise-based crash, whose first node has no frequency.
    pub fn uses_pitch(self) -> bool {
        !matches!(self, Self::Crash)
    }

    /// Envelope and motion at `progress` (0..1 through the effect).
    pub fn voice(self, progress: f32) -> SfxVoice {
        let p = progress.clamp(0.0, 1.0);
        match self {
            Self::Crash => SfxVoice {
                freq: 0.0,
                // Opens bright then thuds down.
                cutoff: 200.0 + 3800.0 * (1.0 - p).powf(1.6),
                gain: 0.55 * (1.0 - p).powi(2),
                pan: 0.0,
            },
            Self::Turn => SfxVoice {
                // A short falling blip.
                freq: 1400.0 - 520.0 * p,
                cutoff: 3200.0,
                gain: 0.22 * (1.0 - p).powf(1.5),
                pan: -0.15,
            },
            Self::Beam => SfxVoice {
                // Exponential rise, swelling then fading: a beam-up.
                freq: 180.0 * (1400.0_f32 / 180.0).powf(p),
                cutoff: 600.0 * (7200.0_f32 / 600.0).powf(p),
                gain: 0.34 * (4.0 * p * (1.0 - p)).powf(0.6),
                pan: 0.0,
            },
            Self::Portal => SfxVoice {
                // A gentle rising third sparkle.
                freq: 660.0 + 660.0 * p,
                cutoff: 2500.0,
                gain: 0.28 * (1.0 - p).powf(1.2),
                pan: 0.1,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MusicSfx;

    #[test]
    fn every_effect_has_a_chain_duration_and_bounded_gain() {
        for sfx in MusicSfx::ALL {
            assert!(sfx.chain().starts_with("~sfx_"), "{sfx:?}");
            assert!(sfx.duration() > 0.0, "{sfx:?}");
            for step in 0..=10 {
                let voice = sfx.voice(step as f32 / 10.0);
                assert!(
                    (0.0..=1.0).contains(&voice.gain),
                    "{sfx:?} gain {}",
                    voice.gain
                );
                assert!(voice.cutoff >= 100.0, "{sfx:?} cutoff {}", voice.cutoff);
                assert!(
                    (-1.0..=1.0).contains(&voice.pan),
                    "{sfx:?} pan {}",
                    voice.pan
                );
            }
        }
    }

    #[test]
    fn the_crash_decays_and_is_not_pitched() {
        assert!(!MusicSfx::Crash.uses_pitch());
        let start = MusicSfx::Crash.voice(0.0);
        let end = MusicSfx::Crash.voice(1.0);
        assert!(start.gain > end.gain);
        assert!(start.cutoff > end.cutoff, "crash should darken as it falls");
    }

    #[test]
    fn the_beam_swells_and_rises() {
        let start = MusicSfx::Beam.voice(0.0);
        let middle = MusicSfx::Beam.voice(0.5);
        let end = MusicSfx::Beam.voice(1.0);
        assert!(middle.gain > start.gain && middle.gain > end.gain);
        assert!(end.freq > middle.freq && middle.freq > start.freq);
        assert!(end.cutoff > start.cutoff);
    }

    #[test]
    fn the_turn_is_a_short_falling_blip() {
        let start = MusicSfx::Turn.voice(0.0);
        let end = MusicSfx::Turn.voice(1.0);
        assert!(start.freq > end.freq);
        assert!(start.gain > end.gain);
    }
}
