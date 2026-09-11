//! The Tetris well, played over a YAML file's ring.
//!
//! [`sim`] is the Bevy-free board and tetrominoes; the plugin owns the pooled
//! block cubes and the side-on camera.

pub mod sim;

pub use sim::{TetrisPhase, TetrisSim};
