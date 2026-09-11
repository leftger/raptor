//! The Pac-Man maze, played over a Go file's ring.
//!
//! [`sim`] is the Bevy-free maze, dots and ghosts; the plugin owns the pooled
//! dot and ghost meshes, the walls and the overhead camera.

pub mod sim;

pub use sim::{PacPhase, PacSim};
