//! Seeded arpeggiator: the evolving melody layer.
//!
//! Rather than holding a static drone, the base track walks a note pattern in
//! time. The pattern is derived from the directory theme's seed, so every folder
//! has its own tune, and it is retriggered with a per-note envelope so the
//! arpeggio reads as plucks instead of a continuous tone. Explorer steps once
//! per beat and softly; Lightcycle steps twice per beat with more gain.

use super::theme::{ModeProfile, MusicTheme};
use crate::config;

/// Scale-degree walk, rotated by the theme seed.
const PATTERN: [u32; 8] = [0, 2, 4, 7, 4, 2, 5, 9];

/// One frame of arpeggiator output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArpVoice {
    pub freq: f32,
    pub cutoff: f32,
    pub gain: f32,
    pub pan: f32,
    /// True on the frame a new note starts.
    pub triggered: bool,
}

/// Steps a note pattern in time and retriggers a per-note envelope.
#[derive(Debug, Default)]
pub struct ArpState {
    clock: f32,
    level: f32,
    step: u64,
}

impl ArpState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Restarts the pattern, e.g. after a room change.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Number of notes played since the last reset. Test-only.
    #[cfg(test)]
    pub fn step(&self) -> u64 {
        self.step
    }

    /// Advances by `dt` and returns the current voice.
    pub fn update(&mut self, dt: f32, theme: &MusicTheme, profile: ModeProfile) -> ArpVoice {
        let dt = dt.clamp(0.0, 0.25);
        let interval = profile.arp_interval(theme.bpm(profile)).max(0.01);

        self.clock += dt;
        let mut triggered = false;
        // Catch up at most a couple of steps, so a long frame cannot machine-gun.
        let mut caught_up = 0;
        while self.clock >= interval && caught_up < 2 {
            self.clock -= interval;
            self.step += 1;
            self.level = velocity(self.step - 1);
            triggered = true;
            caught_up += 1;
        }

        // Exponential decay of the current note.
        let decay = (-dt / profile.arp_decay_tau().max(0.01)).exp();
        self.level *= decay;

        let note = self.step.saturating_sub(1);
        let (degree, octave) = degree(theme.seed, note);
        let freq = theme.degree_hz(degree, octave);
        let cutoff = (theme.node_cutoff(theme.seed ^ note) + degree as f32 * 90.0)
            .clamp(config::MUSIC_VOICE_CUTOFF_MIN, 12_000.0);
        let pan = if note.is_multiple_of(2) { -0.3 } else { 0.3 };
        let gain = self.level * profile.arp_gain();

        ArpVoice {
            freq,
            cutoff,
            gain,
            pan,
            triggered,
        }
    }
}

/// Next (scale degree, octave offset) for a step.
fn degree(seed: u64, note: u64) -> (u32, i32) {
    let rotation = (seed % PATTERN.len() as u64) as usize;
    let index = (note as usize + rotation) % PATTERN.len();
    let octave = if (note / PATTERN.len() as u64) % 4 == 3 {
        1
    } else {
        0
    };
    (PATTERN[index], octave)
}

/// Downbeat-accented velocity.
fn velocity(note: u64) -> f32 {
    if note.is_multiple_of(4) {
        1.0
    } else if note.is_multiple_of(2) {
        0.72
    } else {
        0.55
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> MusicTheme {
        MusicTheme::from_seed(0x51ee_d0d0)
    }

    fn run(state: &mut ArpState, profile: ModeProfile, seconds: f32) -> Vec<ArpVoice> {
        let dt = 1.0 / 120.0;
        let steps = (seconds / dt) as usize;
        let mut voices = Vec::with_capacity(steps);
        for _ in 0..steps {
            voices.push(state.update(dt, &theme(), profile));
        }
        voices
    }

    #[test]
    fn action_steps_much_faster_than_calm() {
        let mut calm = ArpState::new();
        let mut action = ArpState::new();
        let calm_notes = run(&mut calm, ModeProfile::Calm, 8.0)
            .iter()
            .filter(|voice| voice.triggered)
            .count();
        let action_notes = run(&mut action, ModeProfile::Action, 8.0)
            .iter()
            .filter(|voice| voice.triggered)
            .count();
        assert!(
            action_notes > calm_notes * 2,
            "action {action_notes} vs calm {calm_notes}"
        );
        assert!(calm_notes >= 6, "calm should still play: {calm_notes}");
    }

    #[test]
    fn the_same_seed_plays_the_same_tune() {
        let mut a = ArpState::new();
        let mut b = ArpState::new();
        let first: Vec<f32> = run(&mut a, ModeProfile::Calm, 12.0)
            .iter()
            .filter(|voice| voice.triggered)
            .map(|voice| voice.freq)
            .collect();
        let second: Vec<f32> = run(&mut b, ModeProfile::Calm, 12.0)
            .iter()
            .filter(|voice| voice.triggered)
            .map(|voice| voice.freq)
            .collect();
        assert_eq!(first, second);
        assert!(first.len() > 4);
    }

    #[test]
    fn every_note_stays_in_the_theme_scale() {
        let theme = theme();
        let intervals = theme.scale.intervals();
        let mut state = ArpState::new();
        for voice in run(&mut state, ModeProfile::Action, 20.0) {
            if !voice.triggered {
                continue;
            }
            let midi = 69.0 + 12.0 * (voice.freq / 440.0).log2();
            let offset = ((midi.round() as i32 - theme.root_midi as i32).rem_euclid(12)) as u8;
            assert!(
                intervals.contains(&offset),
                "freq {} gave offset {offset}",
                voice.freq
            );
        }
    }

    #[test]
    fn the_envelope_decays_between_notes() {
        let mut state = ArpState::new();
        let voices = run(&mut state, ModeProfile::Calm, 4.0);
        let peak = voices
            .iter()
            .position(|voice| voice.triggered)
            .expect("a note");
        let after = voices[peak + 3].gain;
        assert!(after < voices[peak].gain, "envelope did not decay");
    }

    #[test]
    fn reset_starts_the_pattern_over() {
        let mut state = ArpState::new();
        run(&mut state, ModeProfile::Calm, 3.0);
        assert!(state.step() > 0);
        state.reset();
        assert_eq!(state.step(), 0);
    }
}
