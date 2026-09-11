//! The river surfer, a Jet-Moto-style hoverbike run down a procedural river.
//!
//! [`sim`] is the Bevy-free course, bike physics and obstacles; the plugin owns
//! the water ribbon, rocks, gates and the chase camera.

pub mod sim;

pub use sim::{SurferPhase, SurferSim};
