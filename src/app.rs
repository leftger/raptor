use macroquad::prelude::*;
use std::path::PathBuf;

use crate::camera::CameraController;
use crate::config;
use crate::filesystem::Navigator;
use crate::input::{Command, MouseState};
use crate::render::ScanEffect;

pub struct AppState {
    pub navigator: Navigator,
    pub camera: CameraController,
    pub mouse: MouseState,
    pub scan_effect: ScanEffect,
    pub selected: Option<usize>,
    pub show_labels: bool,
    pub show_hidden: bool,
}

impl AppState {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            navigator: Navigator::new(home),
            camera: CameraController::new(),
            mouse: MouseState::new(),
            scan_effect: ScanEffect::new(),
            selected: None,
            show_labels: true,
            show_hidden: false,
        }
    }

    pub fn update(&mut self) {
        self.camera.update();

        let camera3d = self.camera.to_camera3d();
        self.mouse.update(&self.navigator.entries, &camera3d);
        self.scan_effect.update(get_frame_time());

        if self.mouse.is_dragging {
            self.camera
                .rotate(self.mouse.drag_delta.x, self.mouse.drag_delta.y);
        }
        if self.mouse.scroll_delta != 0.0 {
            self.camera.zoom(self.mouse.scroll_delta);
        }

        if let Some(clicked_idx) = self.mouse.clicked_index {
            if self.selected == Some(clicked_idx) {
                self.execute_command(Command::EnterDirectory);
            } else {
                self.selected = Some(clicked_idx);
            }
        }
    }

    pub fn execute_command(&mut self, command: Command) {
        match command {
            Command::MoveLeft => self.move_selection(-1, 0),
            Command::MoveRight => self.move_selection(1, 0),
            Command::MoveUp => self.move_selection(0, -1),
            Command::MoveDown => self.move_selection(0, 1),

            Command::GoToFirst => {
                if !self.navigator.entries.is_empty() {
                    self.selected = Some(0);
                    self.focus_camera_on_selection();
                }
            }

            Command::GoToLast => {
                if !self.navigator.entries.is_empty() {
                    self.selected = Some(self.navigator.entries.len() - 1);
                    self.focus_camera_on_selection();
                }
            }

            Command::EnterDirectory => {
                if let Some(idx) = self.selected
                    && self.navigator.enter_directory(idx)
                {
                    self.on_directory_changed();
                }
            }

            Command::GoBack => {
                if self.navigator.go_back() {
                    self.on_directory_changed();
                }
            }

            Command::GoToParent => {
                if self.navigator.go_to_parent() {
                    self.on_directory_changed();
                }
            }

            Command::GoToRoot => {
                self.navigator.go_to_root();
                self.on_directory_changed();
            }

            Command::GoHome => {
                self.navigator.go_home();
                self.on_directory_changed();
            }

            Command::ToggleHidden => {
                self.navigator.show_hidden = !self.navigator.show_hidden;
                self.navigator.load(&self.navigator.current_path.clone());
                self.on_directory_changed();
            }

            Command::ToggleLabels => {
                self.show_labels = !self.show_labels;
            }

            #[cfg(target_os = "macos")]
            Command::OpenInFinder => {
                if let Some(idx) = self.selected {
                    if let Some(entry) = self.navigator.entries.get(idx) {
                        let path = self.navigator.current_path.join(&entry.name);
                        // Use 'open -R' to reveal the file/folder in Finder
                        std::process::Command::new("open")
                            .arg("-R")
                            .arg(&path)
                            .spawn()
                            .ok();
                    }
                }
            }

            Command::Select(idx) => {
                if idx < self.navigator.entries.len() {
                    self.selected = Some(idx);
                    self.focus_camera_on_selection();
                }
            }

            Command::ClearSelection => {
                self.selected = None;
            }
        }
    }

    fn move_selection(&mut self, dx: i32, dz: i32) {
        if self.navigator.entries.is_empty() {
            return;
        }

        if self.selected.is_none() {
            self.selected = Some(0);
            self.focus_camera_on_selection();
            return;
        }

        let current_idx = self.selected.unwrap();
        let current_node = &self.navigator.entries[current_idx];
        let (cx, cz) = current_node.grid_pos;

        // Calculate camera-relative direction vector
        let camera_yaw = self.camera.yaw;
        let dx_f = dx as f32;
        let dz_f = dz as f32;
        let sin_yaw = camera_yaw.sin();
        let cos_yaw = camera_yaw.cos();
        
        // Transform to world space
        let world_dx = dx_f * sin_yaw + dz_f * cos_yaw;
        let world_dz = -dx_f * cos_yaw + dz_f * sin_yaw;
        
        // Normalize direction
        let dir_len = (world_dx * world_dx + world_dz * world_dz).sqrt();
        if dir_len < 0.001 {
            return;
        }
        let norm_dx = world_dx / dir_len;
        let norm_dz = world_dz / dir_len;
        
        // Find the best candidate block in this direction
        let mut best_idx: Option<usize> = None;
        let mut best_score = f32::MAX;
        
        for (i, node) in self.navigator.entries.iter().enumerate() {
            if i == current_idx {
                continue;
            }
            
            let (nx, nz) = node.grid_pos;
            let delta_x = (nx - cx) as f32;
            let delta_z = (nz - cz) as f32;
            
            // Calculate dot product to see if block is in the right direction
            let dot = delta_x * norm_dx + delta_z * norm_dz;
            
            // Only consider blocks that are in front of us (even slightly)
            if dot <= 0.0 {
                continue;
            }
            
            // Calculate distance to the block
            let dist = (delta_x * delta_x + delta_z * delta_z).sqrt();
            
            // Calculate perpendicular distance from the intended direction
            let perp_dist = ((delta_x * norm_dz - delta_z * norm_dx).abs()).max(0.01);
            
            // Score: prioritize blocks that are closer and more aligned
            // Use both forward progress (dot) and perpendicular offset (perp_dist)
            let score = dist + perp_dist * 2.0 - dot * 0.5;
            
            if score < best_score {
                best_score = score;
                best_idx = Some(i);
            }
        }
        
        if let Some(new_idx) = best_idx {
            self.selected = Some(new_idx);
            self.focus_camera_on_selection();
        }
    }

    fn focus_camera_on_selection(&mut self) {
        if let Some(idx) = self.selected
            && let Some(node) = self.navigator.entries.get(idx)
        {
            self.camera.set_target(Vec3::new(
                node.grid_pos.0 as f32 * config::GRID_SPACING,
                0.0,
                node.grid_pos.1 as f32 * config::GRID_SPACING,
            ));
        }
    }

    fn on_directory_changed(&mut self) {
        self.selected = None;
        self.scan_effect.reset();
        self.camera.reset_target();
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
