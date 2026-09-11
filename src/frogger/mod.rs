//! The Frogger highway, played over a JavaScript file's ring.
//!
//! [`sim`] is the Bevy-free lanes and hops; the plugin owns the pooled
//! obstacle cubes and the overhead camera.

pub mod sim;

pub use sim::{FroggerPhase, FroggerSim};
