//! The stealth run, with its Bevy wiring in `plugins::lightcycle`.
//!
//! [`sim`] is the Bevy-free room, patrols and detection; the plugin owns the
//! meshes, the vision cones on the floor and the overhead camera.

pub mod sim;

pub use sim::{StealthPhase, StealthSim};
