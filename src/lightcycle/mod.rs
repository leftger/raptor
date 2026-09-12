pub mod logic;

use crate::asteroids::AsteroidsSim;
use crate::bomberman::BomberSim;
use crate::breaker::BreakerSim;
use crate::columns::ColumnsSim;
use crate::disc::{DiscLayout, DiscSim, SourceGame, SourceLanguage};
use crate::document::DocumentLayout;
use crate::filesystem::FileNode;
use crate::frogger::FroggerSim;
use crate::galaga::GalagaSim;
use crate::pacman::PacSim;
use crate::platformer::PlatformerSim;
use crate::plinko::PlinkoSim;
use crate::qbert::QbertSim;
use crate::snake::SnakeSim;
use crate::stealth::StealthSim;
use crate::surfer::SurferSim;
use crate::tetris::TetrisSim;
use bevy::prelude::Resource;
use std::collections::HashMap;
use std::path::PathBuf;

/// Snapshot of one active lightcycle run over a loaded directory, document, or
/// disc-wars ring.
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

/// The sim a source ring is actually playing. Carrying exactly one keeps the
/// game kind and its state from drifting apart: a disc-wars ring has no
/// asteroid field to accidentally read (and vice versa).
#[derive(Debug, Clone)]
pub enum SourceSim {
    DiscWars(DiscSim),
    /// Boxed: the field holds two entity vectors.
    Asteroids(Box<AsteroidsSim>),
    Snake(SnakeSim),
    /// Boxed: the level holds every platform in the run.
    Platformer(Box<PlatformerSim>),
    Breaker(Box<BreakerSim>),
    Stealth(Box<StealthSim>),
    Surfer(Box<SurferSim>),
    Galaga(Box<GalagaSim>),
    PacMan(Box<PacSim>),
    Columns(Box<ColumnsSim>),
    Tetris(Box<TetrisSim>),
    Frogger(Box<FroggerSim>),
    Qbert(Box<QbertSim>),
    Bomberman(Box<BomberSim>),
    Plinko(Box<PlinkoSim>),
}

impl SourceSim {
    /// Which game this state belongs to.
    pub fn game(&self) -> SourceGame {
        match self {
            Self::DiscWars(_) => SourceGame::DiscWars,
            Self::Asteroids(_) => SourceGame::Asteroids,
            Self::Snake(_) => SourceGame::Snake,
            Self::Platformer(_) => SourceGame::Platformer,
            Self::Breaker(_) => SourceGame::Breaker,
            Self::Stealth(_) => SourceGame::Stealth,
            Self::Surfer(_) => SourceGame::RiverSurfer,
            Self::Galaga(_) => SourceGame::Galaga,
            Self::PacMan(_) => SourceGame::PacMan,
            Self::Columns(_) => SourceGame::Columns,
            Self::Tetris(_) => SourceGame::Tetris,
            Self::Frogger(_) => SourceGame::Frogger,
            Self::Qbert(_) => SourceGame::Qbert,
            Self::Bomberman(_) => SourceGame::Bomberman,
            Self::Plinko(_) => SourceGame::Plinko,
        }
    }

    pub fn as_disc(&self) -> Option<&DiscSim> {
        match self {
            Self::DiscWars(disc) => Some(disc),
            _ => None,
        }
    }

    pub fn as_disc_mut(&mut self) -> Option<&mut DiscSim> {
        match self {
            Self::DiscWars(disc) => Some(disc),
            _ => None,
        }
    }

    pub fn as_asteroids(&self) -> Option<&AsteroidsSim> {
        match self {
            Self::Asteroids(field) => Some(field),
            _ => None,
        }
    }

    pub fn as_asteroids_mut(&mut self) -> Option<&mut AsteroidsSim> {
        match self {
            Self::Asteroids(field) => Some(field),
            _ => None,
        }
    }

    pub fn as_snake(&self) -> Option<&SnakeSim> {
        match self {
            Self::Snake(snake) => Some(snake),
            _ => None,
        }
    }

    pub fn as_snake_mut(&mut self) -> Option<&mut SnakeSim> {
        match self {
            Self::Snake(snake) => Some(snake),
            _ => None,
        }
    }

    pub fn as_platformer(&self) -> Option<&PlatformerSim> {
        match self {
            Self::Platformer(level) => Some(level),
            _ => None,
        }
    }

    pub fn as_platformer_mut(&mut self) -> Option<&mut PlatformerSim> {
        match self {
            Self::Platformer(level) => Some(level),
            _ => None,
        }
    }

    pub fn as_breaker(&self) -> Option<&BreakerSim> {
        match self {
            Self::Breaker(level) => Some(level),
            _ => None,
        }
    }

    pub fn as_breaker_mut(&mut self) -> Option<&mut BreakerSim> {
        match self {
            Self::Breaker(level) => Some(level),
            _ => None,
        }
    }

    pub fn as_stealth(&self) -> Option<&StealthSim> {
        match self {
            Self::Stealth(room) => Some(room),
            _ => None,
        }
    }

    pub fn as_stealth_mut(&mut self) -> Option<&mut StealthSim> {
        match self {
            Self::Stealth(room) => Some(room),
            _ => None,
        }
    }

    pub fn as_surfer(&self) -> Option<&SurferSim> {
        match self {
            Self::Surfer(surfer) => Some(surfer),
            _ => None,
        }
    }

    pub fn as_surfer_mut(&mut self) -> Option<&mut SurferSim> {
        match self {
            Self::Surfer(surfer) => Some(surfer),
            _ => None,
        }
    }

    pub fn as_galaga(&self) -> Option<&GalagaSim> {
        match self {
            Self::Galaga(sim) => Some(sim),
            _ => None,
        }
    }

    pub fn as_galaga_mut(&mut self) -> Option<&mut GalagaSim> {
        match self {
            Self::Galaga(sim) => Some(sim),
            _ => None,
        }
    }

    pub fn as_pacman(&self) -> Option<&PacSim> {
        match self {
            Self::PacMan(sim) => Some(sim),
            _ => None,
        }
    }

    pub fn as_pacman_mut(&mut self) -> Option<&mut PacSim> {
        match self {
            Self::PacMan(sim) => Some(sim),
            _ => None,
        }
    }

    pub fn as_columns(&self) -> Option<&ColumnsSim> {
        match self {
            Self::Columns(sim) => Some(sim),
            _ => None,
        }
    }

    pub fn as_columns_mut(&mut self) -> Option<&mut ColumnsSim> {
        match self {
            Self::Columns(sim) => Some(sim),
            _ => None,
        }
    }

    pub fn as_tetris(&self) -> Option<&TetrisSim> {
        match self {
            Self::Tetris(sim) => Some(sim),
            _ => None,
        }
    }

    pub fn as_tetris_mut(&mut self) -> Option<&mut TetrisSim> {
        match self {
            Self::Tetris(sim) => Some(sim),
            _ => None,
        }
    }

    pub fn as_frogger(&self) -> Option<&FroggerSim> {
        match self {
            Self::Frogger(sim) => Some(sim),
            _ => None,
        }
    }

    pub fn as_frogger_mut(&mut self) -> Option<&mut FroggerSim> {
        match self {
            Self::Frogger(sim) => Some(sim),
            _ => None,
        }
    }

    pub fn as_qbert(&self) -> Option<&QbertSim> {
        match self {
            Self::Qbert(sim) => Some(sim),
            _ => None,
        }
    }

    pub fn as_qbert_mut(&mut self) -> Option<&mut QbertSim> {
        match self {
            Self::Qbert(sim) => Some(sim),
            _ => None,
        }
    }

    pub fn as_bomberman(&self) -> Option<&BomberSim> {
        match self {
            Self::Bomberman(sim) => Some(sim),
            _ => None,
        }
    }

    pub fn as_bomberman_mut(&mut self) -> Option<&mut BomberSim> {
        match self {
            Self::Bomberman(sim) => Some(sim),
            _ => None,
        }
    }

    pub fn as_plinko(&self) -> Option<&PlinkoSim> {
        match self {
            Self::Plinko(sim) => Some(sim),
            _ => None,
        }
    }

    pub fn as_plinko_mut(&mut self) -> Option<&mut PlinkoSim> {
        match self {
            Self::Plinko(sim) => Some(sim),
            _ => None,
        }
    }
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
    /// A ring generated from a source file. Which game is played inside it is
    /// chosen by the language; the ring geometry, close gate and restore path
    /// are shared by all of them.
    ///
    /// The player's own movement always stays in [`ActiveRun::sim`]. Disc wars
    /// and snake drive it normally; the asteroid field parks it and only pivots,
    /// stepping [`SourceSim::Asteroids`] instead.
    Source {
        path: PathBuf,
        name: String,
        language: SourceLanguage,
        /// Boxed: a ring's layout is much larger than the other variants.
        layout: Box<DiscLayout>,
        sim: SourceSim,
        focused_block: Option<usize>,
    },
}

impl ActiveRun {
    pub fn directory_cells(&self) -> Option<&HashMap<(i32, i32), usize>> {
        match &self.environment {
            RunEnvironment::Directory { cells, .. } => Some(cells),
            _ => None,
        }
    }

    pub fn directory_nodes(&self) -> Option<&[FileNode]> {
        match &self.environment {
            RunEnvironment::Directory { nodes, .. } => Some(nodes),
            _ => None,
        }
    }

    pub fn is_document(&self) -> bool {
        matches!(self.environment, RunEnvironment::Document { .. })
    }

    pub fn is_source(&self) -> bool {
        matches!(self.environment, RunEnvironment::Source { .. })
    }

    /// Which mini-game this run is, for source runs only.
    pub fn source_game(&self) -> Option<SourceGame> {
        match &self.environment {
            RunEnvironment::Source { sim, .. } => Some(sim.game()),
            _ => None,
        }
    }

    pub fn source_disc_mut(&mut self) -> Option<&mut DiscSim> {
        match &mut self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_disc_mut(),
            _ => None,
        }
    }

    pub fn source_disc(&self) -> Option<&DiscSim> {
        match &self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_disc(),
            _ => None,
        }
    }

    pub fn source_asteroids(&self) -> Option<&AsteroidsSim> {
        match &self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_asteroids(),
            _ => None,
        }
    }

    pub fn source_asteroids_mut(&mut self) -> Option<&mut AsteroidsSim> {
        match &mut self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_asteroids_mut(),
            _ => None,
        }
    }

    pub fn source_snake(&self) -> Option<&SnakeSim> {
        match &self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_snake(),
            _ => None,
        }
    }

    pub fn source_snake_mut(&mut self) -> Option<&mut SnakeSim> {
        match &mut self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_snake_mut(),
            _ => None,
        }
    }

    pub fn source_platformer(&self) -> Option<&PlatformerSim> {
        match &self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_platformer(),
            _ => None,
        }
    }

    pub fn source_platformer_mut(&mut self) -> Option<&mut PlatformerSim> {
        match &mut self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_platformer_mut(),
            _ => None,
        }
    }

    pub fn source_breaker(&self) -> Option<&BreakerSim> {
        match &self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_breaker(),
            _ => None,
        }
    }

    pub fn source_breaker_mut(&mut self) -> Option<&mut BreakerSim> {
        match &mut self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_breaker_mut(),
            _ => None,
        }
    }

    pub fn source_stealth(&self) -> Option<&StealthSim> {
        match &self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_stealth(),
            _ => None,
        }
    }

    pub fn source_stealth_mut(&mut self) -> Option<&mut StealthSim> {
        match &mut self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_stealth_mut(),
            _ => None,
        }
    }

    pub fn source_surfer(&self) -> Option<&SurferSim> {
        match &self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_surfer(),
            _ => None,
        }
    }

    pub fn source_surfer_mut(&mut self) -> Option<&mut SurferSim> {
        match &mut self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_surfer_mut(),
            _ => None,
        }
    }

    pub fn source_galaga(&self) -> Option<&GalagaSim> {
        match &self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_galaga(),
            _ => None,
        }
    }

    pub fn source_galaga_mut(&mut self) -> Option<&mut GalagaSim> {
        match &mut self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_galaga_mut(),
            _ => None,
        }
    }

    pub fn source_pacman(&self) -> Option<&PacSim> {
        match &self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_pacman(),
            _ => None,
        }
    }

    pub fn source_pacman_mut(&mut self) -> Option<&mut PacSim> {
        match &mut self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_pacman_mut(),
            _ => None,
        }
    }

    pub fn source_columns(&self) -> Option<&ColumnsSim> {
        match &self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_columns(),
            _ => None,
        }
    }

    pub fn source_columns_mut(&mut self) -> Option<&mut ColumnsSim> {
        match &mut self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_columns_mut(),
            _ => None,
        }
    }

    pub fn source_tetris(&self) -> Option<&TetrisSim> {
        match &self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_tetris(),
            _ => None,
        }
    }

    pub fn source_tetris_mut(&mut self) -> Option<&mut TetrisSim> {
        match &mut self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_tetris_mut(),
            _ => None,
        }
    }

    pub fn source_frogger(&self) -> Option<&FroggerSim> {
        match &self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_frogger(),
            _ => None,
        }
    }

    pub fn source_frogger_mut(&mut self) -> Option<&mut FroggerSim> {
        match &mut self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_frogger_mut(),
            _ => None,
        }
    }

    pub fn source_qbert(&self) -> Option<&QbertSim> {
        match &self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_qbert(),
            _ => None,
        }
    }

    pub fn source_qbert_mut(&mut self) -> Option<&mut QbertSim> {
        match &mut self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_qbert_mut(),
            _ => None,
        }
    }

    pub fn source_bomberman(&self) -> Option<&BomberSim> {
        match &self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_bomberman(),
            _ => None,
        }
    }

    pub fn source_bomberman_mut(&mut self) -> Option<&mut BomberSim> {
        match &mut self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_bomberman_mut(),
            _ => None,
        }
    }

    pub fn source_plinko(&self) -> Option<&PlinkoSim> {
        match &self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_plinko(),
            _ => None,
        }
    }

    pub fn source_plinko_mut(&mut self) -> Option<&mut PlinkoSim> {
        match &mut self.environment {
            RunEnvironment::Source { sim, .. } => sim.as_plinko_mut(),
            _ => None,
        }
    }

    /// True only while an asteroid field is actually being played.
    pub fn asteroid_field_active(&self) -> bool {
        self.source_asteroids()
            .is_some_and(|field| field.is_active())
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
    /// Bullet time is held down while riding a disc-wars ring. Set from input
    /// each frame; the ring's fixed step is scaled while it is true.
    pub slow_motion: bool,
    /// Seconds of cache-hit ground-speed surge remaining.
    pub cache_boost: f32,
    /// Countdown to the next garbage-collector sweep of a directory arena.
    pub gc_timer: f32,
    /// Seconds left of the collector's world stall.
    pub gc_pause: f32,
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
