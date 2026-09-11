//! The Q*bert pyramid, played over a Zig file's ring.
//!
//! [`sim`] is the Bevy-free pyramid and enemies; the plugin owns the static
//! cube pyramid, the pooled enemy cubes and the tilted camera.

pub mod sim;

pub use sim::{QbertPhase, QbertSim};
