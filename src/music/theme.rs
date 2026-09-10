//! Deterministic, filesystem-seeded musical themes and mode intensity profiles.
//!
//! A directory's path is hashed once (through the same `stable_path_seed` the
//! Lightcycle cities use) into a [`MusicTheme`]. The theme is stable across
//! visits and Rust releases, so a folder keeps its key, scale, tempo, and
//! timbre family. The [`ModeProfile`] then decides how intense that identity is
//! rendered: calm ambient in Explorer, driving in Lightcycle.

use crate::config;
use crate::lightcycle::logic::{CityTheme, stable_path_seed};
use crate::state::InteractionMode;
use std::path::Path;

/// Semitone offsets for the scales the generator can choose from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    Major,
    Minor,
    Dorian,
    PentatonicMinor,
}

impl Scale {
    pub const ALL: [Scale; 4] = [
        Scale::Major,
        Scale::Minor,
        Scale::Dorian,
        Scale::PentatonicMinor,
    ];

    pub fn intervals(self) -> &'static [u8] {
        match self {
            Scale::Major => &[0, 2, 4, 5, 7, 9, 11],
            Scale::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Scale::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Scale::PentatonicMinor => &[0, 3, 5, 7, 10],
        }
    }

    pub fn from_seed(seed: u64) -> Self {
        Self::ALL[(seed % Self::ALL.len() as u64) as usize]
    }

    pub fn name(self) -> &'static str {
        match self {
            Scale::Major => "major",
            Scale::Minor => "minor",
            Scale::Dorian => "dorian",
            Scale::PentatonicMinor => "pent-minor",
        }
    }
}

/// Oscillator character. The index order mirrors [`CityTheme`] so a folder's
/// sound family matches the color of its path-seeded district.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimbreFamily {
    Glass,
    Pulse,
    Reed,
    Brass,
}

impl TimbreFamily {
    pub fn from_seed(seed: u64) -> Self {
        Self::from_city_theme(CityTheme::from_seed(seed))
    }

    pub fn from_city_theme(theme: CityTheme) -> Self {
        match theme {
            CityTheme::Cyan => Self::Glass,
            CityTheme::Magenta => Self::Pulse,
            CityTheme::Violet => Self::Reed,
            CityTheme::Amber => Self::Brass,
        }
    }

    /// Glicol oscillator node name used for this family's voices.
    pub fn waveform(self) -> &'static str {
        match self {
            Self::Glass => "sin",
            Self::Pulse => "saw",
            Self::Reed => "tri",
            Self::Brass => "squ",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Glass => "glass",
            Self::Pulse => "pulse",
            Self::Reed => "reed",
            Self::Brass => "brass",
        }
    }
}

/// Intensity of the generated music, selected by the interaction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeProfile {
    Calm,
    Action,
}

impl ModeProfile {
    /// Iterated by tests that need every profile.
    #[cfg(test)]
    pub const ALL: [ModeProfile; 2] = [ModeProfile::Calm, ModeProfile::Action];

    pub fn from_mode(mode: InteractionMode) -> Self {
        match mode {
            InteractionMode::Explorer => Self::Calm,
            InteractionMode::Lightcycle => Self::Action,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Calm => "CALM",
            Self::Action => "ACTION",
        }
    }

    pub fn tempo_multiplier(self) -> f32 {
        match self {
            Self::Calm => 1.0,
            Self::Action => config::MUSIC_ACTION_TEMPO_MULTIPLIER,
        }
    }

    pub fn voice_budget(self) -> usize {
        match self {
            Self::Calm => config::MUSIC_CALM_VOICE_BUDGET,
            Self::Action => config::MUSIC_ACTION_VOICE_BUDGET,
        }
    }

    pub fn proximity_radius(self) -> f32 {
        match self {
            Self::Calm => config::MUSIC_CALM_PROXIMITY_RADIUS,
            Self::Action => config::MUSIC_ACTION_PROXIMITY_RADIUS,
        }
    }

    pub fn gain_ceiling(self) -> f32 {
        match self {
            Self::Calm => config::MUSIC_CALM_GAIN_CEILING,
            Self::Action => config::MUSIC_ACTION_GAIN_CEILING,
        }
    }

    pub fn smoothing_tau(self) -> f32 {
        match self {
            Self::Calm => config::MUSIC_CALM_SMOOTHING_TAU,
            Self::Action => config::MUSIC_ACTION_SMOOTHING_TAU,
        }
    }
}

/// The stable musical identity of one directory.
#[derive(Debug, Clone, PartialEq)]
pub struct MusicTheme {
    pub seed: u64,
    pub root_midi: u8,
    pub scale: Scale,
    /// Calm-mode tempo; [`MusicTheme::bpm`] scales it by the active profile.
    pub base_bpm: f32,
    pub family: TimbreFamily,
    pub reverb: f32,
}

impl MusicTheme {
    pub fn from_path(path: &Path) -> Self {
        Self::from_seed(stable_path_seed(path))
    }

    /// Maps a raw seed into musical parameters. The bit slices are independent
    /// so root, scale, tempo, and family do not move together.
    pub fn from_seed(seed: u64) -> Self {
        let bpm_steps = (config::MUSIC_CALM_BPM_MAX - config::MUSIC_CALM_BPM_MIN).round() as u64;
        Self {
            seed,
            root_midi: 45 + (seed % 12) as u8, // A2..G#3
            scale: Scale::from_seed(seed >> 4),
            base_bpm: config::MUSIC_CALM_BPM_MIN + ((seed >> 16) % (bpm_steps + 1)) as f32,
            family: TimbreFamily::from_seed(seed),
            reverb: 0.35 + ((seed >> 24) % 30) as f32 / 100.0,
        }
    }

    pub fn root_hz(&self) -> f32 {
        note_hz(self.root_midi)
    }

    /// Tempo for the active profile, clamped to a sane range.
    pub fn bpm(&self, profile: ModeProfile) -> f32 {
        (self.base_bpm * profile.tempo_multiplier()).clamp(40.0, 200.0)
    }

    /// Frequency of a scale degree. Degrees wrap upward through octaves, so
    /// `degree = scale length` is the root one octave up.
    pub fn degree_hz(&self, degree: u32, octave_shift: i32) -> f32 {
        let intervals = self.scale.intervals();
        let index = degree as usize % intervals.len();
        let octave = (degree as i32 / intervals.len() as i32) + octave_shift;
        let midi = self.root_midi as i32 + intervals[index] as i32 + 12 * octave;
        note_hz(midi.clamp(0, 127) as u8)
    }

    /// Pitch for one filesystem entry, in the directory's scale. Directories
    /// sit an octave lower than files so they read as drones.
    pub fn node_hz(&self, node_seed: u64, is_dir: bool) -> f32 {
        let degree = (node_seed % 8) as u32;
        let octave = ((node_seed >> 8) % 3) as i32 - 1;
        let octave = if is_dir { octave - 1 } else { octave };
        self.degree_hz(degree, octave)
    }

    /// Base filter cutoff for one entry before proximity opens it up.
    pub fn node_cutoff(&self, node_seed: u64) -> f32 {
        let base = config::MUSIC_VOICE_CUTOFF_MIN + ((node_seed >> 12) % 900) as f32;
        base.clamp(
            config::MUSIC_VOICE_CUTOFF_MIN,
            config::MUSIC_VOICE_CUTOFF_MIN + 900.0,
        )
    }
}

/// Equal-tempered MIDI note to frequency.
pub fn note_hz(midi: u8) -> f32 {
    440.0 * 2.0_f32.powf((midi as f32 - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_path_always_yields_the_same_theme() {
        let path = PathBuf::from("/home/usuario/Projects/raptor");
        assert_eq!(MusicTheme::from_path(&path), MusicTheme::from_path(&path));
    }

    #[test]
    fn different_paths_can_differ() {
        let a = MusicTheme::from_path(&PathBuf::from("/a"));
        let b = MusicTheme::from_path(&PathBuf::from("/b"));
        assert_ne!(a, b);
    }

    #[test]
    fn mode_profiles_keep_identity_but_change_intensity() {
        let theme = MusicTheme::from_seed(0x1234_5678_9abc_def0);
        let calm = theme.bpm(ModeProfile::Calm);
        let action = theme.bpm(ModeProfile::Action);
        assert!(action > calm, "action should be faster: {action} vs {calm}");
        assert!(ModeProfile::Action.voice_budget() > ModeProfile::Calm.voice_budget());
        assert!(ModeProfile::Action.proximity_radius() > ModeProfile::Calm.proximity_radius());
        assert!(ModeProfile::Action.gain_ceiling() > ModeProfile::Calm.gain_ceiling());
    }

    #[test]
    fn root_and_scale_are_profile_independent() {
        let theme = MusicTheme::from_seed(42);
        // The theme itself owns root/scale/family; profiles only scale intensity.
        assert_eq!(theme.root_hz(), theme.root_hz());
        assert_eq!(theme.family, TimbreFamily::from_seed(42));
    }

    #[test]
    fn degree_zero_is_the_root() {
        let theme = MusicTheme::from_seed(7);
        assert!((theme.degree_hz(0, 0) - theme.root_hz()).abs() < 0.01);
    }

    #[test]
    fn nodes_stay_within_the_scale() {
        let theme = MusicTheme::from_seed(99);
        let intervals = theme.scale.intervals();
        for seed in 0..256_u64 {
            let hz = theme.node_hz(seed, false);
            let midi = 69.0 + 12.0 * (hz / 440.0).log2();
            let rounded = midi.round() as i32;
            let offset = (rounded - theme.root_midi as i32).rem_euclid(12);
            assert!(
                intervals.contains(&(offset as u8)),
                "seed {seed} produced offset {offset}, hz {hz}"
            );
        }
    }
}
