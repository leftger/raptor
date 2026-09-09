use bevy::prelude::{Color, Vec3};

pub const WINDOW_TITLE: &str = "Raptor";
pub const WINDOW_WIDTH: u32 = 1280;
pub const WINDOW_HEIGHT: u32 = 720;

pub const DEFAULT_CAMERA_YAW: f32 = 0.8;
pub const DEFAULT_CAMERA_PITCH: f32 = 0.6;
pub const DEFAULT_CAMERA_DISTANCE: f32 = 25.0;
pub const MIN_CAMERA_DISTANCE: f32 = 5.0;
pub const MAX_CAMERA_DISTANCE: f32 = 50.0;
pub const MIN_CAMERA_PITCH: f32 = 0.1;
pub const MAX_CAMERA_PITCH: f32 = 1.4;
pub const CAMERA_ROTATION_SPEED: f32 = 0.01;
pub const CAMERA_ZOOM_SPEED: f32 = 2.0;
pub const CAMERA_LERP_FACTOR: f32 = 0.1;

pub const GRID_SPACING: f32 = 2.5;
pub const GRID_SIZE: i32 = 20;

pub const BLOCK_WIDTH: f32 = 2.0;
pub const BLOCK_DEPTH: f32 = 2.0;
pub const MIN_BLOCK_HEIGHT: f32 = 0.5;
pub const MAX_BLOCK_HEIGHT: f32 = 3.0;
pub const DIR_HEIGHT_MULTIPLIER: f32 = 0.5;
pub const DIR_HEIGHT_BASE: f32 = 1.0;
pub const FILE_HEIGHT_LOG_SCALE: f32 = 0.3;

pub const BACKGROUND_COLOR: Color = Color::srgb(0.02, 0.02, 0.02);
pub const GRID_COLOR: Color = Color::srgba(0.0, 1.0, 0.25, 0.15);
pub const DIR_COLOR: Color = Color::srgba(0.0, 1.0, 0.25, 0.8);
pub const FILE_COLOR: Color = Color::srgba(0.0, 0.83, 1.0, 0.8);
pub const SELECTED_COLOR: Color = Color::srgba(1.0, 0.0, 1.0, 0.9);
pub const HOVER_DIR_COLOR: Color = Color::srgba(0.3, 1.0, 0.5, 0.9);
pub const HOVER_FILE_COLOR: Color = Color::srgba(0.3, 0.9, 1.0, 0.9);
pub const SCAN_COLOR: Color = Color::srgba(0.0, 1.0, 0.25, 0.3);
pub const UI_PANEL_COLOR: Color = Color::srgba(0.0, 0.05, 0.0, 0.9);
pub const UI_BREADCRUMB_COLOR: Color = Color::srgba(0.0, 0.03, 0.0, 0.95);
pub const TEXT_PRIMARY: Color = Color::srgb(0.0, 1.0, 0.25);
pub const TEXT_SECONDARY: Color = Color::srgb(0.0, 0.83, 1.0);
pub const TEXT_HIGHLIGHT: Color = Color::srgb(1.0, 0.0, 1.0);
pub const TEXT_WARNING: Color = Color::srgba(1.0, 1.0, 0.0, 0.9);

pub const SCAN_SPEED: f32 = 20.0;
pub const SCAN_MAX_HEIGHT: f32 = 30.0;

pub const HEADER_FONT_SIZE: f32 = 20.0;
pub const INFO_FONT_SIZE: f32 = 14.0;
pub const LABEL_FONT_SIZE: f32 = 12.0;
pub const LABEL_FOCUSED_FONT_SIZE: f32 = 16.0;

pub const HEADER_HEIGHT: f32 = 50.0;
pub const BREADCRUMB_HEIGHT: f32 = 25.0;
pub const FOOTER_HEIGHT: f32 = 80.0;
pub const LABEL_MAX_LENGTH: usize = 12;

pub const VIGNETTE_ALPHA: f32 = 0.2;
pub const SCANLINE_STEP: usize = 4;
pub const SCANLINE_HEIGHT: f32 = 2.0;
pub const SCANLINE_ALPHA: f32 = 0.1;

/// Convert a macroquad-style grid coordinate to world coordinates.
pub fn world_position(x: i32, z: i32, height: f32) -> Vec3 {
    Vec3::new(
        x as f32 * GRID_SPACING,
        height / 2.0,
        z as f32 * GRID_SPACING,
    )
}

/// Returns the height the block occupies (the original macroquad code kept block centers at
/// `height / 2.0`; callers can add `height` for tops when placing labels).
pub fn block_top_position(x: i32, z: i32, height: f32) -> Vec3 {
    Vec3::new(
        x as f32 * GRID_SPACING,
        height + 0.5,
        z as f32 * GRID_SPACING,
    )
}
