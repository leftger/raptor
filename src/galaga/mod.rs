//! The Galaga field, a formation-wave shooter played over a JSON file's ring.
//!
//! [`sim`] is the Bevy-free formation, dives, beams and lives; the plugin owns
//! the pooled bug and beam meshes and the overhead camera.

pub mod sim;

pub use sim::{GalagaPhase, GalagaSim};
