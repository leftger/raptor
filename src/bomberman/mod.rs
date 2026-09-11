//! The Bomberman room, played over a PHP file's ring.
//!
//! [`sim`] is the Bevy-free room, crates and bombs; the plugin owns the pooled
//! crate and bomb cubes, the exit marker and the overhead camera.

pub mod sim;

pub use sim::{BomberPhase, BomberSim};
