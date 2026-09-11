//! The brick breaker, with its Bevy wiring in `plugins::lightcycle`.
//!
//! [`sim`] is the Bevy-free ball, bricks and paddle; the plugin owns the meshes
//! and drives the bike along the bottom of the court as the rebounding surface.

pub mod sim;

pub use sim::{BreakerPhase, BreakerSim};
