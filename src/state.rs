use crate::config;
use crate::filesystem::{DirectoryContents, Navigator};
use bevy::prelude::{Component, Message, Resource};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, TryRecvError};

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

/// Background load bookkeeping. A fresh mpsc channel is created for every requested scan.
#[derive(Resource, Default)]
pub struct DirectoryLoadState {
    pub generation: u64,
    pub loading: bool,
    pub receiver: Option<Mutex<Receiver<DirectoryLoadResult>>>,
    pub last_error: Option<String>,
}

impl DirectoryLoadState {
    pub fn next_generation(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }
}

pub struct DirectoryLoadResult {
    pub generation: u64,
    pub path: PathBuf,
    pub result: Result<DirectoryContents, String>,
}

impl DirectoryLoadState {
    /// Poll the active background scan without blocking.
    pub fn poll(&mut self) -> Option<DirectoryLoadResult> {
        let receiver = self.receiver.as_ref()?;
        match receiver.lock().unwrap().try_recv() {
            Ok(result) => {
                if result.generation == self.generation {
                    self.loading = false;
                }
                Some(result)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.loading = false;
                None
            }
        }
    }

    pub fn begin_scan(&mut self, generation: u64, path: PathBuf, show_hidden: bool) {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.receiver = Some(Mutex::new(receiver));
        self.loading = true;
        self.last_error = None;

        std::thread::spawn(move || {
            let result = crate::filesystem::loader::load_directory(&path, show_hidden)
                .map_err(|error| error.to_string());
            let _ = sender.send(DirectoryLoadResult {
                generation,
                path,
                result,
            });
        });
    }
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Message, Debug)]
pub struct DirectoryRequested {
    pub path: PathBuf,
}

#[derive(Message, Debug)]
pub struct DirectoryLoaded {
    pub path: PathBuf,
    pub contents: DirectoryContents,
}

#[derive(Message, Debug)]
pub struct DirectoryLoadFailed {
    pub path: PathBuf,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Attached to each spawned block entity; `index` points into
/// `NavigatorResource.entries`.
#[derive(Component, Debug, Clone, Copy)]
pub struct FileBlock {
    pub index: usize,
}

/// Root node for projected labels (below the main UI chrome).
#[derive(Component, Debug)]
pub struct LabelsRoot;

/// Marker for entities that should be despawned whenever the directory changes.
#[derive(Component, Debug)]
pub struct DirectorySceneRoot;
