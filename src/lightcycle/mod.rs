pub mod logic;

use crate::document::DocumentLayout;
use crate::filesystem::FileNode;
use bevy::prelude::Resource;
use std::collections::HashMap;
use std::path::PathBuf;

/// Snapshot of one active lightcycle run over a loaded directory or document.
#[derive(Debug, Clone)]
pub struct ActiveRun {
    pub sim: logic::LightcycleSim,
    pub arena: logic::Arena,
    pub environment: RunEnvironment,
    /// Human-readable label shown after a crash.
    pub crash_label: Option<String>,
    /// Human-readable label shown while waiting for a folder/parent/document load.
    pub entering_label: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RunEnvironment {
    Directory {
        nodes: Vec<FileNode>,
        cells: HashMap<(i32, i32), usize>,
    },
    Document {
        path: PathBuf,
        name: String,
        layout: DocumentLayout,
        focused_block: Option<usize>,
    },
}

impl ActiveRun {
    pub fn directory_cells(&self) -> Option<&HashMap<(i32, i32), usize>> {
        match &self.environment {
            RunEnvironment::Directory { cells, .. } => Some(cells),
            RunEnvironment::Document { .. } => None,
        }
    }

    pub fn directory_nodes(&self) -> Option<&[FileNode]> {
        match &self.environment {
            RunEnvironment::Directory { nodes, .. } => Some(nodes),
            RunEnvironment::Document { .. } => None,
        }
    }

    pub fn is_document(&self) -> bool {
        matches!(self.environment, RunEnvironment::Document { .. })
    }
}

/// Bevy resource for lightcycle-only state.
#[derive(Resource, Default)]
pub struct LightcycleState {
    pub run: Option<ActiveRun>,
    /// Run built when a mode transition started, held back until the transition
    /// reaches the point where the old world is swapped out for it.
    pub pending_run: Option<ActiveRun>,
    /// Accumulated time used for fixed-step simulation.
    pub clock: f32,
    /// Active crash animation state (debris burst + camera shake).
    pub crash_fx: Option<CrashFx>,
    /// Active directory-tower transport animation.
    pub entry_fx: Option<EntryFx>,
    /// Rebuild the containing directory arena after leaving a document.
    pub restore_directory: bool,
}

/// Timeline for the crash animation.
#[derive(Debug, Clone, Copy)]
pub struct CrashFx {
    pub timer: f32,
    pub duration: f32,
    pub spawned: bool,
}

impl CrashFx {
    pub fn new(duration: f32) -> Self {
        Self {
            timer: duration,
            duration,
            spawned: false,
        }
    }
}

/// Timeline and deferred navigation target for a directory-tower transport.
///
/// The directory request waits until the visual reaches its bright apex so a
/// fast filesystem scan cannot replace the old arena before the effect appears.
#[derive(Debug, Clone)]
pub struct EntryFx {
    pub elapsed: f32,
    pub duration: f32,
    pub target: PathBuf,
    pub spawned: bool,
    pub requested: bool,
}

impl EntryFx {
    pub fn new(target: PathBuf, duration: f32) -> Self {
        Self {
            elapsed: 0.0,
            duration,
            target,
            spawned: false,
            requested: false,
        }
    }

    pub fn progress(&self) -> f32 {
        (self.elapsed / self.duration).clamp(0.0, 1.0)
    }
}
