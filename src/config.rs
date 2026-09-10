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
pub const MAX_DIRECTORY_ENTRIES: usize = 30_000;
pub const DIR_CHILD_COUNT_CAP: usize = 500;
pub const MESH_CHUNK_SIZE: usize = 512;
pub const LABEL_BUDGET: usize = 512;

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

pub const LIGHTCYCLE_CELLS_PER_SEC: f32 = 3.5;
pub const LIGHTCYCLE_FIXED_STEP: f32 = 1.0 / 60.0;
pub const LIGHTCYCLE_MAX_SUBSTEPS: usize = 4;
/// How early (in cells) the rendered path begins curving before an intersection.
pub const LIGHTCYCLE_TURN_RADIUS: f32 = 0.4;
/// How far the cycle banks into a corner, in radians.
pub const LIGHTCYCLE_LEAN_ANGLE: f32 = 0.55;
pub const LIGHTCYCLE_CRASH_FX_DURATION: f32 = 0.6;
pub const LIGHTCYCLE_ARENA_PADDING: i32 = 1;
/// Smallest arena side, in cells. Arenas are square, so a folder with one or two
/// entries still gets room to turn around instead of a shallow corridor.
pub const LIGHTCYCLE_MIN_ARENA_SPAN: i32 = 9;
/// Percentage of interior cells that seed a short procedural barrier. Routes
/// to every filesystem landmark are carved after generation.
pub const LIGHTCYCLE_STREET_WALL_SEED_CHANCE: u8 = 9;
pub const LIGHTCYCLE_SPAWN_SEARCH_RADIUS: i32 = 4096;
/// Tower lattice spacing in cells. Each original grid row/column is multiplied
/// by this stride, leaving empty street cells between directory/file towers.
pub const LIGHTCYCLE_TOWER_STRIDE: i32 = 3;
pub const LIGHTCYCLE_TOWER_SIZE: f32 = 2.2;

/// glTF scene rendered as the player's cycle, relative to the `assets` directory.
pub const LIGHTCYCLE_MODEL_ASSET: &str = "models/light_cycle/scene.gltf";
/// Uniform scale for the cycle model. The source asset is 3.31 units long, so
/// this renders the cycle just under one grid cell long.
pub const LIGHTCYCLE_MODEL_SCALE: f32 = 0.72;
/// The model's nose points down its local -X, while gameplay drives entities
/// forward along +X.
pub const LIGHTCYCLE_MODEL_YAW: f32 = std::f32::consts::PI;
/// Rendered height of the scaled cycle model.
pub const LIGHTCYCLE_CYCLE_HEIGHT: f32 = 1.0;
pub const LIGHTCYCLE_TRAIL_HEIGHT: f32 = 1.85;
/// Thin enough to read as a sheet of glass rather than a stack of bricks.
pub const LIGHTCYCLE_TRAIL_THICKNESS: f32 = 0.14;
/// How far behind the cycle origin the wall is born, in cells. Matches the
/// scaled model's rear axle so the sheet appears to leave the tail, not the
/// cell center.
pub const LIGHTCYCLE_TRAIL_TAIL: f32 = 0.42;
/// Distance along the ribbon, in cells, over which the wall grows from a
/// meniscus at the tail to full height.
pub const LIGHTCYCLE_TRAIL_EMANATE: f32 = 0.55;
pub const LIGHTCYCLE_TRAIL_SPAWN_HEIGHT: f32 = 0.16;
pub const LIGHTCYCLE_WALL_HEIGHT: f32 = 1.4;
pub const LIGHTCYCLE_WALL_THICKNESS: f32 = 0.2;
pub const LIGHTCYCLE_STREET_WALL_HEIGHT: f32 = 1.1;
pub const LIGHTCYCLE_STREET_WALL_SIZE: f32 = 1.75;
pub const LIGHTCYCLE_PORTAL_HEIGHT: f32 = 2.6;
/// How many wall cells the parent gate covers. Wide enough that reaching the
/// parent directory does not need single-cell precision.
pub const LIGHTCYCLE_PORTAL_WIDTH_CELLS: i32 = 3;
/// Thickness of the gate's posts and lintel.
pub const LIGHTCYCLE_PORTAL_FRAME_THICKNESS: f32 = 0.28;
/// Radians per second of the gate frame's brightness pulse.
pub const LIGHTCYCLE_PORTAL_PULSE_SPEED: f32 = 3.2;
/// Light bars sweeping up through the gate's opening.
pub const LIGHTCYCLE_PORTAL_BAR_COUNT: usize = 4;
/// Full sweeps of the opening per second.
pub const LIGHTCYCLE_PORTAL_BAR_SPEED: f32 = 0.45;
pub const LIGHTCYCLE_PORTAL_BAR_HEIGHT: f32 = 0.18;
pub const LIGHTCYCLE_PORTAL_BAR_ALPHA: f32 = 0.5;

pub const LIGHTCYCLE_TRAIL_COLOR: Color = Color::srgba(0.55, 0.95, 1.0, 1.0);
/// Tint of light passing through the trail, slightly greener than the surface
/// so the sheet reads as thick glass rather than a cyan decal.
pub const LIGHTCYCLE_TRAIL_ATTENUATION: Color = Color::srgba(0.35, 0.9, 0.85, 1.0);
pub const LIGHTCYCLE_WALL_COLOR: Color = Color::srgba(0.0, 0.7, 0.6, 1.0);
pub const LIGHTCYCLE_STREET_WALL_COLOR: Color = Color::srgba(0.05, 0.42, 0.68, 1.0);
pub const LIGHTCYCLE_PORTAL_COLOR: Color = Color::srgba(1.0, 0.85, 0.1, 1.0);
/// Trough of the gate frame's pulse.
pub const LIGHTCYCLE_PORTAL_DIM_COLOR: Color = Color::srgba(0.32, 0.25, 0.03, 1.0);

pub const LIGHTCYCLE_CAMERA_DISTANCE: f32 = 14.0;
pub const LIGHTCYCLE_CAMERA_HEIGHT: f32 = 8.0;
pub const LIGHTCYCLE_CAMERA_LOOKAHEAD: f32 = 4.0;
/// Time constant for the chase camera easing onto a new heading, in seconds.
/// Without this lag a corner looks like the world rotating around a still bike.
pub const LIGHTCYCLE_CAMERA_TURN_LAG: f32 = 0.28;

/// Convert a grid coordinate to a point on the ground plane.
pub fn ground_position(x: i32, z: i32) -> Vec3 {
    Vec3::new(x as f32 * GRID_SPACING, 0.0, z as f32 * GRID_SPACING)
}

/// Convert a grid coordinate to world coordinates at the block center.
pub fn world_position(x: i32, z: i32, height: f32) -> Vec3 {
    let mut position = ground_position(x, z);
    position.y = height / 2.0;
    position
}

/// World position just above a block, used for labels.
pub fn block_top_position(x: i32, z: i32, height: f32) -> Vec3 {
    let mut position = ground_position(x, z);
    position.y = height + 0.5;
    position
}
