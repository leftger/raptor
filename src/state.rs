use crate::config;
use crate::filesystem::Navigator;
use bevy::prelude::{Component, Resource};
use std::path::PathBuf;

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
