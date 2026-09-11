//! The asteroid-field mini-game, with its Bevy wiring in `plugins::lightcycle`.
//!
//! [`sim`] is the Bevy-free state machine; the plugin owns the entities, the
//! camera and the sound.

pub mod sim;

pub use sim::{AsteroidsPhase, AsteroidsSim};
