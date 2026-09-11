//! Source-language identity for a disc-wars ring.
//!
//! Each language is a ring type, not a different game (see
//! `docs/disc-wars-plan.md`, section 1). This module owns the extension
//! allowlist and the small per-language touches: the compiler name the opponent
//! dais carries, the ring accent colour, and the arpeggiator tint applied while
//! the ring is open.

use crate::config;
use bevy::prelude::Color;
use std::path::Path;

/// A source language that may be ridden as a disc-wars ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceLanguage {
    Rust,
    C,
    Cpp,
    Python,
}

/// Which mini-game a source file opens.
///
/// The ring geometry is shared; only the game inside it changes. Python files
/// are an asteroid field, the rest are disc-wars rings. Moving the field to
/// another extension is a one-line change in [`SourceLanguage::game`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceGame {
    DiscWars,
    Asteroids,
}

impl SourceLanguage {
    /// Every language, for tests that need to cover them all.
    pub const ALL: [SourceLanguage; 4] = [
        SourceLanguage::Rust,
        SourceLanguage::C,
        SourceLanguage::Cpp,
        SourceLanguage::Python,
    ];

    /// Maps a lowercase file extension to a language, or `None` when the
    /// extension is not part of the allowlist.
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "rs" => Some(Self::Rust),
            // Headers are their own ring for now; the plan leaves linking a
            // header to its `.c` to phase 2.
            "c" | "h" => Some(Self::C),
            "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => Some(Self::Cpp),
            "py" | "pyi" => Some(Self::Python),
            _ => None,
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|extension| extension.to_str())
            .and_then(Self::from_extension)
    }

    /// Short name used in the HUD and the dais label.
    pub fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Python => "python",
        }
    }

    /// Which mini-game a file of this language hosts. See [`SourceGame`].
    pub fn game(self) -> SourceGame {
        match self {
            Self::Python => SourceGame::Asteroids,
            Self::Rust | Self::C | Self::Cpp => SourceGame::DiscWars,
        }
    }

    /// Tool the "compiler" opponent is themed after. Flavor text only; nothing
    /// here is a real diagnostic.
    pub fn compiler(self) -> &'static str {
        match self {
            Self::Rust => "rustc",
            Self::C => "cc",
            Self::Cpp => "c++",
            Self::Python => "CPython",
        }
    }

    /// Accent used for the ring wall trim, the gate, and the floor markings.
    pub fn accent(self) -> Color {
        match self {
            Self::Rust => config::DISC_RUST_ACCENT,
            Self::C => config::DISC_C_ACCENT,
            Self::Cpp => config::DISC_CPP_ACCENT,
            Self::Python => config::DISC_PYTHON_ACCENT,
        }
    }

    /// Arpeggiator gain multiplier while the ring is open.
    pub fn arp_gain_scale(self) -> f32 {
        match self {
            Self::Rust => config::MUSIC_DISC_RUST_ARP_GAIN,
            Self::C => config::MUSIC_DISC_C_ARP_GAIN,
            Self::Cpp => config::MUSIC_DISC_CPP_ARP_GAIN,
            Self::Python => config::MUSIC_DISC_PYTHON_ARP_GAIN,
        }
    }

    /// Arpeggiator step-rate multiplier while the ring is open. Rust steps
    /// harder, Python pumps slower, so the fight reads differently per language
    /// without changing the folder's key or scale.
    pub fn arp_rate_scale(self) -> f32 {
        match self {
            Self::Rust => config::MUSIC_DISC_RUST_ARP_RATE,
            Self::C => config::MUSIC_DISC_C_ARP_RATE,
            Self::Cpp => config::MUSIC_DISC_CPP_ARP_RATE,
            Self::Python => config::MUSIC_DISC_PYTHON_ARP_RATE,
        }
    }

    /// Deterministic crash flavor, e.g. `error[E0308]`. Never a real diagnostic.
    pub fn crash_flavor(self, seed: u64) -> &'static str {
        match self {
            Self::Rust => "error[E0308]: mismatched types",
            Self::C => {
                if seed & 1 == 0 {
                    "Segmentation fault (core dumped)"
                } else {
                    "warning: implicit declaration of function"
                }
            }
            Self::Cpp => {
                if seed & 1 == 0 {
                    "error: no matching function for call"
                } else {
                    "undefined reference to `vtable'"
                }
            }
            Self::Python => {
                if seed & 1 == 0 {
                    "Traceback (most recent call last)"
                } else {
                    "IndentationError: unexpected indent"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SourceLanguage;
    use std::path::PathBuf;

    #[test]
    fn the_allowlist_covers_the_planned_extensions() {
        for (path, language) in [
            ("main.rs", SourceLanguage::Rust),
            ("lib.c", SourceLanguage::C),
            ("api.h", SourceLanguage::C),
            ("engine.cpp", SourceLanguage::Cpp),
            ("widget.hpp", SourceLanguage::Cpp),
            ("tool.py", SourceLanguage::Python),
            ("stubs.pyi", SourceLanguage::Python),
        ] {
            assert_eq!(
                SourceLanguage::from_path(&PathBuf::from(path)),
                Some(language),
                "{path}"
            );
        }
    }

    #[test]
    fn non_source_extensions_are_excluded() {
        for path in [
            "README.md",
            "photo.png",
            "Cargo.lock",
            "archive.tar.gz",
            "noext",
        ] {
            assert_eq!(
                SourceLanguage::from_path(&PathBuf::from(path)),
                None,
                "{path}"
            );
        }
    }

    #[test]
    fn detection_is_case_insensitive() {
        assert_eq!(
            SourceLanguage::from_path(&PathBuf::from("MAIN.RS")),
            Some(SourceLanguage::Rust)
        );
    }

    #[test]
    fn every_language_has_a_distinct_identity() {
        let mut names = std::collections::BTreeSet::new();
        let mut accents = std::collections::BTreeSet::new();
        for language in SourceLanguage::ALL {
            assert!(names.insert(language.name()));
            assert!(!language.compiler().is_empty());
            accents.insert(format!("{:?}", language.accent()));
        }
        assert_eq!(accents.len(), SourceLanguage::ALL.len());
    }

    #[test]
    fn python_files_host_the_asteroid_field() {
        use super::SourceGame;
        assert_eq!(SourceLanguage::Python.game(), SourceGame::Asteroids);
        for language in [SourceLanguage::Rust, SourceLanguage::C, SourceLanguage::Cpp] {
            assert_eq!(language.game(), SourceGame::DiscWars);
        }
    }
}
