//! The side-scrolling platformer, with its Bevy wiring in `plugins::lightcycle`.
//!
//! [`sim`] is the Bevy-free level and physics; the plugin owns the meshes, the
//! side camera and the sound. Nothing here touches the cell grid the bike games
//! share — the runner lives in metres on the `X`/`Y` plane.

pub mod sim;

pub use sim::{PlatformerPhase, PlatformerSim};
