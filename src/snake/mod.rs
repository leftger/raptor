//! The snake mini-game, with its Bevy wiring in `plugins::lightcycle`.
//!
//! [`sim`] is the Bevy-free state machine; the plugin owns the entities, the
//! camera and the sound. The rider's movement is the ordinary lightcycle grid,
//! so this module only tracks the power-ups, the finite tail and the exit.

pub mod sim;

pub use sim::SnakeSim;
