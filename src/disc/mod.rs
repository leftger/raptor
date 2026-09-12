//! Disc wars: a fighting disc-wars arena entered from source files.
//!
//! See `docs/disc-wars-plan.md`. The module mirrors `document/` and `music/`:
//! most of it is Bevy-free and unit-testable, and the Bevy wiring lives in
//! `plugins/lightcycle.rs` and `plugins/ui.rs`.
//!
//! - [`language`] owns the extension allowlist and per-language ring identity.
//! - [`layout`] fingerprints a file's bytes into a circular [`layout::DiscLayout`].
//! - [`combat`] runs the fight itself on the shared lightcycle fixed clock.
//! - [`load`] is the background byte loader, capped like a document.
//!
//! The public types are re-exported here so the plugin layer has one import.

pub mod combat;
pub mod language;
pub mod layout;
pub mod load;

pub use combat::{DiscEvents, DiscPhase, DiscSim, PlayerSnapshot};
pub use language::{SourceGame, SourceLanguage};
pub use layout::{DiscLayout, build_capped_disc_arena, build_disc_arena, build_flat_arena};
pub use load::{SourceLoadFailed, SourceLoadState, SourceLoaded, SourceRequested, WarpRequested};
