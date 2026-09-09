pub mod logic;

use crate::filesystem::FileNode;
use bevy::prelude::Resource;
use std::collections::HashMap;

/// Snapshot of one active lightcycle run over a loaded directory layout.
///
/// The run owns its nodes/cell map so gameplay systems do not depend on the
/// `NavigatorResource` being applied before a `DirectoryLoaded` reset.
#[derive(Debug, Clone)]
pub struct ActiveRun {
    pub sim: logic::LightcycleSim,
    pub arena: logic::Arena,
    pub nodes: Vec<FileNode>,
    /// Maps grid positions to indices into `nodes` for file/dir collisions.
    pub cells: HashMap<(i32, i32), usize>,
    /// Human-readable label shown after a crash.
    pub crash_label: Option<String>,
    /// Human-readable label shown while waiting for a folder/parent load.
    pub entering_label: Option<String>,
    /// Set when trail geometry should be rebuilt.
    pub trail_dirty: bool,
}

/// Bevy resource for lightcycle-only state.
#[derive(Resource, Default)]
pub struct LightcycleState {
    pub run: Option<ActiveRun>,
    /// Accumulated time used for fixed-step simulation.
    pub clock: f32,
    /// Active crash animation state (debris burst + camera shake).
    pub crash_fx: Option<CrashFx>,
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
