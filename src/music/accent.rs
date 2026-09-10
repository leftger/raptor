//! One-shot musical accents triggered by gameplay.
//!
//! The audio thread keeps a dedicated silent noise chain and briefly opens it
//! when one of these messages arrives, so crashes, transports, turns, and
//! portals get a short hit without rebuilding the graph.

use bevy::prelude::Message;

/// A short accent layered over the mix. Emitted by the Lightcycle systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Message)]
pub enum MusicAccent {
    Crash,
    Entry,
    Turn,
    Portal,
}

impl MusicAccent {
    /// Initial gain and low-pass cutoff of the hit.
    pub fn voice(self) -> (f32, f32) {
        match self {
            Self::Crash => (0.5, 700.0),
            Self::Entry => (0.32, 2600.0),
            Self::Turn => (0.16, 1800.0),
            Self::Portal => (0.38, 1400.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MusicAccent;

    #[test]
    fn accent_voices_are_bounded_and_positive() {
        let accents = [
            MusicAccent::Crash,
            MusicAccent::Entry,
            MusicAccent::Turn,
            MusicAccent::Portal,
        ];
        for accent in accents {
            let (gain, cutoff) = accent.voice();
            assert!(gain > 0.0 && gain <= 1.0, "{accent:?} gain {gain}");
            assert!(cutoff > 0.0, "{accent:?} cutoff {cutoff}");
        }
    }
}
