//! The Plinko board, played over an R file's ring.
//!
//! [`sim`] is the Bevy-free board, pins and balls; the plugin owns the static
//! pins and buckets, the pooled balls and the side-on camera.

pub mod sim;

pub use sim::{PlinkoPhase, PlinkoSim};
