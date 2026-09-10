//! Procedurally generated music driven by the filesystem.
//!
//! The module is split so most of it stays audio-free and unit-testable:
//!
//! - `theme` derives a deterministic [`theme::MusicTheme`] and a
//!   [`theme::ModeProfile`] from a directory path and the interaction mode.
//! - `score` turns a theme plus nearby nodes into Glicol source strings.
//! - `proximity` maps a listener position to per-entry voice targets.
//! - `arp` steps the seeded evolving melody layer.
//! - `accent` defines the gameplay one-shots layered over the mix.
//! - `engine` owns the realtime Glicol render thread and the cpal output stream.
//!
//! See `docs/music-plan.md` for the design.

pub mod accent;
pub mod arp;
pub mod engine;
pub mod proximity;
pub mod score;
pub mod theme;

pub use accent::MusicAccent;
pub use arp::ArpState;
pub use proximity::VoiceMixer;
pub use score::full_code;
pub use theme::{ModeProfile, MusicTheme};
