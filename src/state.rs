use crate::config;
use crate::filesystem::Navigator;
use bevy::prelude::{Component, Resource};
use std::path::{Path, PathBuf};

#[derive(Resource)]
pub struct NavigatorResource(pub Navigator);

impl NavigatorResource {
    pub fn new(initial_path: PathBuf, show_hidden: bool) -> Self {
        Self(Navigator::empty(initial_path, show_hidden))
    }
}

#[derive(Resource, Default)]
pub struct SelectionState {
    pub selected: Option<usize>,
    pub hovered: Option<usize>,
}

#[derive(Resource, Debug, Clone)]
pub struct UiSettings {
    pub show_labels: bool,
    pub show_fps: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            show_labels: true,
            show_fps: true,
        }
    }
}

#[derive(Resource, Default)]
pub struct UiNotice {
    pub message: Option<String>,
}

/// Pause-menu state for the lightcycle mode: whether the run is frozen and
/// which warp target the menu cursor points at.
#[derive(Resource, Default)]
pub struct PauseState {
    pub paused: bool,
    pub warp_index: usize,
}

/// Which directories the rider has already opened, so a revisit can be a
/// cache hit.
#[derive(Resource, Default)]
pub struct CacheState {
    pub visited: std::collections::HashSet<PathBuf>,
}

/// Raster settings that trade looks for frame time.
///
/// These are the levers that actually matter on a weak GPU: multisampling,
/// HDR bloom, the full-screen scanline overlay, and the window size.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct RenderSettings {
    /// Multisample count: 1, 2, 4, or 8.
    pub msaa: u32,
    /// HDR bloom while riding (lightcycle mode). Measured at about 15ms a
    /// frame on integrated graphics, because it forces the HDR pipeline.
    pub bloom: bool,
    /// Full-screen scanline overlay.
    pub scanlines: bool,
    /// Vignette post-process on the camera.
    pub vignette: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            msaa: config::MSAA_SAMPLES,
            bloom: true,
            scanlines: true,
            vignette: true,
        }
    }
}

impl RenderSettings {
    /// Multisampling as a Bevy sample count.
    pub fn msaa_samples(&self) -> u32 {
        self.msaa.max(1)
    }
}

/// Drives the occasional plunge of the directory call stack through the arena.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct StackMotion {
    /// Seconds until the next plunge begins.
    pub timer: f32,
    /// Progress through a plunge in `0.0..1.0`, or `None` while idle.
    pub plunge: Option<f32>,
}

impl Default for StackMotion {
    fn default() -> Self {
        Self {
            timer: config::STACK_PLUNGE_INTERVAL,
            plunge: None,
        }
    }
}

impl StackMotion {
    /// Ticks the clock by `dt` and reports progress through a running plunge.
    pub fn advance(&mut self, dt: f32) -> Option<f32> {
        match self.plunge {
            Some(progress) => {
                let next = progress + dt / config::STACK_PLUNGE_SECONDS;
                if next >= 1.0 {
                    self.plunge = None;
                    self.timer = config::STACK_PLUNGE_INTERVAL;
                    None
                } else {
                    self.plunge = Some(next);
                    Some(next)
                }
            }
            None => {
                self.timer -= dt;
                if self.timer <= 0.0 {
                    self.plunge = Some(0.0);
                }
                self.plunge
            }
        }
    }
}

/// The fake machine telemetry shown in the HUD: a syscall trace cursor and
/// register/clock readouts.
#[derive(Resource, Default)]
pub struct MachineState {
    pub index: usize,
    pub pc: u32,
    pub sp: u32,
    pub clock_mhz: f32,
    pub temperature: f32,
}

/// The rising memory flood that chases a directory run.
///
/// `plane` is the flood's grid-Z line, in the same cell coordinates the arena
/// and the bike use, so contact is a plain comparison.
#[derive(Resource, Default)]
pub struct FloodState {
    pub active: bool,
    pub timer: f32,
    pub delay: f32,
    pub min_z: f32,
    pub max_z: f32,
    pub center_x: f32,
    pub width: f32,
    pub plane: f32,
}

impl FloodState {
    /// Sends the flood back to the arena's edge and restarts its countdown.
    ///
    /// A restarted run gets its full delay back: otherwise the wall it died to
    /// is still standing past the spawn cell, and the respawn dies instantly to
    /// a hazard it never had a chance to outrun.
    pub fn recede(&mut self) {
        self.timer = 0.0;
        self.plane = self.min_z;
    }
}

/// Directory visit history, played as a version-control time machine.
///
/// `past` is the commit log of directories already ridden; `future` holds what
/// a rewind undid, so a fast-forward can put it back.
#[derive(Resource, Default)]
pub struct HistoryState {
    pub past: Vec<PathBuf>,
    pub future: Vec<PathBuf>,
    /// Last rewind/fast-forward result shown on the status line.
    pub notice: String,
    pub notice_timer: f32,
}

impl HistoryState {
    /// Records riding into `path`. A load that lands where the log already
    /// points (a rewind or fast-forward arriving) is not a new commit, and any
    /// redo branch is dropped when the rider genuinely goes somewhere new.
    pub fn commit(&mut self, path: &Path) {
        if self.past.last().is_some_and(|last| last == path) {
            return;
        }
        self.past.push(path.to_path_buf());
        if self.past.len() > config::HISTORY_LIMIT {
            self.past.remove(0);
        }
        self.future.clear();
    }

    /// Steps back one directory: the present lands on the redo stack and the
    /// previous commit becomes the target.
    pub fn rewind(&mut self) -> Option<PathBuf> {
        if !self.can_rewind() {
            return None;
        }
        let current = self.past.pop()?;
        self.future.push(current);
        self.past.last().cloned()
    }

    /// Steps forward again, undoing a rewind.
    pub fn fast_forward(&mut self) -> Option<PathBuf> {
        let target = self.future.pop()?;
        self.past.push(target.clone());
        Some(target)
    }

    /// How deep the commit log is, for the HUD.
    pub fn depth(&self) -> usize {
        self.past.len()
    }

    /// True while a redo branch is waiting.
    pub fn can_rewind(&self) -> bool {
        self.past.len() > 1
    }

    /// True while a fast-forward is available.
    pub fn can_fast_forward(&self) -> bool {
        !self.future.is_empty()
    }
}

#[derive(Resource)]
pub struct OrbitCameraResource {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: bevy::math::Vec3,
    pub target_destination: bevy::math::Vec3,
}

impl Default for OrbitCameraResource {
    fn default() -> Self {
        Self {
            yaw: config::DEFAULT_CAMERA_YAW,
            pitch: config::DEFAULT_CAMERA_PITCH,
            distance: config::DEFAULT_CAMERA_DISTANCE,
            target: bevy::math::Vec3::ZERO,
            target_destination: bevy::math::Vec3::ZERO,
        }
    }
}

impl OrbitCameraResource {
    pub fn rotate(&mut self, delta_x: f32, delta_y: f32) {
        self.yaw += delta_x * config::CAMERA_ROTATION_SPEED;
        self.pitch = (self.pitch - delta_y * config::CAMERA_ROTATION_SPEED)
            .clamp(config::MIN_CAMERA_PITCH, config::MAX_CAMERA_PITCH);
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance - delta * config::CAMERA_ZOOM_SPEED)
            .clamp(config::MIN_CAMERA_DISTANCE, config::MAX_CAMERA_DISTANCE);
    }

    pub fn set_target(&mut self, target: bevy::math::Vec3) {
        self.target_destination = target;
    }

    pub fn reset_target(&mut self) {
        self.target_destination = bevy::math::Vec3::ZERO;
    }

    pub fn update(&mut self) {
        let factor = config::CAMERA_LERP_FACTOR;
        self.target = self.target.lerp(self.target_destination, factor);
    }

    pub fn position(&self) -> bevy::math::Vec3 {
        bevy::math::Vec3::new(
            self.target.x + self.distance * self.yaw.cos() * self.pitch.cos(),
            self.target.y + self.distance * self.pitch.sin(),
            self.target.z + self.distance * self.yaw.sin() * self.pitch.cos(),
        )
    }
}

#[derive(Resource, Default)]
pub struct ScanEffectResource {
    pub y_position: f32,
    pub active: bool,
}

impl ScanEffectResource {
    pub fn reset(&mut self) {
        self.y_position = 0.0;
        self.active = true;
    }

    pub fn update(&mut self, delta_seconds: f32) {
        if self.active {
            self.y_position += delta_seconds * config::SCAN_SPEED;
            if self.y_position > config::SCAN_MAX_HEIGHT {
                self.active = false;
            }
        }
    }
}

/// Root node for projected labels (below the main UI chrome).
#[derive(Component, Debug)]
pub struct LabelsRoot;

/// Marker for entities that should be despawned whenever the directory changes.
#[derive(Component, Debug)]
pub struct DirectorySceneRoot;

/// The active interaction mode over the shared directory scene.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InteractionMode {
    #[default]
    Explorer,
    Lightcycle,
}

/// Non-directory entities owned by the lightcycle mode (cycle, walls, portal).
///
/// Kept separate from [`DirectorySceneRoot`] so a directory change never
/// despawns the player.
#[derive(Component, Debug)]
pub struct LightcycleSceneRoot;

/// Trail mesh chunks owned by the lightcycle mode.
#[derive(Component, Debug)]
pub struct TrailSceneRoot;

#[cfg(test)]
mod tests {
    use super::HistoryState;
    use std::path::{Path, PathBuf};

    fn path(name: &str) -> PathBuf {
        PathBuf::from(name)
    }

    #[test]
    fn a_revisit_of_the_same_directory_is_not_a_new_commit() {
        let mut history = HistoryState::default();
        history.commit(Path::new("/a"));
        history.commit(Path::new("/a"));
        assert_eq!(history.depth(), 1, "the log should not grow on a revisit");
    }

    #[test]
    fn rewinding_walks_back_and_fast_forwarding_restores() {
        let mut history = HistoryState::default();
        history.commit(Path::new("/a"));
        history.commit(Path::new("/b"));
        history.commit(Path::new("/c"));

        assert_eq!(history.rewind(), Some(path("/b")));
        assert_eq!(history.rewind(), Some(path("/a")));
        assert!(history.can_fast_forward());

        assert_eq!(history.fast_forward(), Some(path("/b")));
        assert_eq!(history.fast_forward(), Some(path("/c")));
        assert!(!history.can_fast_forward(), "the redo branch is spent");
    }

    #[test]
    fn the_first_commit_has_nowhere_to_rewind_to() {
        let mut history = HistoryState::default();
        history.commit(Path::new("/a"));
        assert_eq!(history.rewind(), None);
        assert_eq!(
            history.depth(),
            1,
            "a refused rewind must not consume the log"
        );
    }

    #[test]
    fn going_somewhere_new_drops_the_redo_branch() {
        let mut history = HistoryState::default();
        history.commit(Path::new("/a"));
        history.commit(Path::new("/b"));
        assert_eq!(history.rewind(), Some(path("/a")));
        assert!(history.can_fast_forward());

        history.commit(Path::new("/elsewhere"));
        assert!(
            !history.can_fast_forward(),
            "a new commit forks the history"
        );
        assert_eq!(history.depth(), 2);
    }

    #[test]
    fn the_log_is_capped() {
        let mut history = HistoryState::default();
        for index in 0..(crate::config::HISTORY_LIMIT + 8) {
            history.commit(Path::new(&format!("/dir{index}")));
        }
        assert_eq!(history.depth(), crate::config::HISTORY_LIMIT);
    }
}
