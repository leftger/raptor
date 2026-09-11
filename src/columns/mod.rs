//! The Columns well, played over a Ruby file's ring.
//!
//! [`sim`] is the Bevy-free well and gem matching; the plugin owns the pooled
//! gem cubes and the side-on camera.

pub mod sim;

pub use sim::{ColumnsPhase, ColumnsSim};
