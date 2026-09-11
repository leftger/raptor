use crate::asteroids::{AsteroidsPhase, AsteroidsSim};
use crate::bomberman::{BomberPhase, BomberSim};
use crate::breaker::{BreakerPhase, BreakerSim};
use crate::columns::{ColumnsPhase, ColumnsSim};
use crate::config;
use crate::disc::{
    DiscEvents, DiscLayout, DiscPhase, DiscSim, PlayerSnapshot, SourceGame, SourceLanguage,
    SourceLoadFailed, SourceLoadState, SourceLoaded, SourceRequested, build_capped_disc_arena,
    build_disc_arena, build_flat_arena,
};
use crate::document::{
    DocumentLayout, DocumentLoadFailed, DocumentLoadState, DocumentLoaded, DocumentRequested,
    build_document_arena_from_parse,
    parse::{ParseLimits, parse_markdown_bytes},
};
use crate::filesystem::FileNode;
use crate::frogger::{FroggerPhase, FroggerSim};
use crate::galaga::{GalagaPhase, GalagaSim};
use crate::lightcycle::logic::{
    Arena, ArenaKind, CellContent, CityStructure, CityStructureKind, CityTheme, CrashReason,
    GatePlacement, Heading, LightcycleSim, ParentPortal, RunPhase, StepOutcome, Wall,
    classify_next_content,
};
use crate::lightcycle::{ActiveRun, LightcycleState, RunEnvironment, SourceSim};
use crate::load::{DirectoryLoadFailed, DirectoryLoaded, DirectoryRequested};
use crate::music::MusicSfx;
use crate::pacman::{PacPhase, PacSim};
use crate::platformer::{PlatformerPhase, PlatformerSim};
use crate::plinko::{PlinkoPhase, PlinkoSim};
use crate::plugins::transition::{ModeTransition, gods_eye_pose};
use crate::qbert::{QbertPhase, QbertSim};
use crate::snake::SnakeSim;
use crate::state::{
    DirectorySceneRoot, InteractionMode, LightcycleSceneRoot, NavigatorResource,
    OrbitCameraResource, TrailSceneRoot,
};
use crate::stealth::{StealthPhase, StealthSim};
use crate::surfer::{SurferPhase, SurferSim};
use crate::tetris::{TetrisPhase, TetrisSim};
use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use std::collections::HashMap;
use std::path::Path;

pub struct LightcyclePlugin;

impl Plugin for LightcyclePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InteractionMode>()
            .init_resource::<LightcycleState>()
            .add_message::<DocumentRequested>()
            .add_message::<DocumentLoaded>()
            .add_message::<DocumentLoadFailed>()
            .add_message::<SourceRequested>()
            .add_message::<SourceLoaded>()
            .add_message::<SourceLoadFailed>()
            .add_systems(Startup, setup_lightcycle_assets)
            .add_systems(
                Update,
                (
                    (
                        toggle_mode,
                        apply_mode_swap,
                        reset_on_directory_loaded,
                        start_document_loads,
                        poll_document_loads,
                        reset_on_document_loaded,
                        start_source_loads,
                        poll_source_loads,
                        reset_on_source_loaded,
                        apply_load_failure,
                        apply_document_load_failure,
                        apply_source_load_failure,
                        sync_directory_scene_visibility,
                    ),
                    (
                        read_lightcycle_input.run_if(in_lightcycle_mode),
                        step_lightcycle.run_if(in_lightcycle_mode),
                        restore_directory_arena.run_if(in_lightcycle_mode),
                        spawn_crash_effect.run_if(in_lightcycle_mode),
                        update_crash_effects.run_if(in_lightcycle_mode),
                        update_trail_mesh.run_if(in_lightcycle_mode),
                        animate_parent_gate.run_if(in_lightcycle_mode),
                        animate_city_beacons.run_if(in_lightcycle_mode),
                        update_document_focus.run_if(in_lightcycle_mode),
                        update_disc_focus.run_if(in_lightcycle_mode),
                        sync_disc_entities.run_if(in_lightcycle_mode),
                    ),
                    (
                        sync_asteroid_entities.run_if(in_lightcycle_mode),
                        sync_galaga_entities.run_if(in_lightcycle_mode),
                        sync_pacman_entities.run_if(in_lightcycle_mode),
                        sync_columns_entities.run_if(in_lightcycle_mode),
                        sync_tetris_entities.run_if(in_lightcycle_mode),
                        sync_frogger_entities.run_if(in_lightcycle_mode),
                        sync_qbert_entities.run_if(in_lightcycle_mode),
                        sync_bomberman_entities.run_if(in_lightcycle_mode),
                        sync_plinko_entities.run_if(in_lightcycle_mode),
                        sync_snake_entities.run_if(in_lightcycle_mode),
                        sync_character_entities.run_if(in_lightcycle_mode),
                        tag_character_model.run_if(in_lightcycle_mode),
                        prepare_character_walk.run_if(in_lightcycle_mode),
                        drive_character_walk.run_if(in_lightcycle_mode),
                        sync_breaker_entities.run_if(in_lightcycle_mode),
                        sync_stealth_entities.run_if(in_lightcycle_mode),
                        fit_guard_cones.run_if(in_lightcycle_mode),
                        animate_disc_pickups.run_if(in_lightcycle_mode),
                        update_cycle_transform.run_if(in_lightcycle_mode),
                        update_chase_camera.run_if(in_lightcycle_mode),
                    ),
                )
                    .chain()
                    .after(crate::plugins::filesystem::apply_loaded),
            )
            .add_systems(
                Update,
                (
                    cleanup_orphaned_entry_effect
                        .after(read_lightcycle_input)
                        .before(spawn_entry_effect),
                    spawn_entry_effect.after(step_lightcycle),
                    animate_entry_effect
                        .after(spawn_entry_effect)
                        .after(update_cycle_transform)
                        .before(update_chase_camera),
                )
                    .run_if(in_lightcycle_mode),
            );
    }
}

#[derive(Resource)]
struct LightcycleAssets {
    unit_cube: Handle<Mesh>,
    entry_beam_mesh: Handle<Mesh>,
    entry_halo_mesh: Handle<Mesh>,
    cycle_scene: Handle<WorldAsset>,
    trail_material: Handle<StandardMaterial>,
    wall_material: Handle<StandardMaterial>,
    city_floor_material: Handle<StandardMaterial>,
    city_foundation_material: Handle<StandardMaterial>,
    city_glass_material: Handle<StandardMaterial>,
    city_accent_materials: [[Handle<StandardMaterial>; 2]; 4],
    portal_material: Handle<StandardMaterial>,
    portal_bar_material: Handle<StandardMaterial>,
    dir_tower_material: Handle<StandardMaterial>,
    file_tower_material: Handle<StandardMaterial>,
    markdown_tower_material: Handle<StandardMaterial>,
    source_tower_material: Handle<StandardMaterial>,
    disc_floor_material: Handle<StandardMaterial>,
    disc_ring_material: Handle<StandardMaterial>,
    disc_plinth_material: Handle<StandardMaterial>,
    disc_hazard_material: Handle<StandardMaterial>,
    disc_opponent_material: Handle<StandardMaterial>,
    disc_player_disc_material: Handle<StandardMaterial>,
    disc_pickup_material: Handle<StandardMaterial>,
    disc_safe_pad_material: Handle<StandardMaterial>,
    /// One accent per [`SourceLanguage`], indexed by [`disc_language_index`].
    disc_accent_materials: [Handle<StandardMaterial>; SourceLanguage::COUNT],
    /// Flat cylinder thrown and returned during a fight.
    disc_mesh: Handle<Mesh>,
    /// Taller cylinder body for the Recognizer opponent.
    recognizer_mesh: Handle<Mesh>,
    /// Blocky rock body and beam tracer for the asteroid field.
    rock_material: Handle<StandardMaterial>,
    beam_material: Handle<StandardMaterial>,
    /// Power-up orb and sealed-exit bar for a snake ring.
    snake_food_material: Handle<StandardMaterial>,
    snake_lock_material: Handle<StandardMaterial>,
    /// The Tron runner, and the slabs and door of a platformer level.
    tron_scene: Handle<WorldAsset>,
    /// The same file as a `Gltf`, for the walk clip the scene cannot expose.
    tron_gltf: Handle<Gltf>,
    platform_material: Handle<StandardMaterial>,
    exit_material: Handle<StandardMaterial>,
    /// Bricks, ball and court walls for the breaker.
    brick_material: Handle<StandardMaterial>,
    ball_material: Handle<StandardMaterial>,
    court_material: Handle<StandardMaterial>,
    /// Floor, cover, guards and their vision cones for the stealth run.
    stealth_floor_material: Handle<StandardMaterial>,
    stealth_wall_material: Handle<StandardMaterial>,
    stealth_cone_material: Handle<StandardMaterial>,
    stealth_exit_material: Handle<StandardMaterial>,
    /// Unit-length cone with the stealth half-angle, scaled by its range.
    vision_cone: Handle<Mesh>,
    /// Water ribbon, rocks, boost gates and the finish gate for the surfer.
    surfer_water_material: Handle<StandardMaterial>,
    surfer_rock_material: Handle<StandardMaterial>,
    surfer_gate_material: Handle<StandardMaterial>,
    surfer_finish_material: Handle<StandardMaterial>,
    /// Bug bodies and beam bolts for the Galaga field.
    galaga_bug_material: Handle<StandardMaterial>,
    galaga_beam_material: Handle<StandardMaterial>,
    /// Arcade block: shared materials for the seven fixed-screen games.
    gem_materials: [Handle<StandardMaterial>; 4],
    tetris_materials: [Handle<StandardMaterial>; 7],
    qbert_cube_dim: Handle<StandardMaterial>,
    qbert_cube_lit: Handle<StandardMaterial>,
    qbert_enemy_material: Handle<StandardMaterial>,
    plinko_pin_material: Handle<StandardMaterial>,
    plinko_ball_material: Handle<StandardMaterial>,
    bomber_crate_material: Handle<StandardMaterial>,
    bomber_bomb_material: Handle<StandardMaterial>,
    document_floor_material: Handle<StandardMaterial>,
    document_rule_material: Handle<StandardMaterial>,
    document_margin_material: Handle<StandardMaterial>,
    document_ink_material: Handle<StandardMaterial>,
    document_heading_material: Handle<StandardMaterial>,
    document_folio_material: Handle<StandardMaterial>,
    document_focus_material: Handle<StandardMaterial>,
    crash_material: Handle<StandardMaterial>,
    entry_beam_material: Handle<StandardMaterial>,
    entry_halo_material: Handle<StandardMaterial>,
}

#[derive(Component)]
struct CycleEntity;

/// Small mesh burst emitted at the crash point.
#[derive(Component)]
struct CrashDebris {
    velocity: Vec3,
    life: f32,
    max_life: f32,
    initial_scale: f32,
}

/// Entity owned by the directory-tower transport effect.
#[derive(Component)]
struct EntryTransportEntity;

#[derive(Component)]
struct EntryBeam;

#[derive(Component)]
struct EntryHalo {
    phase: f32,
}

/// A post or lintel of the parent gate.
#[derive(Component)]
struct GateFrame;

/// A light bar sweeping up through the parent gate's opening.
#[derive(Component)]
struct GateScanBar {
    /// Position in the sweep at startup, so the bars are evenly spaced.
    offset: f32,
    /// Height of the opening the bar travels up before wrapping.
    travel: f32,
}

#[derive(Component)]
struct CityBeacon {
    base_height: f32,
    phase: f32,
}

#[derive(Component)]
struct DocumentFocusMarker;

/// The player's thrown disc.
#[derive(Component)]
struct PlayerDiscEntity;

/// The Recognizer opponent's body.
#[derive(Component)]
struct OpponentEntity;

/// The opponent's disc.
#[derive(Component)]
struct OpponentDiscEntity;

/// One pickup waiting on a ring floor, keyed into `DiscLayout::pickups`.
#[derive(Component)]
struct DiscPickupEntity {
    index: usize,
    phase: f32,
}

/// One pooled rock in the asteroid field, keyed into `AsteroidsSim::rocks`.
#[derive(Component)]
struct RockEntity {
    index: usize,
}

/// One pooled beam in the asteroid field, keyed into `AsteroidsSim::beams`.
#[derive(Component)]
struct BeamEntity {
    index: usize,
}

/// One pooled bug in the Galaga field, keyed into `GalagaSim::bugs`.
#[derive(Component)]
struct BugEntity {
    index: usize,
}

/// One pooled beam in the Galaga field, keyed into `GalagaSim::beams`.
#[derive(Component)]
struct GalagaBeamEntity {
    index: usize,
}

/// One pooled dot in the Pac-Man maze, keyed by its cell.
#[derive(Component)]
struct DotEntity {
    cell: (i32, i32),
}

/// One pooled ghost in the Pac-Man maze, keyed into `PacSim::ghosts`.
#[derive(Component)]
struct GhostEntity {
    index: usize,
}

/// One pooled cell of the Columns well, keyed by its row-major index.
#[derive(Component)]
struct GemEntity {
    index: usize,
}

/// One pooled cell of the Tetris board, keyed by its row-major index.
#[derive(Component)]
struct BlockEntity {
    index: usize,
}

/// One pooled obstacle cube on the Frogger highway.
#[derive(Component)]
struct FrogObstacleEntity {
    index: usize,
}

/// One cube of the Q*bert pyramid, keyed by its row/index pair.
#[derive(Component)]
struct QbertCubeEntity {
    row: usize,
    index: usize,
}

/// One pooled enemy on the Q*bert pyramid, keyed into `QbertSim::enemies`.
#[derive(Component)]
struct QbertEnemyEntity {
    index: usize,
}

/// One pooled crate in the Bomberman room, keyed by its cell.
#[derive(Component)]
struct BomberCrateEntity {
    cell: (i32, i32),
}

/// One pooled bomb in the Bomberman room, keyed into `BomberSim::bombs`.
#[derive(Component)]
struct BomberBombEntity {
    index: usize,
}

/// One pooled ball on the Plinko board, keyed into `PlinkoSim::balls`.
#[derive(Component)]
struct PlinkoBallEntity {
    index: usize,
}

/// One power-up on a snake ring, keyed into `SnakeSim::food`.
#[derive(Component)]
struct SnakeFoodEntity {
    index: usize,
}

/// The bar sealing a snake ring's exit until enough power-ups are collected.
#[derive(Component)]
struct SnakeGateLock;

/// The Tron runner, on a platformer level or a stealth run.
#[derive(Component)]
struct CharacterEntity;

/// Easing state for the on-foot character.
///
/// The walk itself is the asset's animation clip, so the only thing left to
/// track here is `base`: the ground position the figure is easing toward, which
/// smooths the stealth sim's whole-cell steps into a glide.
#[derive(Component)]
struct CharacterAnim {
    base: Vec3,
}

impl CharacterAnim {
    fn at(base: Vec3) -> Self {
        Self { base }
    }
}

/// The character's walk clip, once its animation graph has been built.
///
/// The glTF loader creates the `AnimationPlayer` but no graph, so one is built
/// from the clip the asset carries.
#[derive(Component)]
struct CharacterWalk(AnimationNodeIndex);

/// Marks everything spawned beneath an on-foot character.
///
/// The loader creates the `AnimationPlayer` deep inside the scene, so there is
/// nothing to tag at spawn time: the character's subtree is walked instead,
/// which also picks up descendants that only appear a frame or two later.
#[derive(Component)]
struct CharacterModel;

/// The breaker's ball.
#[derive(Component)]
struct BallEntity;

/// One brick of a breaker wall, keyed into `BreakerSim::bricks`.
#[derive(Component)]
struct BrickEntity {
    index: usize,
}

/// One patrol's body, keyed into `StealthSim::guards`.
#[derive(Component)]
struct GuardEntity {
    index: usize,
}

/// One patrol's field-of-vision cone, keyed into `StealthSim::guards`.
#[derive(Component)]
struct GuardConeEntity {
    index: usize,
    /// This guard's own cone, because its shape is cut to what the guard can
    /// actually see. It starts as the plain fan and is replaced once the sim's
    /// rays are available, which is on the first frame.
    mesh: Option<Handle<Mesh>>,
}

/// Direction the chase camera is currently following.
///
/// This trails the cycle's own heading so a corner reads as the cycle swinging
/// across the frame. Locking the camera to the cycle instead makes the world
/// appear to rotate around a stationary bike. It lives on the cycle so each run
/// starts from the spawn heading.
#[derive(Component)]
struct ChaseCamera {
    forward: Vec3,
    /// Right-drag free look, in radians: `x` swings the rig around the cycle and
    /// `y` raises it above the default chase pitch. Held only while dragging;
    /// releasing the button eases it back to zero.
    look: Vec2,
}

impl ChaseCamera {
    /// Folds one frame of right-drag mouse motion into the free-look offset.
    ///
    /// Both axes are negated to match the explorer's orbit camera, which
    /// measures its own yaw and pitch in the opposite sense. Pitch is clamped
    /// here rather than only at render time so holding a drag past the limit
    /// cannot bank up rotation that the next drag has to spend undoing.
    fn apply_look_drag(&mut self, delta: Vec2) {
        let base_pitch = chase_base_pitch();
        self.look.x = wrap_angle(self.look.x - delta.x * config::CAMERA_ROTATION_SPEED);
        self.look.y = (self.look.y - delta.y * config::CAMERA_ROTATION_SPEED).clamp(
            config::LIGHTCYCLE_CAMERA_MIN_PITCH - base_pitch,
            config::LIGHTCYCLE_CAMERA_MAX_PITCH - base_pitch,
        );
    }

    /// Eases free look back behind the cycle after the button is released, with
    /// a frame-rate independent time constant.
    fn recenter_look(&mut self, delta_seconds: f32) {
        if self.look == Vec2::ZERO {
            return;
        }

        let blend = 1.0 - (-delta_seconds / config::LIGHTCYCLE_CAMERA_LOOK_RECENTER).exp();
        self.look = self.look.lerp(Vec2::ZERO, blend.clamp(0.0, 1.0));

        // An exponential ease never quite arrives, so land it rather than
        // leaving the camera drifting by fractions of a degree forever.
        if self.look.length_squared() < LOOK_RECENTER_SNAP * LOOK_RECENTER_SNAP {
            self.look = Vec2::ZERO;
        }
    }
}

/// Free-look offset below which recentering snaps home, in radians.
const LOOK_RECENTER_SNAP: f32 = 1.0e-3;

/// Wraps an angle into `[-PI, PI)` so recentering unwinds the short way round
/// however many times a drag has spun the camera about the cycle.
fn wrap_angle(angle: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    (angle + PI).rem_euclid(TAU) - PI
}

fn unlit_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        unlit: true,
        ..default()
    }
}

fn neon_material(color: Color, emissive: LinearRgba) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        emissive,
        unlit: true,
        ..default()
    }
}

fn city_palette() -> [[(Color, LinearRgba); 2]; 4] {
    [
        [
            (config::LIGHTCYCLE_CITY_CYAN, LinearRgba::rgb(0.0, 2.6, 3.4)),
            (
                config::LIGHTCYCLE_CITY_BLUE,
                LinearRgba::rgb(0.15, 1.0, 3.0),
            ),
        ],
        [
            (
                config::LIGHTCYCLE_CITY_MAGENTA,
                LinearRgba::rgb(3.4, 0.03, 2.0),
            ),
            (config::LIGHTCYCLE_CITY_CYAN, LinearRgba::rgb(0.0, 2.4, 3.2)),
        ],
        [
            (
                config::LIGHTCYCLE_CITY_VIOLET,
                LinearRgba::rgb(1.8, 0.18, 3.4),
            ),
            (config::LIGHTCYCLE_CITY_PINK, LinearRgba::rgb(3.4, 0.2, 1.2)),
        ],
        [
            (
                config::LIGHTCYCLE_CITY_AMBER,
                LinearRgba::rgb(3.4, 1.1, 0.03),
            ),
            (config::LIGHTCYCLE_CITY_CYAN, LinearRgba::rgb(0.0, 2.4, 3.2)),
        ],
    ]
}

/// Lane markings use a district's primary accent, so ground seams use the
/// secondary one to stay readable against them.
const CITY_TRIM_ACCENT: usize = 1;

fn city_theme_index(theme: CityTheme) -> usize {
    match theme {
        CityTheme::Cyan => 0,
        CityTheme::Magenta => 1,
        CityTheme::Violet => 2,
        CityTheme::Amber => 3,
    }
}

/// Index into [`LightcycleAssets::disc_accent_materials`].
fn disc_language_index(language: SourceLanguage) -> usize {
    SourceLanguage::ALL
        .iter()
        .position(|candidate| *candidate == language)
        .unwrap_or(0)
}

/// Lit transmissive sheet: the directional light and the arena behind it show
/// through, with a cyan tint and a hard specular so it reads as glass rather
/// than an unlit neon brick.
fn trail_glass_material() -> StandardMaterial {
    StandardMaterial {
        base_color: config::LIGHTCYCLE_TRAIL_COLOR,
        perceptual_roughness: 0.08,
        metallic: 0.02,
        specular_transmission: 0.92,
        thickness: 0.28,
        ior: 1.45,
        attenuation_color: config::LIGHTCYCLE_TRAIL_ATTENUATION,
        attenuation_distance: 0.8,
        emissive: LinearRgba::rgb(0.05, 0.55, 0.7),
        clearcoat: 1.0,
        clearcoat_perceptual_roughness: 0.06,
        double_sided: true,
        cull_mode: None,
        ..default()
    }
}

fn setup_lightcycle_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let palette = city_palette();
    let city_accent_materials = std::array::from_fn(|theme| {
        std::array::from_fn(|accent| {
            let (color, emissive) = palette[theme][accent];
            materials.add(neon_material(color, emissive))
        })
    });
    commands.insert_resource(LightcycleAssets {
        unit_cube: meshes.add(Cuboid::default()),
        entry_beam_mesh: meshes.add(Cylinder::new(
            config::LIGHTCYCLE_ENTRY_BEAM_RADIUS,
            config::LIGHTCYCLE_ENTRY_BEAM_HEIGHT,
        )),
        entry_halo_mesh: meshes.add(Torus::new(
            config::LIGHTCYCLE_ENTRY_HALO_INNER_RADIUS,
            config::LIGHTCYCLE_ENTRY_HALO_OUTER_RADIUS,
        )),
        disc_mesh: meshes.add(Cylinder::new(
            config::DISC_MESH_RADIUS,
            config::DISC_MESH_THICKNESS,
        )),
        recognizer_mesh: meshes.add(Cylinder::new(
            config::RECOGNIZER_RADIUS,
            config::RECOGNIZER_HEIGHT,
        )),
        rock_material: materials.add(StandardMaterial {
            base_color: config::ASTEROIDS_ROCK_COLOR,
            emissive: LinearRgba::from(config::ASTEROIDS_ROCK_CORE_COLOR) * 0.3,
            perceptual_roughness: 0.92,
            ..default()
        }),
        beam_material: materials.add(StandardMaterial {
            base_color: config::ASTEROIDS_BEAM_COLOR,
            emissive: LinearRgba::from(config::ASTEROIDS_BEAM_COLOR) * 3.4,
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        }),
        snake_food_material: materials.add(StandardMaterial {
            base_color: config::SNAKE_FOOD_COLOR,
            emissive: LinearRgba::from(config::SNAKE_FOOD_COLOR) * 2.2,
            unlit: true,
            ..default()
        }),
        snake_lock_material: materials.add(StandardMaterial {
            base_color: config::SNAKE_GATE_COLOR,
            emissive: LinearRgba::from(config::SNAKE_GATE_COLOR) * 1.6,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        tron_scene: asset_server
            .load(GltfAssetLabel::Scene(0).from_asset(config::TRON_MODEL_ASSET)),
        tron_gltf: asset_server.load(config::TRON_MODEL_ASSET),
        platform_material: materials.add(unlit_material(config::PLATFORMER_PLATFORM_COLOR)),
        exit_material: materials.add(StandardMaterial {
            base_color: config::PLATFORMER_EXIT_COLOR,
            emissive: LinearRgba::from(config::PLATFORMER_EXIT_COLOR) * 2.4,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        brick_material: materials.add(StandardMaterial {
            base_color: config::BREAKER_BRICK_COLOR,
            emissive: LinearRgba::from(config::BREAKER_BRICK_COLOR) * 1.5,
            unlit: true,
            ..default()
        }),
        ball_material: materials.add(StandardMaterial {
            base_color: config::BREAKER_BALL_COLOR,
            emissive: LinearRgba::from(config::BREAKER_BALL_COLOR) * 3.0,
            unlit: true,
            ..default()
        }),
        court_material: materials.add(unlit_material(config::BREAKER_WALL_COLOR)),
        stealth_floor_material: materials.add(unlit_material(config::STEALTH_FLOOR_COLOR)),
        stealth_wall_material: materials.add(StandardMaterial {
            base_color: config::STEALTH_WALL_COLOR,
            emissive: LinearRgba::from(config::STEALTH_WALL_COLOR) * 0.6,
            unlit: true,
            ..default()
        }),
        stealth_cone_material: materials.add(StandardMaterial {
            base_color: config::STEALTH_CONE_COLOR,
            emissive: LinearRgba::from(config::STEALTH_CONE_COLOR) * 1.4,
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        }),
        stealth_exit_material: materials.add(StandardMaterial {
            base_color: config::STEALTH_EXIT_COLOR,
            emissive: LinearRgba::from(config::STEALTH_EXIT_COLOR) * 2.4,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        vision_cone: meshes.add(vision_cone_mesh(
            config::STEALTH_VISION_HALF_ANGLE,
            config::STEALTH_CONE_SEGMENTS,
        )),
        surfer_water_material: materials.add(StandardMaterial {
            base_color: config::SURFER_WATER_COLOR,
            emissive: LinearRgba::from(config::SURFER_WATER_COLOR) * 0.2,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        surfer_rock_material: materials.add(unlit_material(config::SURFER_ROCK_COLOR)),
        surfer_gate_material: materials.add(StandardMaterial {
            base_color: config::SURFER_GATE_COLOR,
            emissive: LinearRgba::from(config::SURFER_GATE_COLOR) * 2.2,
            unlit: true,
            ..default()
        }),
        surfer_finish_material: materials.add(StandardMaterial {
            base_color: config::SURFER_FINISH_COLOR,
            emissive: LinearRgba::from(config::SURFER_FINISH_COLOR) * 2.4,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        galaga_bug_material: materials.add(StandardMaterial {
            base_color: config::GALAGA_BUG_COLOR,
            emissive: LinearRgba::from(config::GALAGA_BUG_COLOR) * 2.0,
            unlit: true,
            ..default()
        }),
        galaga_beam_material: materials.add(StandardMaterial {
            base_color: config::GALAGA_BEAM_COLOR,
            emissive: LinearRgba::from(config::GALAGA_BEAM_COLOR) * 3.0,
            unlit: true,
            ..default()
        }),
        gem_materials: std::array::from_fn(|index| {
            materials.add(StandardMaterial {
                base_color: config::COLUMNS_GEM_COLORS_LIST[index],
                emissive: LinearRgba::from(config::COLUMNS_GEM_COLORS_LIST[index]) * 1.6,
                unlit: true,
                ..default()
            })
        }),
        tetris_materials: std::array::from_fn(|index| {
            materials.add(StandardMaterial {
                base_color: config::TETRIS_COLORS[index],
                emissive: LinearRgba::from(config::TETRIS_COLORS[index]) * 1.4,
                unlit: true,
                ..default()
            })
        }),
        qbert_cube_dim: materials.add(unlit_material(config::QBERT_CUBE_DIM_COLOR)),
        qbert_cube_lit: materials.add(StandardMaterial {
            base_color: config::QBERT_CUBE_LIT_COLOR,
            emissive: LinearRgba::from(config::QBERT_CUBE_LIT_COLOR) * 1.8,
            unlit: true,
            ..default()
        }),
        qbert_enemy_material: materials.add(StandardMaterial {
            base_color: config::QBERT_ENEMY_COLOR,
            emissive: LinearRgba::from(config::QBERT_ENEMY_COLOR) * 2.0,
            unlit: true,
            ..default()
        }),
        plinko_pin_material: materials.add(unlit_material(config::PLINKO_PIN_COLOR)),
        plinko_ball_material: materials.add(StandardMaterial {
            base_color: config::PLINKO_BALL_COLOR,
            emissive: LinearRgba::from(config::PLINKO_BALL_COLOR) * 1.8,
            unlit: true,
            ..default()
        }),
        bomber_crate_material: materials.add(unlit_material(config::BOMBER_CRATE_COLOR)),
        bomber_bomb_material: materials.add(unlit_material(config::BOMBER_BOMB_COLOR)),
        cycle_scene: asset_server
            .load(GltfAssetLabel::Scene(0).from_asset(config::LIGHTCYCLE_MODEL_ASSET)),
        trail_material: materials.add(trail_glass_material()),
        wall_material: materials.add(unlit_material(config::LIGHTCYCLE_WALL_COLOR)),
        city_floor_material: materials.add(StandardMaterial {
            base_color: config::LIGHTCYCLE_CITY_FLOOR_COLOR,
            unlit: true,
            ..default()
        }),
        city_foundation_material: materials.add(StandardMaterial {
            base_color: config::LIGHTCYCLE_CITY_FOUNDATION_COLOR,
            unlit: true,
            ..default()
        }),
        city_glass_material: materials.add(StandardMaterial {
            base_color: config::LIGHTCYCLE_CITY_GLASS_COLOR,
            emissive: LinearRgba::rgb(0.02, 0.12, 0.25),
            metallic: 0.18,
            perceptual_roughness: 0.08,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        city_accent_materials,
        portal_material: materials.add(unlit_material(config::LIGHTCYCLE_PORTAL_COLOR)),
        portal_bar_material: materials.add(StandardMaterial {
            base_color: config::LIGHTCYCLE_PORTAL_COLOR
                .with_alpha(config::LIGHTCYCLE_PORTAL_BAR_ALPHA),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        dir_tower_material: materials.add(unlit_material(config::DIR_COLOR)),
        file_tower_material: materials.add(unlit_material(config::FILE_COLOR)),
        markdown_tower_material: materials.add(unlit_material(config::MARKDOWN_TOWER_COLOR)),
        source_tower_material: materials.add(neon_material(
            config::SOURCE_TOWER_COLOR,
            LinearRgba::rgb(2.2, 0.9, 0.05),
        )),
        document_floor_material: materials.add(unlit_material(config::DOCUMENT_FLOOR_COLOR)),
        document_rule_material: materials.add(unlit_material(config::DOCUMENT_RULE_COLOR)),
        document_margin_material: materials.add(unlit_material(config::DOCUMENT_MARGIN_COLOR)),
        document_ink_material: materials.add(neon_material(
            config::DOCUMENT_INK_COLOR,
            LinearRgba::from(config::DOCUMENT_INK_EMISSIVE),
        )),
        document_heading_material: materials.add(neon_material(
            config::DOCUMENT_HEADING_COLOR,
            LinearRgba::rgb(0.55, 0.28, 0.08),
        )),
        document_folio_material: materials.add(unlit_material(config::DOCUMENT_FOLIO_COLOR)),
        document_focus_material: materials.add(neon_material(
            config::DOCUMENT_FOCUS_COLOR,
            LinearRgba::rgb(2.4, 0.9, 0.1),
        )),
        crash_material: materials.add(unlit_material(Color::srgb(1.0, 0.45, 0.1))),
        entry_beam_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.25, 0.92, 1.0, 0.14),
            emissive: LinearRgba::rgb(0.08, 1.8, 2.8),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        entry_halo_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.7, 0.98, 1.0, 0.92),
            emissive: LinearRgba::rgb(3.2, 5.0, 6.0),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            ..default()
        }),
        disc_floor_material: materials.add(unlit_material(config::DISC_FLOOR_COLOR)),
        disc_ring_material: materials.add(neon_material(
            config::DISC_RING_COLOR,
            LinearRgba::rgb(0.05, 0.4, 0.62),
        )),
        disc_plinth_material: materials.add(unlit_material(config::DISC_RING_COLOR)),
        disc_hazard_material: materials.add(neon_material(
            config::DISC_HAZARD_COLOR,
            LinearRgba::rgb(2.4, 0.2, 0.1),
        )),
        disc_opponent_material: materials.add(neon_material(
            config::DISC_OPPONENT_COLOR,
            LinearRgba::rgb(2.4, 1.1, 0.1),
        )),
        disc_player_disc_material: materials.add(neon_material(
            config::DISC_PLAYER_DISC_COLOR,
            LinearRgba::rgb(0.6, 2.6, 3.0),
        )),
        disc_pickup_material: materials.add(neon_material(
            config::DISC_PICKUP_COLOR,
            LinearRgba::rgb(2.4, 2.1, 0.3),
        )),
        disc_safe_pad_material: materials.add(neon_material(
            config::DISC_SAFE_PAD_COLOR,
            LinearRgba::rgb(0.1, 1.4, 0.5),
        )),
        disc_accent_materials: std::array::from_fn(|index| {
            let accent = SourceLanguage::ALL[index].accent();
            materials.add(neon_material(accent, LinearRgba::from(accent)))
        }),
    });
}

fn in_lightcycle_mode(mode: Res<InteractionMode>) -> bool {
    *mode == InteractionMode::Lightcycle
}

fn sync_directory_scene_visibility(
    mode: Res<InteractionMode>,
    mut directory_scene: Query<&mut Visibility, With<DirectorySceneRoot>>,
) {
    let visible = *mode == InteractionMode::Explorer;
    for mut visibility in &mut directory_scene {
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn tower_position(grid_pos: (i32, i32)) -> (i32, i32) {
    (
        grid_pos.0 * config::LIGHTCYCLE_TOWER_STRIDE,
        grid_pos.1 * config::LIGHTCYCLE_TOWER_STRIDE,
    )
}

fn build_active_run(path: &Path, nodes: Vec<FileNode>) -> ActiveRun {
    let cells: HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (tower_position(node.grid_pos), index))
        .collect();
    let parent_gate = path
        .parent()
        .is_some()
        .then(|| GatePlacement::for_path(path, config::LIGHTCYCLE_PORTAL_WIDTH_CELLS));
    let mut arena = Arena::from_nodes(
        cells.keys().copied(),
        parent_gate,
        config::LIGHTCYCLE_ARENA_PADDING,
        config::LIGHTCYCLE_MIN_ARENA_SPAN,
    );
    arena.generate_city(
        path,
        cells.keys().copied(),
        config::LIGHTCYCLE_CITY_STRUCTURE_SEED_CHANCE,
        config::LIGHTCYCLE_TOWER_STRIDE,
    );

    let sim = spawn_sim(&arena, &cells);

    ActiveRun {
        sim,
        arena,
        environment: RunEnvironment::Directory { nodes, cells },
        crash_label: None,
        entering_label: None,
    }
}

fn build_document_run(path: &Path, bytes: &[u8]) -> ActiveRun {
    let parsed = parse_markdown_bytes(bytes, ParseLimits::default(), false);
    let (arena, layout) = build_document_arena_from_parse(path, parsed);
    let sim = spawn_sim(&arena, &HashMap::new());
    ActiveRun {
        sim,
        arena,
        environment: RunEnvironment::Document {
            path: path.to_path_buf(),
            name: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("document")
                .to_string(),
            layout,
            focused_block: None,
        },
        crash_label: None,
        entering_label: None,
    }
}

/// Builds the run for a source file: a fresh player cycle at the ring center
/// plus the sim for whichever game the language hosts.
fn build_source_run(path: &Path, language: SourceLanguage, bytes: &[u8]) -> ActiveRun {
    let game = language.game();
    let (arena, layout) = match game {
        // The field and the snake ring want a bounded playfield whatever the
        // file size; only disc wars scales its coliseum with the file.
        SourceGame::Asteroids => {
            build_capped_disc_arena(path, language, bytes, config::ASTEROIDS_RADIUS_CELLS)
        }
        SourceGame::Snake => {
            build_capped_disc_arena(path, language, bytes, config::SNAKE_RADIUS_CELLS)
        }
        SourceGame::DiscWars => build_disc_arena(path, language, bytes),
        // The off-grid games carry a metadata-only arena; their level is their
        // own. The half-extent only sizes the (unused) metadata box.
        SourceGame::Platformer | SourceGame::Breaker | SourceGame::Stealth => {
            build_flat_arena(path, language, bytes, config::PLATFORMER_HALF_EXTENT)
        }
        SourceGame::RiverSurfer => {
            build_flat_arena(path, language, bytes, config::SURFER_HALF_EXTENT)
        }
        SourceGame::Galaga => build_flat_arena(path, language, bytes, config::GALAGA_HALF_EXTENT),
        // The whole arcade block shares one arena extent; each game's board is
        // small enough to fit inside it.
        SourceGame::PacMan
        | SourceGame::Columns
        | SourceGame::Tetris
        | SourceGame::Frogger
        | SourceGame::Qbert
        | SourceGame::Bomberman
        | SourceGame::Plinko => build_flat_arena(path, language, bytes, config::ARCADE_HALF_EXTENT),
    };
    let sim = LightcycleSim::start(layout.player_spawn, layout.player_spawn_heading);
    let sim_state = match game {
        SourceGame::DiscWars => SourceSim::DiscWars(DiscSim::new(&layout)),
        SourceGame::Asteroids => {
            let mut field = Box::new(AsteroidsSim::new(
                layout.seed,
                ring_center_world(&layout),
                ring_radius_world(&layout),
            ));
            // The parked cycle starts on the ring's spawn heading so the aim
            // angle is already meaningful when the field ends and the bike is
            // handed back.
            field.angle = heading_facing(layout.player_spawn_heading);
            SourceSim::Asteroids(field)
        }
        SourceGame::Snake => SourceSim::Snake(SnakeSim::new(
            layout.seed,
            layout.player_spawn,
            &ring_food_cells(&arena),
            config::SNAKE_FOOD_TARGET,
        )),
        SourceGame::Platformer => SourceSim::Platformer(Box::new(PlatformerSim::new(
            layout.seed,
            level_metres(&layout),
        ))),
        SourceGame::Breaker => SourceSim::Breaker(Box::new(BreakerSim::new(layout.seed))),
        SourceGame::Stealth => SourceSim::Stealth(Box::new(StealthSim::new(layout.seed))),
        SourceGame::RiverSurfer => {
            SourceSim::Surfer(Box::new(SurferSim::new(layout.seed, layout.signals.lines)))
        }
        SourceGame::Galaga => {
            SourceSim::Galaga(Box::new(GalagaSim::new(layout.seed, layout.signals.lines)))
        }
        SourceGame::PacMan => {
            SourceSim::PacMan(Box::new(PacSim::new(layout.seed, layout.signals.lines)))
        }
        SourceGame::Columns => {
            SourceSim::Columns(Box::new(ColumnsSim::new(layout.seed, layout.signals.lines)))
        }
        SourceGame::Tetris => {
            SourceSim::Tetris(Box::new(TetrisSim::new(layout.seed, layout.signals.lines)))
        }
        SourceGame::Frogger => {
            SourceSim::Frogger(Box::new(FroggerSim::new(layout.seed, layout.signals.lines)))
        }
        SourceGame::Qbert => {
            SourceSim::Qbert(Box::new(QbertSim::new(layout.seed, layout.signals.lines)))
        }
        SourceGame::Bomberman => {
            SourceSim::Bomberman(Box::new(BomberSim::new(layout.seed, layout.signals.lines)))
        }
        SourceGame::Plinko => {
            SourceSim::Plinko(Box::new(PlinkoSim::new(layout.seed, layout.signals.lines)))
        }
    };
    ActiveRun {
        sim,
        arena,
        environment: RunEnvironment::Source {
            path: path.to_path_buf(),
            name: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("source")
                .to_string(),
            language,
            layout: Box::new(layout),
            sim: sim_state,
            focused_block: None,
        },
        crash_label: None,
        entering_label: None,
    }
}

/// Level length for an off-grid run, in metres: longer file, longer level.
fn level_metres(layout: &DiscLayout) -> f32 {
    layout.signals.lines as f32 * config::PLATFORMER_METRES_PER_LINE
}

/// Middle of a ring, in world units.
fn ring_center_world(layout: &DiscLayout) -> (f32, f32) {
    (
        layout.center.0 as f32 * config::GRID_SPACING,
        layout.center.1 as f32 * config::GRID_SPACING,
    )
}

/// Inner radius of a ring wall, in world units.
fn ring_radius_world(layout: &DiscLayout) -> f32 {
    layout.radius as f32 * config::GRID_SPACING
}

/// Driveable cells of a ring, in a stable order, for scattering power-ups.
fn ring_food_cells(arena: &Arena) -> Vec<(i32, i32)> {
    arena.roads.iter().copied().collect()
}

/// True when `cell` is a ring's close gate.
fn is_ring_gate(arena: &Arena, cell: (i32, i32)) -> bool {
    arena
        .parent_portal
        .as_ref()
        .is_some_and(|portal| portal.contains(cell))
}

fn spawn_sim(arena: &Arena, cells: &HashMap<(i32, i32), usize>) -> LightcycleSim {
    let blocked =
        |cell: (i32, i32)| cells.contains_key(&cell) || arena.street_walls.contains(&cell);

    if let Some((spawn, heading)) = arena.spawn_with_runway(
        blocked,
        config::LIGHTCYCLE_SPAWN_SEARCH_RADIUS,
        config::LIGHTCYCLE_SPAWN_RUNWAY_CELLS,
    ) {
        return LightcycleSim::start(spawn, heading);
    }

    // Nowhere to ride at all: hold the run until a restart or another folder
    // replaces the map.
    let cell = arena
        .nearest_empty_cell(blocked, config::LIGHTCYCLE_SPAWN_SEARCH_RADIUS)
        .unwrap_or_else(|| arena.center());
    LightcycleSim::ready(cell, Heading::PosX)
}

/// Starts the flight between the two modes when `M` is pressed.
///
/// Nothing about the world changes here. The run for a lightcycle mode is built
/// but parked, because the flight has to know which road it is diving into and
/// where its chase rig will end up before [`apply_mode_swap`] puts that arena on
/// screen at the top of the climb.
fn toggle_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mode: Res<InteractionMode>,
    mut transition: ResMut<ModeTransition>,
    mut state: ResMut<LightcycleState>,
    navigator: Res<NavigatorResource>,
    mut orbit: ResMut<OrbitCameraResource>,
    camera: Single<&Transform, With<Camera3d>>,
) {
    if !keys.just_pressed(KeyCode::KeyM) || transition.is_active() {
        return;
    }

    let from = **camera;
    match *mode {
        InteractionMode::Lightcycle => {
            // Park the orbit rig now rather than letting it lerp home after the
            // swap, so the flight can aim at the exact pose the explorer camera
            // will hold when it takes over.
            orbit.reset_target();
            orbit.target = Vec3::ZERO;
            let to =
                Transform::from_translation(orbit.position()).looking_at(orbit.target, Vec3::Y);
            // Rise out of the street the cycle is on, so the overhead shot
            // arrives holding the heading the run was riding.
            let road = state
                .run
                .as_ref()
                .map_or(Vec3::X, |run| pose_forward(&cycle_cell_pose(&run.sim)));
            let apex = gods_eye_pose(orbit.target, road);
            transition.start(InteractionMode::Explorer, from, apex, to, orbit.target);
        }
        InteractionMode::Explorer => {
            let run = build_active_run(&navigator.0.current_path, navigator.0.entries.clone());
            let (to, focus, road) = chase_landing_pose(&run);
            let apex = gods_eye_pose(focus, road);
            state.pending_run = Some(run);
            transition.start(InteractionMode::Lightcycle, from, apex, to, focus);
        }
    }
}

/// Where the chase camera will sit once a run spawns, the point it looks at,
/// and the road the cycle will ride away down.
fn chase_landing_pose(run: &ActiveRun) -> (Transform, Vec3, Vec3) {
    let pose = cycle_cell_pose(&run.sim);
    let cycle = pose_world_position(&pose);
    let (offset, view_forward) = chase_camera_rig(pose_forward(&pose), Vec2::ZERO);
    let focus = cycle + view_forward * config::LIGHTCYCLE_CAMERA_LOOKAHEAD;
    (
        Transform::from_translation(cycle + offset).looking_at(focus, Vec3::Y),
        focus,
        view_forward,
    )
}

/// Tears down the old world and builds the new one, at the top of the flight's
/// climb, where the camera is highest and the flash covers the frame.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn apply_mode_swap(
    mut transition: ResMut<ModeTransition>,
    mut mode: ResMut<InteractionMode>,
    mut state: ResMut<LightcycleState>,
    assets: Res<LightcycleAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
    old_lightcycle_entities: Query<Entity, Or<(With<LightcycleSceneRoot>, With<TrailSceneRoot>)>>,
) {
    let Some(target) = transition.pending_swap() else {
        return;
    };

    despawn_lightcycle_entities(&mut commands, &old_lightcycle_entities);
    state.clock = 0.0;
    state.run = None;
    state.crash_fx = None;
    state.entry_fx = None;
    state.restore_directory = false;

    if target == InteractionMode::Lightcycle
        && let Some(run) = state.pending_run.take()
    {
        spawn_run_entities(&mut commands, &assets, &mut meshes, &run);
        state.run = Some(run);
    }

    state.pending_run = None;
    *mode = target;
    transition.mark_swapped();
}

#[allow(clippy::type_complexity)]
fn despawn_lightcycle_entities(
    commands: &mut Commands,
    old_lightcycle_entities: &Query<Entity, Or<(With<LightcycleSceneRoot>, With<TrailSceneRoot>)>>,
) {
    for entity in old_lightcycle_entities {
        commands.entity(entity).despawn();
    }
}

fn spawn_run_entities(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    run: &ActiveRun,
) {
    let pose = cycle_cell_pose(&run.sim);
    commands.spawn((
        LightcycleSceneRoot,
        CycleEntity,
        Transform::from_translation(pose_world_position(&pose)).with_rotation(pose_rotation(&pose)),
        Visibility::default(),
        ChaseCamera {
            forward: pose_forward(&pose),
            look: Vec2::ZERO,
        },
        Pickable::IGNORE,
        children![(
            WorldAssetRoot(assets.cycle_scene.clone()),
            Transform::from_rotation(Quat::from_rotation_y(config::LIGHTCYCLE_MODEL_YAW))
                .with_scale(Vec3::splat(config::LIGHTCYCLE_MODEL_SCALE)),
        )],
    ));

    spawn_city_floor(commands, assets, meshes, &run.arena);
    match &run.environment {
        RunEnvironment::Directory { .. } => {
            spawn_road_markings(commands, assets, meshes, &run.arena);
            spawn_city_structures(commands, assets, meshes, &run.arena);
            spawn_arena_walls(commands, assets, &run.arena);
            spawn_towers(commands, assets, meshes, run);
        }
        RunEnvironment::Document { layout, .. } => {
            spawn_document_page(commands, assets, meshes, &run.arena, layout);
            spawn_arena_walls(commands, assets, &run.arena);
            spawn_document_focus_marker(commands, assets, run);
        }
        RunEnvironment::Source {
            language,
            layout,
            sim,
            ..
        } => match sim {
            // The field shares the ring and its gate, but stands alone: no
            // hazards, pickups or opponent, and its own pooled rocks/beams.
            SourceSim::Asteroids(field) => {
                spawn_ring_shell(commands, meshes, assets, &run.arena, layout, *language);
                spawn_asteroid_field(commands, assets, field);
            }
            // Snake is the ordinary grid run plus power-ups and a locked gate.
            SourceSim::Snake(snake) => {
                spawn_ring_shell(commands, meshes, assets, &run.arena, layout, *language);
                spawn_snake_field(commands, assets, &run.arena, snake);
            }
            SourceSim::Platformer(level) => {
                spawn_platformer_level(commands, assets, meshes, *language, level);
            }
            SourceSim::Breaker(level) => {
                spawn_breaker_court(commands, assets, level);
            }
            SourceSim::Stealth(room) => {
                spawn_stealth_room(commands, assets, meshes, room);
            }
            SourceSim::Surfer(surfer) => {
                spawn_surfer_course(commands, assets, meshes, surfer);
            }
            SourceSim::Galaga(sim) => {
                spawn_galaga_field(commands, assets, sim);
            }
            SourceSim::PacMan(sim) => {
                spawn_pac_maze(commands, assets, meshes, sim);
            }
            SourceSim::Columns(sim) => {
                spawn_gem_well(commands, assets, sim);
            }
            SourceSim::Tetris(sim) => {
                spawn_tetris_board(commands, assets, sim);
            }
            SourceSim::Frogger(sim) => {
                spawn_frogger_highway(commands, assets, sim);
            }
            SourceSim::Qbert(sim) => {
                spawn_qbert_pyramid(commands, assets, sim);
            }
            SourceSim::Bomberman(sim) => {
                spawn_bomber_room(commands, assets, meshes, sim);
            }
            SourceSim::Plinko(sim) => {
                spawn_plinko_board(commands, assets, meshes, sim);
            }
            SourceSim::DiscWars(disc) => {
                spawn_disc_arena(
                    commands, assets, meshes, &run.arena, layout, *language, disc,
                );
                spawn_disc_focus_marker(commands, assets, run);
            }
        },
    }
    spawn_trail_ribbon(commands, assets, meshes, run);
}

fn spawn_trail_ribbon(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    run: &ActiveRun,
) {
    commands.spawn((
        TrailSceneRoot,
        Mesh3d(meshes.add(build_trail_mesh(&run.sim))),
        MeshMaterial3d(assets.trail_material.clone()),
        Pickable::IGNORE,
    ));
}

fn spawn_city_floor(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    arena: &Arena,
) {
    let spacing = config::GRID_SPACING;
    let span_x = (arena.max.0 - arena.min.0 + 1) as f32 * spacing;
    let span_z = (arena.max.1 - arena.min.1 + 1) as f32 * spacing;
    let center = Vec3::new(
        (arena.min.0 + arena.max.0) as f32 * spacing * 0.5,
        -0.08,
        (arena.min.1 + arena.max.1) as f32 * spacing * 0.5,
    );
    let mesh = Mesh::from(Cuboid::default()).transformed_by(
        Transform::from_translation(center).with_scale(Vec3::new(span_x, 0.12, span_z)),
    );
    let material = match arena.kind {
        ArenaKind::Document => assets.document_floor_material.clone(),
        ArenaKind::Disc => assets.disc_floor_material.clone(),
        ArenaKind::Directory => assets.city_floor_material.clone(),
    };
    commands.spawn((
        LightcycleSceneRoot,
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material),
        Pickable::IGNORE,
    ));
}

/// Builds a disc-wars ring: circular floor, ring wall, plinth, hazards, safe
/// pads, close gate, pickups, and the two fighters.
#[allow(clippy::too_many_arguments)]
/// Ring wall, corner plinth and close gate. Shared by every source arena, so
/// the asteroid field gets the same containment and the same way out.
fn spawn_ring_shell(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    assets: &LightcycleAssets,
    arena: &Arena,
    layout: &DiscLayout,
    language: SourceLanguage,
) {
    let accent = assets.disc_accent_materials[disc_language_index(language)].clone();
    let center = layout.center;
    let radius = layout.radius as f32;

    // Split the lethal fill into the ring band and the corner fill, so the
    // ring reads as a wall and the rest as ground the ring sits in.
    let mut ring_cells = Vec::new();
    let mut plinth_cells = Vec::new();
    for &cell in &arena.street_walls {
        let dx = (cell.0 - center.0) as f32;
        let dz = (cell.1 - center.1) as f32;
        if (dx * dx + dz * dz).sqrt() <= radius + 1.0 {
            ring_cells.push(cell);
        } else {
            plinth_cells.push(cell);
        }
    }
    spawn_disc_cube_layer(
        commands,
        meshes,
        &ring_cells,
        assets.disc_ring_material.clone(),
        1.7,
        1.7,
    );
    spawn_disc_cube_layer(
        commands,
        meshes,
        &plinth_cells,
        assets.disc_plinth_material.clone(),
        0.08,
        2.0,
    );

    spawn_disc_gate(commands, assets, arena, layout, &accent);
}

fn spawn_disc_arena(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    arena: &Arena,
    layout: &DiscLayout,
    language: SourceLanguage,
    disc: &DiscSim,
) {
    let center = layout.center;
    spawn_ring_shell(commands, meshes, assets, arena, layout, language);

    let hazards: Vec<_> = layout.hazards.iter().copied().collect();
    spawn_disc_cube_layer(
        commands,
        meshes,
        &hazards,
        assets.disc_hazard_material.clone(),
        0.06,
        1.5,
    );
    let pads: Vec<_> = layout.safe_pads.iter().copied().collect();
    spawn_disc_cube_layer(
        commands,
        meshes,
        &pads,
        assets.disc_safe_pad_material.clone(),
        0.05,
        1.3,
    );

    for (index, spot) in layout.pickups.iter().enumerate() {
        let taken = disc.taken.contains(&index);
        let phase = index as f32 / layout.pickups.len().max(1) as f32;
        commands.spawn((
            LightcycleSceneRoot,
            DiscPickupEntity { index, phase },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.disc_pickup_material.clone()),
            Transform::from_translation(
                config::ground_position(spot.cell.0, spot.cell.1) + Vec3::Y * 0.45,
            )
            .with_scale(Vec3::splat(0.3)),
            if taken {
                Visibility::Hidden
            } else {
                Visibility::Visible
            },
            Pickable::IGNORE,
        ));
    }

    let opponent_visible = if disc.opponent.alive {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    commands.spawn((
        LightcycleSceneRoot,
        OpponentEntity,
        Mesh3d(assets.recognizer_mesh.clone()),
        MeshMaterial3d(assets.disc_opponent_material.clone()),
        Transform::from_translation(
            config::ground_position(disc.opponent.cell.0, disc.opponent.cell.1)
                + Vec3::Y * (config::RECOGNIZER_HEIGHT * 0.5),
        ),
        opponent_visible,
        Pickable::IGNORE,
    ));

    let player_disc_visible = if disc.player_disc.is_some() {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    commands.spawn((
        LightcycleSceneRoot,
        PlayerDiscEntity,
        Mesh3d(assets.disc_mesh.clone()),
        MeshMaterial3d(assets.disc_player_disc_material.clone()),
        Transform::from_translation(config::ground_position(center.0, center.1)),
        player_disc_visible,
        Pickable::IGNORE,
    ));

    let opponent_disc_visible = if disc.opponent.disc.is_some() {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    commands.spawn((
        LightcycleSceneRoot,
        OpponentDiscEntity,
        Mesh3d(assets.disc_mesh.clone()),
        MeshMaterial3d(assets.disc_opponent_material.clone()),
        Transform::from_translation(config::ground_position(center.0, center.1)),
        opponent_disc_visible,
        Pickable::IGNORE,
    ));
}

/// Spawns the pooled rock and beam bodies for an asteroid field.
///
/// The sim drives visibility and transforms; the pool is fixed because a rock
/// only ever splits into a bounded number of children.
fn spawn_asteroid_field(commands: &mut Commands, assets: &LightcycleAssets, sim: &AsteroidsSim) {
    let rock_count = config::ASTEROIDS_MAX_ROCKS.max(sim.rocks.len());
    for index in 0..rock_count {
        let live = sim.rocks.get(index);
        let radius = live.map_or(1.0, |rock| rock.size.radius());
        commands.spawn((
            LightcycleSceneRoot,
            RockEntity { index },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.rock_material.clone()),
            Transform::from_xyz(0.0, radius, 0.0).with_scale(Vec3::splat(radius * 2.0)),
            if live.is_some() {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
            Pickable::IGNORE,
        ));
    }
    for index in 0..config::ASTEROIDS_MAX_BEAMS {
        let live = sim.beams.get(index);
        commands.spawn((
            LightcycleSceneRoot,
            BeamEntity { index },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.beam_material.clone()),
            Transform::from_xyz(0.0, 0.35, 0.0),
            if live.is_some() {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
            Pickable::IGNORE,
        ));
    }
}

/// Spawns the pooled bug and beam bodies for a Galaga field.
///
/// The formation is a fixed grid, so the bug pool never grows; the sim drives
/// transforms and visibility.
fn spawn_galaga_field(commands: &mut Commands, assets: &LightcycleAssets, sim: &GalagaSim) {
    for (index, bug) in sim.bugs.iter().enumerate() {
        commands.spawn((
            LightcycleSceneRoot,
            BugEntity { index },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.galaga_bug_material.clone()),
            Transform::from_xyz(bug.x, config::GALAGA_BUG_HEIGHT * 0.5, bug.z).with_scale(
                Vec3::new(
                    config::GALAGA_BUG_RADIUS * 2.0,
                    config::GALAGA_BUG_HEIGHT,
                    config::GALAGA_BUG_RADIUS * 2.0,
                ),
            ),
            if bug.alive {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
            Pickable::IGNORE,
        ));
    }
    for index in 0..config::GALAGA_MAX_BEAMS {
        let live = sim.beams.get(index);
        commands.spawn((
            LightcycleSceneRoot,
            GalagaBeamEntity { index },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.galaga_beam_material.clone()),
            Transform::from_xyz(0.0, 0.35, 0.0),
            if live.is_some() {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
            Pickable::IGNORE,
        ));
    }
}

/// Spawns the Pac-Man maze: static walls plus pooled dots and ghosts.
fn spawn_pac_maze(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    _meshes: &mut Assets<Mesh>,
    sim: &PacSim,
) {
    for row in 0..config::PAC_ROWS {
        for col in 0..config::PAC_COLS {
            if !PacSim::solid((col, row)) {
                continue;
            }
            let (x, z) = PacSim::center((col, row));
            commands.spawn((
                LightcycleSceneRoot,
                Mesh3d(assets.unit_cube.clone()),
                MeshMaterial3d(assets.stealth_wall_material.clone()),
                Transform::from_xyz(x, config::GRID_SPACING * 0.5, z)
                    .with_scale(Vec3::splat(config::GRID_SPACING)),
                Visibility::Visible,
                Pickable::IGNORE,
            ));
        }
    }
    for (index, ghost) in sim.ghosts.iter().enumerate() {
        commands.spawn((
            LightcycleSceneRoot,
            GhostEntity { index },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.galaga_bug_material.clone()),
            Transform::from_xyz(ghost.x, 0.9, ghost.z).with_scale(Vec3::splat(1.5)),
            Visibility::Visible,
            Pickable::IGNORE,
        ));
    }
    for &cell in &sim.dots {
        let (x, z) = PacSim::center(cell);
        commands.spawn((
            LightcycleSceneRoot,
            DotEntity { cell },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.snake_food_material.clone()),
            Transform::from_xyz(x, 0.35, z).with_scale(Vec3::splat(0.55)),
            Visibility::Visible,
            Pickable::IGNORE,
        ));
    }
}

/// Spawns the pooled gem cells of a Columns well.
fn spawn_gem_well(commands: &mut Commands, assets: &LightcycleAssets, sim: &ColumnsSim) {
    let rendered = sim.render_board();
    for (index, colour) in rendered.iter().copied().enumerate() {
        let col = index % config::COLUMNS_COLS;
        let row = index / config::COLUMNS_COLS;
        let x = (col as f32 - (config::COLUMNS_COLS - 1) as f32 * 0.5) * 1.6;
        // Row 0 is the top of the well, so higher rows sit lower on screen.
        // The whole well is lifted above the arena floor.
        let y = ((config::COLUMNS_ROWS - 1) - row) as f32 * 1.6 + 0.8;
        commands.spawn((
            LightcycleSceneRoot,
            GemEntity { index },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(
                assets.gem_materials[colour.unwrap_or(0) as usize % config::COLUMNS_GEM_COLORS]
                    .clone(),
            ),
            Transform::from_xyz(x, y, 0.0).with_scale(Vec3::splat(1.5)),
            if colour.is_some() {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
            Pickable::IGNORE,
        ));
    }

    // A visible frame marks the playfield: side walls plus a floor bar.
    let board_half = config::COLUMNS_COLS as f32 * 0.8;
    let wall_x = board_half + 0.55;
    let board_height = config::COLUMNS_ROWS as f32 * 1.6;
    let wall_scale = Vec3::new(0.4, board_height + 0.6, 0.5);
    for x in [-wall_x, wall_x] {
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.stealth_wall_material.clone()),
            Transform::from_xyz(x, board_height * 0.5 + 0.3, 0.0).with_scale(wall_scale),
            Visibility::Visible,
            Pickable::IGNORE,
        ));
    }
    commands.spawn((
        LightcycleSceneRoot,
        Mesh3d(assets.unit_cube.clone()),
        MeshMaterial3d(assets.stealth_wall_material.clone()),
        Transform::from_xyz(0.0, 0.2, 0.0).with_scale(Vec3::new(wall_x * 2.0 + 0.8, 0.4, 0.5)),
        Visibility::Visible,
        Pickable::IGNORE,
    ));
}

/// Spawns the pooled block cells of a Tetris board.
fn spawn_tetris_board(commands: &mut Commands, assets: &LightcycleAssets, sim: &TetrisSim) {
    let rendered = sim.render_board();
    for (index, colour) in rendered.iter().copied().enumerate() {
        let col = index % config::TETRIS_COLS;
        let row = index / config::TETRIS_COLS;
        let x = (col as f32 - (config::TETRIS_COLS - 1) as f32 * 0.5) * 1.2;
        // Row 0 is the top of the board, so higher rows sit lower on screen.
        // The whole board is lifted above the arena floor.
        let y = ((config::TETRIS_ROWS - 1) - row) as f32 * 1.2 + 0.6;
        commands.spawn((
            LightcycleSceneRoot,
            BlockEntity { index },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.tetris_materials[colour.unwrap_or(0) as usize % 7].clone()),
            Transform::from_xyz(x, y, 0.0).with_scale(Vec3::splat(1.15)),
            if colour.is_some() {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
            Pickable::IGNORE,
        ));
    }

    // A visible frame marks the playfield: side walls plus a floor bar.
    let board_half = config::TETRIS_COLS as f32 * 0.6;
    let wall_x = board_half + 0.55;
    let board_height = config::TETRIS_ROWS as f32 * 1.2;
    let wall_scale = Vec3::new(0.4, board_height + 0.6, 0.5);
    for x in [-wall_x, wall_x] {
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.stealth_wall_material.clone()),
            Transform::from_xyz(x, board_height * 0.5 + 0.3, 0.0).with_scale(wall_scale),
            Visibility::Visible,
            Pickable::IGNORE,
        ));
    }
    commands.spawn((
        LightcycleSceneRoot,
        Mesh3d(assets.unit_cube.clone()),
        MeshMaterial3d(assets.stealth_wall_material.clone()),
        Transform::from_xyz(0.0, 0.15, 0.0).with_scale(Vec3::new(wall_x * 2.0 + 0.8, 0.3, 0.5)),
        Visibility::Visible,
        Pickable::IGNORE,
    ));
}

/// Spawns the pooled obstacle cubes of a Frogger highway.
fn spawn_frogger_highway(commands: &mut Commands, assets: &LightcycleAssets, sim: &FroggerSim) {
    let cells = sim.obstacle_cells();
    for (index, &cell) in cells.iter().enumerate() {
        let (x, z) = FroggerSim::center(cell);
        commands.spawn((
            LightcycleSceneRoot,
            FrogObstacleEntity { index },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.galaga_bug_material.clone()),
            Transform::from_xyz(x, 0.6, z).with_scale(Vec3::splat(1.9)),
            Visibility::Visible,
            Pickable::IGNORE,
        ));
    }
}

/// Spawns the Q*bert pyramid cubes and its pooled enemies.
fn spawn_qbert_pyramid(commands: &mut Commands, assets: &LightcycleAssets, sim: &QbertSim) {
    for row in 0..config::QBERT_ROWS {
        for index in 0..=row {
            let (x, z) = QbertSim::cube_position(row, index);
            let lit = sim.lit[QbertSim::cube_index(row, index)];
            let y =
                (config::QBERT_ROWS as f32 - 1.0 - row as f32) * config::QBERT_CUBE_HEIGHT * 0.5;
            commands.spawn((
                LightcycleSceneRoot,
                QbertCubeEntity { row, index },
                Mesh3d(assets.unit_cube.clone()),
                MeshMaterial3d(if lit {
                    assets.qbert_cube_lit.clone()
                } else {
                    assets.qbert_cube_dim.clone()
                }),
                Transform::from_xyz(x, y, z).with_scale(Vec3::new(
                    config::QBERT_CUBE_SPACING * 0.9,
                    config::QBERT_CUBE_HEIGHT,
                    config::QBERT_CUBE_SPACING * 0.9,
                )),
                Visibility::Visible,
                Pickable::IGNORE,
            ));
        }
    }
    for (index, enemy) in sim.enemies.iter().enumerate() {
        let (x, z) = QbertSim::cube_position(enemy.row, enemy.index);
        let y =
            (config::QBERT_ROWS as f32 - 1.0 - enemy.row as f32) * config::QBERT_CUBE_HEIGHT * 0.5
                + config::QBERT_CUBE_HEIGHT * 0.8;
        commands.spawn((
            LightcycleSceneRoot,
            QbertEnemyEntity { index },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.qbert_enemy_material.clone()),
            Transform::from_xyz(x, y, z).with_scale(Vec3::splat(1.4)),
            Visibility::Visible,
            Pickable::IGNORE,
        ));
    }
}

/// Spawns the Bomberman room: crates, bombs, walls and the exit marker.
fn spawn_bomber_room(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    _meshes: &mut Assets<Mesh>,
    sim: &BomberSim,
) {
    for row in 0..config::BOMBER_ROWS {
        for col in 0..config::BOMBER_COLS {
            let cell = (col, row);
            let (x, z) = BomberSim::center(cell);
            let is_border = col == 0
                || col == config::BOMBER_COLS - 1
                || row == 0
                || row == config::BOMBER_ROWS - 1;
            if is_border {
                commands.spawn((
                    LightcycleSceneRoot,
                    Mesh3d(assets.unit_cube.clone()),
                    MeshMaterial3d(assets.stealth_wall_material.clone()),
                    Transform::from_xyz(x, 1.0, z).with_scale(Vec3::splat(2.0)),
                    Visibility::Visible,
                    Pickable::IGNORE,
                ));
            } else if sim.crates.contains(&cell) {
                commands.spawn((
                    LightcycleSceneRoot,
                    BomberCrateEntity { cell },
                    Mesh3d(assets.unit_cube.clone()),
                    MeshMaterial3d(assets.bomber_crate_material.clone()),
                    Transform::from_xyz(x, 0.8, z).with_scale(Vec3::splat(1.8)),
                    Visibility::Visible,
                    Pickable::IGNORE,
                ));
            }
        }
    }
    let (ex, ez) = BomberSim::center(sim.exit);
    commands.spawn((
        LightcycleSceneRoot,
        Mesh3d(assets.unit_cube.clone()),
        MeshMaterial3d(assets.stealth_exit_material.clone()),
        Transform::from_xyz(ex, 0.5, ez).with_scale(Vec3::new(1.9, 0.4, 1.9)),
        Visibility::Visible,
        Pickable::IGNORE,
    ));
    for index in 0..config::BOMBER_MAX_BOMBS {
        commands.spawn((
            LightcycleSceneRoot,
            BomberBombEntity { index },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.bomber_bomb_material.clone()),
            Transform::from_xyz(0.0, 0.6, 0.0),
            Visibility::Hidden,
            Pickable::IGNORE,
        ));
    }
}

/// Spawns the Plinko board: static pins and buckets plus pooled balls.
fn spawn_plinko_board(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    _meshes: &mut Assets<Mesh>,
    sim: &PlinkoSim,
) {
    for pin in &sim.pins {
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.plinko_pin_material.clone()),
            Transform::from_xyz(pin.x, pin.y, 0.0).with_scale(Vec3::splat(0.42)),
            Visibility::Visible,
            Pickable::IGNORE,
        ));
    }
    for slot in 0..8 {
        let x = (slot as f32 - 3.5) * 2.0;
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.disc_accent_materials[slot].clone()),
            Transform::from_xyz(x, -config::PLINKO_HEIGHT * 0.5 - 1.0, 0.0)
                .with_scale(Vec3::new(1.7, 0.4, 0.4)),
            Visibility::Visible,
            Pickable::IGNORE,
        ));
    }
    for index in 0..config::PLINKO_BALLS {
        commands.spawn((
            LightcycleSceneRoot,
            PlinkoBallEntity { index },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.plinko_ball_material.clone()),
            Transform::from_xyz(0.0, config::PLINKO_HEIGHT * 0.5, 0.0),
            Visibility::Hidden,
            Pickable::IGNORE,
        ));
    }
}

/// Spawns a snake ring's power-ups and the bar sealing its exit.
///
/// The power-ups are a fixed pool keyed by index; the sim marks them eaten.
fn spawn_snake_field(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    arena: &Arena,
    snake: &SnakeSim,
) {
    for (index, food) in snake.food.iter().enumerate() {
        commands.spawn((
            LightcycleSceneRoot,
            SnakeFoodEntity { index },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.snake_food_material.clone()),
            Transform::from_translation(
                config::ground_position(food.cell.0, food.cell.1) + Vec3::Y * 0.5,
            )
            .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4))
            .with_scale(Vec3::splat(config::SNAKE_FOOD_SIZE)),
            if food.eaten {
                Visibility::Hidden
            } else {
                Visibility::Visible
            },
            Pickable::IGNORE,
        ));
    }

    // A solid bar in the gate opening until the exit opens. The physical lock is
    // the classification; this is what the rider sees.
    let Some(portal) = arena.parent_portal.as_ref() else {
        return;
    };
    let gate = portal.to;
    commands.spawn((
        LightcycleSceneRoot,
        SnakeGateLock,
        Mesh3d(assets.unit_cube.clone()),
        MeshMaterial3d(assets.snake_lock_material.clone()),
        Transform::from_translation(config::ground_position(gate.0, gate.1) + Vec3::Y * 0.9)
            .with_scale(Vec3::new(
                config::GRID_SPACING,
                config::LIGHTCYCLE_WALL_HEIGHT * 0.9,
                config::GRID_SPACING,
            )),
        if snake.exit_open {
            Visibility::Hidden
        } else {
            Visibility::Visible
        },
        Pickable::IGNORE,
    ));
}

/// Spawns a platformer level: a backdrop, the platforms with neon lips, the
/// exit door and the runner.
fn spawn_platformer_level(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    language: SourceLanguage,
    level: &PlatformerSim,
) {
    let _ = meshes;
    let depth = config::PLATFORMER_DEPTH;
    let lip = assets.disc_accent_materials[disc_language_index(language)].clone();

    // A dark slab behind the level so the platforms read against something.
    let span = level.length + 60.0;
    commands.spawn((
        LightcycleSceneRoot,
        Mesh3d(assets.unit_cube.clone()),
        MeshMaterial3d(assets.platform_material.clone()),
        Transform::from_translation(Vec3::new(level.length * 0.5, 8.0, -depth))
            .with_scale(Vec3::new(span, 46.0, 1.0)),
        Pickable::IGNORE,
    ));

    for platform in &level.platforms {
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.platform_material.clone()),
            Transform::from_translation(Vec3::new(
                platform.x + platform.w * 0.5,
                platform.y - platform.h * 0.5,
                0.0,
            ))
            .with_scale(Vec3::new(platform.w, platform.h, depth)),
            Pickable::IGNORE,
        ));
        // The lip marks the surface the runner lands on.
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(lip.clone()),
            Transform::from_translation(Vec3::new(platform.x + platform.w * 0.5, platform.y, 0.0))
                .with_scale(Vec3::new(platform.w, 0.14, depth + 0.2)),
            Pickable::IGNORE,
        ));
    }

    let door = level.exit_box();
    commands.spawn((
        LightcycleSceneRoot,
        Mesh3d(assets.unit_cube.clone()),
        MeshMaterial3d(assets.exit_material.clone()),
        Transform::from_translation(Vec3::new(door.x + door.w * 0.5, door.y - door.h * 0.5, 0.0))
            .with_scale(Vec3::new(door.w, door.h, depth * 0.4)),
        Pickable::IGNORE,
    ));

    commands.spawn((
        LightcycleSceneRoot,
        CharacterEntity,
        CharacterAnim::at(Vec3::new(level.runner.x, level.runner.y, 0.0)),
        Transform::from_translation(Vec3::new(level.runner.x, level.runner.y, 0.0)),
        Visibility::default(),
        Pickable::IGNORE,
        children![(
            WorldAssetRoot(assets.tron_scene.clone()),
            Transform::from_rotation(Quat::from_rotation_y(config::TRON_MODEL_YAW))
                .with_scale(Vec3::splat(config::TRON_MODEL_SCALE)),
        )],
    ));
}

/// Spawns a breaker court: the side and ceiling walls, the bricks, and the ball.
///
/// The bike itself is the paddle, so it is left to `update_cycle_transform`.
fn spawn_breaker_court(commands: &mut Commands, assets: &LightcycleAssets, level: &BreakerSim) {
    let (width, height) = level.court;
    let thickness = 0.8;
    let depth = config::PLATFORMER_DEPTH;
    // Side walls and ceiling.
    for (x, y, w, h) in [
        (
            -width * 0.5 - thickness * 0.5,
            height * 0.5,
            thickness,
            height,
        ),
        (
            width * 0.5 + thickness * 0.5,
            height * 0.5,
            thickness,
            height,
        ),
        (
            0.0,
            height + thickness * 0.5,
            width + thickness * 2.0,
            thickness,
        ),
    ] {
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.court_material.clone()),
            Transform::from_translation(Vec3::new(x, y, -depth * 0.5))
                .with_scale(Vec3::new(w, h, depth)),
            Pickable::IGNORE,
        ));
    }

    for (index, brick) in level.bricks.iter().enumerate() {
        let (bx, by, bw, bh) = level.brick_box(brick);
        commands.spawn((
            LightcycleSceneRoot,
            BrickEntity { index },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.brick_material.clone()),
            Transform::from_translation(Vec3::new(bx + bw * 0.5, by + bh * 0.5, 0.0))
                .with_scale(Vec3::new(bw, bh, depth * 0.6)),
            if brick.alive {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
            Pickable::IGNORE,
        ));
    }

    commands.spawn((
        LightcycleSceneRoot,
        BallEntity,
        Mesh3d(assets.unit_cube.clone()),
        MeshMaterial3d(assets.ball_material.clone()),
        Transform::from_translation(Vec3::new(level.ball.x, level.ball.y, 0.0))
            .with_scale(Vec3::splat(config::BREAKER_BALL_RADIUS * 2.0)),
        Pickable::IGNORE,
    ));
}

/// Spawns a stealth room: floor, cover, the door, the character and the patrols
/// with their vision cones.
fn spawn_stealth_room(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    room: &StealthSim,
) {
    let _ = meshes;
    let (half_w, half_h) = (
        config::STEALTH_WIDTH as f32 * 0.5,
        config::STEALTH_HEIGHT as f32 * 0.5,
    );
    let span = config::GRID_SPACING;
    let width = half_w * 2.0 * span;
    let height = half_h * 2.0 * span;

    commands.spawn((
        LightcycleSceneRoot,
        Mesh3d(assets.unit_cube.clone()),
        MeshMaterial3d(assets.stealth_floor_material.clone()),
        Transform::from_translation(Vec3::new(0.0, -0.1, 0.0)).with_scale(Vec3::new(
            width + span * 2.0,
            0.2,
            height + span * 2.0,
        )),
        Pickable::IGNORE,
    ));

    for cell in &room.cover {
        let position = config::ground_position(cell.0, cell.1);
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.stealth_wall_material.clone()),
            Transform::from_translation(position + Vec3::Y * config::STEALTH_WALL_HEIGHT * 0.5)
                .with_scale(Vec3::new(
                    span * 0.96,
                    config::STEALTH_WALL_HEIGHT,
                    span * 0.96,
                )),
            Pickable::IGNORE,
        ));
    }

    let exit = config::ground_position(room.exit.0, room.exit.1);
    commands.spawn((
        LightcycleSceneRoot,
        Mesh3d(assets.unit_cube.clone()),
        MeshMaterial3d(assets.stealth_exit_material.clone()),
        Transform::from_translation(exit + Vec3::Y * 1.4).with_scale(Vec3::new(
            span * 0.9,
            2.8,
            span * 0.9,
        )),
        Pickable::IGNORE,
    ));

    let facing = |angle: f32| Quat::from_rotation_y(std::f32::consts::FRAC_PI_2 - angle);
    let body = |assets: &LightcycleAssets, scale: f32| {
        children![(
            WorldAssetRoot(assets.tron_scene.clone()),
            Transform::from_rotation(Quat::from_rotation_y(config::TRON_MODEL_YAW))
                .with_scale(Vec3::splat(scale)),
        )]
    };

    let start = config::ground_position(room.character.0, room.character.1);
    commands.spawn((
        LightcycleSceneRoot,
        CharacterEntity,
        CharacterAnim::at(start),
        Transform::from_translation(start).with_rotation(facing(room.heading.angle())),
        Visibility::default(),
        Pickable::IGNORE,
        body(assets, config::STEALTH_CHARACTER_SCALE),
    ));

    for (index, guard) in room.guards.iter().enumerate() {
        let cell = guard.cell();
        let position = config::ground_position(cell.0, cell.1);
        commands.spawn((
            LightcycleSceneRoot,
            GuardEntity { index },
            Transform::from_translation(position)
                .with_rotation(facing(guard.patrol.heading().angle())),
            Visibility::default(),
            Pickable::IGNORE,
            body(
                assets,
                config::STEALTH_CHARACTER_SCALE * config::STEALTH_GUARD_SCALE,
            ),
        ));
        commands.spawn((
            LightcycleSceneRoot,
            GuardConeEntity { index, mesh: None },
            Mesh3d(assets.vision_cone.clone()),
            MeshMaterial3d(assets.stealth_cone_material.clone()),
            Transform::from_translation(position + Vec3::Y * 0.08)
                .with_rotation(facing(guard.vision_angle()))
                .with_scale(Vec3::splat(config::STEALTH_CONE_REACH)),
            Pickable::IGNORE,
        ));
    }
}

/// Spawns a river surfer course: the water ribbon that follows the sim's
/// centreline, the rocks, the boost gates and the finish gate. The bike itself
/// is the shared cycle, posed from the sim every frame.
fn spawn_surfer_course(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    surfer: &SurferSim,
) {
    commands.spawn((
        LightcycleSceneRoot,
        Mesh3d(meshes.add(surfer_river_mesh(surfer))),
        MeshMaterial3d(assets.surfer_water_material.clone()),
        Pickable::IGNORE,
    ));

    for rock in &surfer.rocks {
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.surfer_rock_material.clone()),
            Transform::from_translation(Vec3::new(
                rock.x,
                config::SURFER_ROCK_HEIGHT * 0.5,
                rock.z,
            ))
            .with_scale(Vec3::new(
                rock.radius * 2.0,
                config::SURFER_ROCK_HEIGHT,
                rock.radius * 2.0,
            )),
            Pickable::IGNORE,
        ));
    }

    for gate in &surfer.gates {
        spawn_surfer_gate(
            commands,
            assets,
            gate.x,
            gate.z,
            config::SURFER_GATE_SPAN,
            config::SURFER_GATE_HEIGHT,
            &assets.surfer_gate_material,
        );
    }

    let finish_x = surfer.centerline(surfer.length);
    spawn_surfer_gate(
        commands,
        assets,
        finish_x,
        surfer.length,
        surfer.width * 0.9,
        config::SURFER_FINISH_HEIGHT,
        &assets.surfer_finish_material,
    );
}

/// Two posts and a lintel framing a gate opening across the river.
#[allow(clippy::too_many_arguments)]
fn spawn_surfer_gate(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    x: f32,
    z: f32,
    span: f32,
    height: f32,
    material: &Handle<StandardMaterial>,
) {
    let post = Vec3::new(0.18, height, 0.18);
    for side in [-1.0_f32, 1.0] {
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(Vec3::new(x + side * span, height * 0.5, z))
                .with_scale(post),
            Pickable::IGNORE,
        ));
    }
    commands.spawn((
        LightcycleSceneRoot,
        Mesh3d(assets.unit_cube.clone()),
        MeshMaterial3d(material.clone()),
        Transform::from_translation(Vec3::new(x, height, z)).with_scale(Vec3::new(
            span * 2.0 + 0.36,
            0.18,
            0.18,
        )),
        Pickable::IGNORE,
    ));
}

/// The water ribbon, tessellated along the sim's centreline so the visual
/// banks match the gameplay banks exactly.
fn surfer_river_mesh(surfer: &SurferSim) -> Mesh {
    let start = -config::SURFER_RIVER_MARGIN;
    let end = surfer.length + config::SURFER_RIVER_MARGIN;
    let steps = ((end - start) / config::SURFER_RIVER_SAMPLE).ceil() as usize;
    let mut positions = Vec::with_capacity((steps + 1) * 2);
    let mut normals = Vec::with_capacity((steps + 1) * 2);
    for step in 0..=steps {
        let z = start + (end - start) * step as f32 / steps as f32;
        let center = surfer.centerline(z);
        positions.push([center - surfer.width, 0.0, z]);
        positions.push([center + surfer.width, 0.0, z]);
        normals.push([0.0, 1.0, 0.0]);
        normals.push([0.0, 1.0, 0.0]);
    }
    let mut indices = Vec::with_capacity(steps * 6);
    for step in 0..steps as u32 {
        let a = step * 2;
        let b = step * 2 + 1;
        let c = (step + 1) * 2;
        let d = (step + 1) * 2 + 1;
        indices.extend_from_slice(&[a, c, b, b, c, d]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}

/// A floor fan: a centre point, then one rim point per ray, each reaching as far
/// as that ray can see. Radii are in the mesh's own units.
fn cone_positions(half_angle: f32, radii: &[f32]) -> Vec<[f32; 3]> {
    let segments = radii.len().saturating_sub(1).max(1);
    let mut positions = Vec::with_capacity(radii.len() + 1);
    positions.push([0.0, 0.0, 0.0]);
    for (step, reach) in radii.iter().enumerate() {
        let t = step as f32 / segments as f32;
        let angle = -half_angle + t * half_angle * 2.0;
        positions.push([angle.sin() * reach, 0.0, angle.cos() * reach]);
    }
    positions
}

/// Moves an existing cone's rim out to `radii`, leaving its topology alone.
fn refit_cone(mesh: &mut Mesh, half_angle: f32, radii: &[f32]) {
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, cone_positions(half_angle, radii));
}

/// A cone whose rays reach `radii`: the shape of what a guard can actually see.
fn cone_mesh(half_angle: f32, radii: &[f32]) -> Mesh {
    let positions = cone_positions(half_angle, radii);
    let segments = radii.len().saturating_sub(1).max(1);
    let mut indices = Vec::new();
    for step in 0..segments as u32 {
        indices.extend_from_slice(&[0, step + 1, step + 2]);
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}

/// Unit-length flat cone with the stealth half-angle, opening along local `+Z`.
///
/// Used as a guard's cone until its real shape has been measured, so the first
/// frame is not a hole in the floor.
fn vision_cone_mesh(half_angle: f32, segments: usize) -> Mesh {
    let radii = vec![1.0; segments + 1];
    cone_mesh(half_angle, &radii)
}

/// Cuts each guard's cone to what it can actually see, rebuilding the mesh on the
/// first frame and refitting it after that.
///
/// Detection samples line of sight, so a cone that ignores cover tells the player
/// a lie about where they are safe.
fn fit_guard_cones(
    state: Res<LightcycleState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut cones: Query<(&mut GuardConeEntity, &mut Transform, &mut Mesh3d)>,
) {
    let Some(room) = state.run.as_ref().and_then(|run| run.source_stealth()) else {
        return;
    };
    for (mut cone, mut transform, mut mesh) in &mut cones {
        let radii = room.vision_radii(cone.index, config::STEALTH_CONE_SEGMENTS);
        match cone.mesh.clone() {
            Some(handle) => {
                if let Some(mut geometry) = meshes.get_mut(&handle) {
                    refit_cone(&mut geometry, config::STEALTH_VISION_HALF_ANGLE, &radii);
                }
            }
            None => {
                // The radii are in cells, so the scale drops to one cell from the
                // fixed reach the placeholder fan was drawn at.
                let handle = meshes.add(cone_mesh(config::STEALTH_VISION_HALF_ANGLE, &radii));
                mesh.0 = handle.clone();
                transform.scale = Vec3::splat(config::GRID_SPACING);
                cone.mesh = Some(handle);
            }
        }
    }
}

/// Merges same-sized cuboids at `cells` and spawns them as one batched entity.
fn spawn_disc_cube_layer(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    cells: &[(i32, i32)],
    material: Handle<StandardMaterial>,
    height: f32,
    footprint: f32,
) {
    for chunk in cells.chunks(config::MESH_CHUNK_SIZE) {
        let mut chunk = chunk.iter();
        let Some(&first) = chunk.next() else {
            continue;
        };
        let mut mesh = disc_cube(first, height, footprint);
        for &cell in chunk {
            mesh.merge(&disc_cube(cell, height, footprint))
                .expect("disc cuboid meshes must be merge-compatible");
        }
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Pickable::IGNORE,
        ));
    }
}

fn disc_cube(cell: (i32, i32), height: f32, footprint: f32) -> Mesh {
    Mesh::from(Cuboid::default()).transformed_by(
        Transform::from_translation(
            config::ground_position(cell.0, cell.1) + Vec3::Y * (height * 0.5),
        )
        .with_scale(Vec3::new(footprint, height, footprint)),
    )
}

/// Posts and lintel framing the corridor mouth, so the close gate reads as a
/// door in the ring wall.
fn spawn_disc_gate(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    arena: &Arena,
    layout: &DiscLayout,
    accent: &Handle<StandardMaterial>,
) {
    let Some(portal) = arena.parent_portal else {
        return;
    };
    let center = layout.center;
    let span_of = |cell: (i32, i32)| (cell.0 - center.0).abs() + (cell.1 - center.1).abs();
    let outer = if span_of(portal.from) > span_of(portal.to) {
        portal.from
    } else {
        portal.to
    };
    let base = config::ground_position(outer.0, outer.1);
    // The corridor runs along the axis from the center to the outer cell, so
    // the door opening is perpendicular to it.
    let opening_axis = if (outer.0 - center.0) == 0 {
        Vec3::X
    } else {
        Vec3::Z
    };
    let post = Vec3::new(0.24, 2.2, 0.24);
    for side in [-1.0_f32, 1.0] {
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(accent.clone()),
            Transform::from_translation(base + opening_axis * (side * 1.05) + Vec3::Y * 1.1)
                .with_scale(post),
            Pickable::IGNORE,
        ));
    }
    let lintel_scale = if opening_axis == Vec3::X {
        Vec3::new(2.5, 0.22, 0.24)
    } else {
        Vec3::new(0.24, 0.22, 2.5)
    };
    commands.spawn((
        LightcycleSceneRoot,
        Mesh3d(assets.unit_cube.clone()),
        MeshMaterial3d(accent.clone()),
        Transform::from_translation(base + Vec3::Y * 2.25).with_scale(lintel_scale),
        Pickable::IGNORE,
    ));
}

fn spawn_disc_focus_marker(commands: &mut Commands, assets: &LightcycleAssets, run: &ActiveRun) {
    let RunEnvironment::Source { language, .. } = &run.environment else {
        return;
    };
    let pose = cycle_cell_pose(&run.sim);
    commands.spawn((
        LightcycleSceneRoot,
        DocumentFocusMarker,
        Mesh3d(assets.unit_cube.clone()),
        MeshMaterial3d(assets.disc_accent_materials[disc_language_index(*language)].clone()),
        Transform::from_translation(pose_world_position(&pose) + Vec3::Y * 0.08)
            .with_scale(Vec3::new(1.4, 0.08, 1.4)),
        Pickable::IGNORE,
    ));
}

/// Keeps the disc, opponent, opponent disc, and pickups glued to the sim.
#[allow(clippy::type_complexity)]
fn sync_disc_entities(
    state: Res<LightcycleState>,
    mut player_disc: Query<
        (&mut Transform, &mut Visibility),
        (
            With<PlayerDiscEntity>,
            Without<OpponentEntity>,
            Without<OpponentDiscEntity>,
        ),
    >,
    mut opponent: Query<
        (&mut Transform, &mut Visibility),
        (
            With<OpponentEntity>,
            Without<PlayerDiscEntity>,
            Without<OpponentDiscEntity>,
        ),
    >,
    mut opponent_disc: Query<
        (&mut Transform, &mut Visibility),
        (
            With<OpponentDiscEntity>,
            Without<PlayerDiscEntity>,
            Without<OpponentEntity>,
        ),
    >,
    mut pickups: Query<
        (&DiscPickupEntity, &mut Visibility),
        (
            Without<PlayerDiscEntity>,
            Without<OpponentEntity>,
            Without<OpponentDiscEntity>,
        ),
    >,
) {
    let Some(run) = state.run.as_ref() else {
        return;
    };
    let Some(disc) = run.source_disc() else {
        return;
    };

    if let Ok((mut transform, mut visibility)) = player_disc.single_mut() {
        match disc.player_disc.as_ref() {
            Some(flying) => {
                transform.translation = disc_entity_position(flying.cell);
                *visibility = Visibility::Visible;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
    if let Ok((mut transform, mut visibility)) = opponent.single_mut() {
        if disc.opponent.alive {
            // The opponent steps a whole cell at a time. Render it partway to the
            // cell it is walking into so it glides instead of teleporting; while
            // it charges it stands exactly on its cell, so the shot is readable.
            let progress = if disc.opponent.windup > 0.0 {
                0.0
            } else {
                disc.opponent.move_clock.clamp(0.0, 1.0)
            };
            let (dx, dz) = disc.opponent.heading.delta();
            transform.translation =
                config::ground_position(disc.opponent.cell.0, disc.opponent.cell.1)
                    + Vec3::new(dx as f32, 0.0, dz as f32) * (progress * config::GRID_SPACING)
                    + Vec3::Y * (config::RECOGNIZER_HEIGHT * 0.5);
            // Swell while winding up, so its shot is telegraphed.
            let charge = (disc.opponent.windup / config::DISC_OPPONENT_WINDUP).clamp(0.0, 1.0);
            transform.scale =
                Vec3::new(1.0 + charge * 0.35, 1.0 - charge * 0.2, 1.0 + charge * 0.35);
            *visibility = Visibility::Visible;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
    if let Ok((mut transform, mut visibility)) = opponent_disc.single_mut() {
        match disc.opponent.disc.as_ref() {
            Some(flying) => {
                transform.translation = disc_entity_position(flying.cell);
                *visibility = Visibility::Visible;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
    for (pickup, mut visibility) in &mut pickups {
        *visibility = if disc.taken.contains(&pickup.index) {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

/// Places the pooled rocks and beams of an asteroid field. Anything past the
/// live end of the sim's vectors is hidden, so splits and pops need no spawning.
#[allow(clippy::type_complexity)]
fn sync_asteroid_entities(
    state: Res<LightcycleState>,
    mut rocks: Query<
        (&RockEntity, &mut Transform, &mut Visibility),
        (Without<BeamEntity>, Without<CycleEntity>),
    >,
    mut beams: Query<
        (&BeamEntity, &mut Transform, &mut Visibility),
        (Without<RockEntity>, Without<CycleEntity>),
    >,
) {
    // Only an actual asteroid field owns rock entities; a disc-wars ring also
    // carries a (never stepped) field sim, so check the game kind too.
    let Some(run) = state.run.as_ref() else {
        return;
    };
    if run.source_game() != Some(SourceGame::Asteroids) {
        return;
    }
    let Some(sim) = run.source_asteroids() else {
        return;
    };

    for (entity, mut transform, mut visibility) in &mut rocks {
        match sim.rocks.get(entity.index) {
            Some(rock) => {
                let radius = rock.size.radius();
                transform.translation = Vec3::new(rock.x, radius, rock.z);
                transform.rotation =
                    Quat::from_rotation_y(rock.angle) * Quat::from_rotation_x(rock.angle * 0.61);
                transform.scale = Vec3::splat(radius * 2.0);
                *visibility = Visibility::Visible;
            }
            None => *visibility = Visibility::Hidden,
        }
    }

    for (entity, mut transform, mut visibility) in &mut beams {
        match sim.beams.get(entity.index) {
            Some(beam) => {
                transform.translation = Vec3::new(beam.x, 0.35, beam.z);
                transform.rotation = Quat::from_rotation_y(-beam.vz.atan2(beam.vx));
                transform.scale = Vec3::new(config::ASTEROIDS_BEAM_LENGTH, 0.12, 0.12);
                *visibility = Visibility::Visible;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}

/// Places the pooled bugs and beams of a Galaga field. Dead bugs and spent
/// beams are hidden rather than despawned, so the pool never needs to grow.
#[allow(clippy::type_complexity)]
fn sync_galaga_entities(
    state: Res<LightcycleState>,
    mut bugs: Query<
        (&BugEntity, &mut Transform, &mut Visibility),
        (Without<GalagaBeamEntity>, Without<CycleEntity>),
    >,
    mut beams: Query<
        (&GalagaBeamEntity, &mut Transform, &mut Visibility),
        (Without<BugEntity>, Without<CycleEntity>),
    >,
) {
    let Some(sim) = state.run.as_ref().and_then(|run| run.source_galaga()) else {
        return;
    };

    for (entity, mut transform, mut visibility) in &mut bugs {
        match sim.bugs.get(entity.index) {
            Some(bug) if bug.alive => {
                transform.translation = Vec3::new(bug.x, config::GALAGA_BUG_HEIGHT * 0.5, bug.z);
                *visibility = Visibility::Visible;
            }
            _ => *visibility = Visibility::Hidden,
        }
    }

    for (entity, mut transform, mut visibility) in &mut beams {
        match sim.beams.get(entity.index) {
            Some(beam) => {
                transform.translation = Vec3::new(beam.x, 0.35, beam.z);
                transform.scale = Vec3::new(0.12, 0.12, config::GALAGA_BEAM_LENGTH);
                *visibility = Visibility::Visible;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}

/// Places the pooled dots and ghosts of a Pac-Man maze.
#[allow(clippy::type_complexity)]
fn sync_pacman_entities(
    state: Res<LightcycleState>,
    mut dots: Query<(&DotEntity, &mut Visibility), (Without<GhostEntity>, Without<CycleEntity>)>,
    mut ghosts: Query<
        (&GhostEntity, &mut Transform, &mut Visibility),
        (Without<DotEntity>, Without<CycleEntity>),
    >,
) {
    let Some(sim) = state.run.as_ref().and_then(|run| run.source_pacman()) else {
        return;
    };
    for (entity, mut visibility) in &mut dots {
        *visibility = if sim.dots.contains(&entity.cell) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for (entity, mut transform, mut visibility) in &mut ghosts {
        match sim.ghosts.get(entity.index) {
            Some(ghost) => {
                transform.translation = Vec3::new(ghost.x, 0.9, ghost.z);
                *visibility = Visibility::Visible;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}

/// Places the pooled gem cells of a Columns well.
#[allow(clippy::type_complexity)]
fn sync_columns_entities(
    state: Res<LightcycleState>,
    assets: Res<LightcycleAssets>,
    mut gems: Query<
        (
            &GemEntity,
            &mut Transform,
            &mut Visibility,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        Without<CycleEntity>,
    >,
) {
    let Some(sim) = state.run.as_ref().and_then(|run| run.source_columns()) else {
        return;
    };
    let rendered = sim.render_board();
    for (entity, mut transform, mut visibility, mut material) in &mut gems {
        match rendered.get(entity.index) {
            Some(Some(colour)) => {
                let col = entity.index % config::COLUMNS_COLS;
                let row = entity.index / config::COLUMNS_COLS;
                transform.translation = Vec3::new(
                    (col as f32 - (config::COLUMNS_COLS - 1) as f32 * 0.5) * 1.6,
                    ((config::COLUMNS_ROWS - 1) - row) as f32 * 1.6 + 0.8,
                    0.0,
                );
                material.0 =
                    assets.gem_materials[*colour as usize % config::COLUMNS_GEM_COLORS].clone();
                *visibility = Visibility::Visible;
            }
            _ => *visibility = Visibility::Hidden,
        }
    }
}

/// Places the pooled block cells of a Tetris board.
#[allow(clippy::type_complexity)]
fn sync_tetris_entities(
    state: Res<LightcycleState>,
    assets: Res<LightcycleAssets>,
    mut blocks: Query<
        (
            &BlockEntity,
            &mut Transform,
            &mut Visibility,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        Without<CycleEntity>,
    >,
) {
    let Some(sim) = state.run.as_ref().and_then(|run| run.source_tetris()) else {
        return;
    };
    let rendered = sim.render_board();
    for (entity, mut transform, mut visibility, mut material) in &mut blocks {
        match rendered.get(entity.index) {
            Some(Some(colour)) => {
                let col = entity.index % config::TETRIS_COLS;
                let row = entity.index / config::TETRIS_COLS;
                transform.translation = Vec3::new(
                    (col as f32 - (config::TETRIS_COLS - 1) as f32 * 0.5) * 1.2,
                    ((config::TETRIS_ROWS - 1) - row) as f32 * 1.2 + 0.6,
                    0.0,
                );
                material.0 = assets.tetris_materials[*colour as usize % 7].clone();
                *visibility = Visibility::Visible;
            }
            _ => *visibility = Visibility::Hidden,
        }
    }
}

/// Places the pooled obstacle cubes of a Frogger highway.
#[allow(clippy::type_complexity)]
fn sync_frogger_entities(
    state: Res<LightcycleState>,
    mut obstacles: Query<
        (&FrogObstacleEntity, &mut Transform, &mut Visibility),
        Without<CycleEntity>,
    >,
) {
    let Some(sim) = state.run.as_ref().and_then(|run| run.source_frogger()) else {
        return;
    };
    let cells = sim.obstacle_cells();
    for (entity, mut transform, mut visibility) in &mut obstacles {
        match cells.get(entity.index) {
            Some(&cell) => {
                let (x, z) = FroggerSim::center(cell);
                transform.translation = Vec3::new(x, 0.6, z);
                *visibility = Visibility::Visible;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}

/// Relights Q*bert cubes and places the pooled enemies.
#[allow(clippy::type_complexity)]
fn sync_qbert_entities(
    state: Res<LightcycleState>,
    assets: Res<LightcycleAssets>,
    mut cubes: Query<
        (
            &QbertCubeEntity,
            &mut Transform,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        Without<QbertEnemyEntity>,
    >,
    mut enemies: Query<
        (&QbertEnemyEntity, &mut Transform, &mut Visibility),
        Without<QbertCubeEntity>,
    >,
) {
    let Some(sim) = state.run.as_ref().and_then(|run| run.source_qbert()) else {
        return;
    };
    for (entity, mut transform, mut material) in &mut cubes {
        let lit = sim.lit.get(QbertSim::cube_index(entity.row, entity.index));
        let (x, z) = QbertSim::cube_position(entity.row, entity.index);
        let y =
            (config::QBERT_ROWS as f32 - 1.0 - entity.row as f32) * config::QBERT_CUBE_HEIGHT * 0.5;
        transform.translation = Vec3::new(x, y, z);
        material.0 = if lit == Some(&true) {
            assets.qbert_cube_lit.clone()
        } else {
            assets.qbert_cube_dim.clone()
        };
    }
    for (entity, mut transform, mut visibility) in &mut enemies {
        match sim.enemies.get(entity.index) {
            Some(enemy) => {
                let (x, z) = QbertSim::cube_position(enemy.row, enemy.index);
                let y = (config::QBERT_ROWS as f32 - 1.0 - enemy.row as f32)
                    * config::QBERT_CUBE_HEIGHT
                    * 0.5
                    + config::QBERT_CUBE_HEIGHT * 0.8;
                transform.translation = Vec3::new(x, y, z);
                *visibility = Visibility::Visible;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}

/// Places the pooled crates and bombs of a Bomberman room.
#[allow(clippy::type_complexity)]
fn sync_bomberman_entities(
    state: Res<LightcycleState>,
    mut crates: Query<
        (&BomberCrateEntity, &mut Visibility),
        (Without<BomberBombEntity>, Without<CycleEntity>),
    >,
    mut bombs: Query<
        (&BomberBombEntity, &mut Transform, &mut Visibility),
        (Without<BomberCrateEntity>, Without<CycleEntity>),
    >,
) {
    let Some(sim) = state.run.as_ref().and_then(|run| run.source_bomberman()) else {
        return;
    };
    for (entity, mut visibility) in &mut crates {
        *visibility = if sim.crates.contains(&entity.cell) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for (entity, mut transform, mut visibility) in &mut bombs {
        match sim.bombs.get(entity.index) {
            Some(bomb) => {
                let (x, z) = BomberSim::center(bomb.cell);
                transform.translation = Vec3::new(x, 0.6, z);
                *visibility = Visibility::Visible;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}

/// Places the pooled balls of a Plinko board.
#[allow(clippy::type_complexity)]
fn sync_plinko_entities(
    state: Res<LightcycleState>,
    mut balls: Query<(&PlinkoBallEntity, &mut Transform, &mut Visibility), Without<CycleEntity>>,
) {
    let Some(sim) = state.run.as_ref().and_then(|run| run.source_plinko()) else {
        return;
    };
    for (entity, mut transform, mut visibility) in &mut balls {
        match sim.balls.get(entity.index) {
            Some(ball) => {
                transform.translation = Vec3::new(ball.x, ball.y, 0.0);
                *visibility = Visibility::Visible;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}

/// Hides a snake ring's power-ups once eaten, and its exit bar once unlocked.
fn sync_snake_entities(
    state: Res<LightcycleState>,
    mut food: Query<(&SnakeFoodEntity, &mut Visibility), Without<SnakeGateLock>>,
    mut lock: Query<&mut Visibility, (With<SnakeGateLock>, Without<SnakeFoodEntity>)>,
) {
    let Some(snake) = state.run.as_ref().and_then(|run| run.source_snake()) else {
        return;
    };

    for (item, mut visibility) in &mut food {
        *visibility = match snake.food.get(item.index) {
            Some(food) if !food.eaten => Visibility::Visible,
            _ => Visibility::Hidden,
        };
    }

    if let Ok(mut visibility) = lock.single_mut() {
        *visibility = if snake.exit_open {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

/// Poses the on-foot character, whichever game it belongs to.
fn sync_character_entities(
    state: Res<LightcycleState>,
    time: Res<Time>,
    mut character: Query<(&mut Transform, &mut CharacterAnim), Without<CycleEntity>>,
) {
    let Some(run) = state.run.as_ref() else {
        return;
    };
    let Some(pose) = character_pose(run) else {
        return;
    };

    let dt = time.delta_secs();
    for (mut transform, mut anim) in &mut character {
        // The stealth sim moves in whole cells; easing toward the cell turns
        // that into a glide, and the walk clip supplies the limbs. The
        // platformer's physics is already continuous.
        let base = if pose.smooth {
            // Cover the ground at the pace the sim steps, instead of easing to
            // each cell and waiting. The second term only bites once the figure
            // has fallen behind, so a frame hitch does not leave it trailing.
            let to_target = pose.target - anim.base;
            let distance = to_target.length();
            let travel = config::STEALTH_WALK_SPEED.max(distance * 2.0) * dt;
            if distance <= travel {
                pose.target
            } else {
                anim.base + to_target / distance * travel
            }
        } else {
            pose.target
        };
        anim.base = base;
        transform.translation = base;
        transform.rotation = Quat::from_rotation_y(pose.yaw);
    }
}

/// Where the on-foot character should be and which way it faces.
struct CharacterPose {
    /// Ground position the figure is walking toward.
    target: Vec3,
    yaw: f32,
    /// True when the sim moves in grid steps that need easing out.
    smooth: bool,
}

/// The unit vector a heading points along, in the `(x, z)` the world is built on.
fn unit_of(heading: Heading) -> (f32, f32) {
    let angle = heading_angle(heading);
    (angle.cos(), angle.sin())
}

/// One cell along `heading`. This mirrors the sim's own step for a view-only
/// walk along a wall, so a mismatch could only ever misplace the camera.
fn step_cell(cell: (i32, i32), heading: Heading) -> (i32, i32) {
    match heading {
        Heading::PosX => (cell.0 + 1, cell.1),
        Heading::NegX => (cell.0 - 1, cell.1),
        Heading::PosZ => (cell.0, cell.1 + 1),
        Heading::NegZ => (cell.0, cell.1 - 1),
    }
}

fn character_pose(run: &ActiveRun) -> Option<CharacterPose> {
    if let Some(level) = run.source_platformer() {
        return Some(CharacterPose {
            target: Vec3::new(level.runner.x, level.runner.y, 0.0),
            // A quarter turn each way, not a half: the model's forward is
            // `+Z`, so facing along the level's `X` axis means pointing it at
            // `+X` or `-X`.
            yaw: if level.runner.facing >= 0.0 {
                std::f32::consts::FRAC_PI_2
            } else {
                -std::f32::consts::FRAC_PI_2
            },
            smooth: false,
        });
    }
    run.source_stealth().map(|room| {
        // Backed against a wall, the figure is leaned into it. Standing a whole
        // cell short reads as not quite touching, which loses the pose entirely;
        // `hug` is the wall's direction, so the lean is toward it. It eases in and
        // out through the same smoothing as the walking, so nothing snaps.
        let stand = config::ground_position(room.character.0, room.character.1);
        let target = match room.hug {
            Some(wall) => {
                let angle = heading_angle(wall);
                stand + Vec3::new(angle.cos(), 0.0, angle.sin()) * config::STEALTH_HUG_LEAN
            }
            None => stand,
        };
        CharacterPose {
            target,
            // The sim already faces the figure away from a wall it is hugging, so
            // this is the walking facing in every case.
            yaw: std::f32::consts::FRAC_PI_2 - heading_angle(room.heading),
            smooth: true,
        }
    })
}

/// Builds the walk graph for the character's player and attaches it. The glTF
/// loader makes the `AnimationPlayer` but leaves the graph to us.
fn prepare_character_walk(
    mut commands: Commands,
    assets: Res<LightcycleAssets>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    players: Query<Entity, (With<AnimationPlayer>, Without<CharacterWalk>)>,
) {
    if players.is_empty() {
        return;
    }
    let Some(clip) = gltfs
        .get(&assets.tron_gltf)
        .and_then(|gltf| gltf.named_animations.get(config::WALK_CLIP))
        .cloned()
    else {
        return;
    };
    for entity in &players {
        let (graph, nodes) = AnimationGraph::from_clips([clip.clone()]);
        if let Some(index) = nodes.first().copied() {
            commands.entity(entity).insert((
                AnimationGraphHandle(graphs.add(graph)),
                CharacterWalk(index),
            ));
        }
    }
}

/// Plays the walk while the character is moving and rewinds it when it stops,
/// so it never stands mid-stride. The rate is matched to ground speed: a clip
/// run at the wrong speed makes the feet skate.
fn drive_character_walk(
    state: Res<LightcycleState>,
    assets: Res<LightcycleAssets>,
    gltfs: Res<Assets<Gltf>>,
    mut reported: Local<bool>,
    mut character: Query<(&mut AnimationPlayer, &CharacterWalk), With<CharacterModel>>,
    mut guards: Query<(&mut AnimationPlayer, &CharacterWalk), Without<CharacterModel>>,
) {
    let Some(run) = state.run.as_ref() else {
        return;
    };
    // Say once, when the character's player first exists, what the animation
    // plumbing actually found. If the character ever walks without animating,
    // this line is the first thing to look at.
    if !*reported && !(character.is_empty() && guards.is_empty()) {
        *reported = true;
        let clips: Vec<Box<str>> = gltfs
            .get(&assets.tron_gltf)
            .map(|gltf| gltf.named_animations.keys().cloned().collect())
            .unwrap_or_default();
        info!(
            "walk animation: {} character player(s), {} other, {:?} clip, asset carries {clips:?}",
            character.iter().len(),
            guards.iter().len(),
            config::WALK_CLIP,
        );
    }
    // The patrol step rate, and the character's own ground speed in world units
    // per second. The guards walk continuously, so their clip must not stop
    // just because the player is waiting for them to pass.
    let step_speed = config::STEALTH_WALK_SPEED;
    let character_speed = if let Some(room) = run.source_stealth() {
        if room.walking { step_speed } else { 0.0 }
    } else if let Some(level) = run.source_platformer() {
        level.runner.vx.abs()
    } else {
        0.0
    };
    let guard_speed = if run.source_stealth().is_some() {
        step_speed
    } else {
        0.0
    };

    // The character's own player is tagged; every other player in the scene
    // belongs to a guard, which keeps walking while the player waits.
    walk_players(character.iter_mut(), character_speed);
    walk_players(guards.iter_mut(), guard_speed);
}

/// Tags every entity under an on-foot character, however deep.
fn tag_character_model(
    mut commands: Commands,
    characters: Query<Entity, With<CharacterEntity>>,
    children: Query<&Children>,
    tagged: Query<(), With<CharacterModel>>,
) {
    for character in &characters {
        let mut stack = vec![character];
        while let Some(entity) = stack.pop() {
            if tagged.get(entity).is_err() {
                commands.entity(entity).insert(CharacterModel);
            }
            if let Ok(kids) = children.get(entity) {
                stack.extend(kids.iter());
            }
        }
    }
}

/// Drives one set of players at a ground speed in world units per second. Zero
/// stops them, which is what standing still has to look like.
fn walk_players<'a>(
    players: impl Iterator<Item = (Mut<'a, AnimationPlayer>, &'a CharacterWalk)>,
    speed: f32,
) {
    for (mut player, walk) in players {
        if speed <= 0.05 {
            if player.is_playing_animation(walk.0) {
                player.stop(walk.0);
            }
            continue;
        }
        let rate = (speed / config::WALK_CLIP_GROUND).clamp(0.3, 2.5);
        let active = player.play(walk.0);
        // Bevy's default repeat mode is `Never`: the clip plays once and then
        // parks on its last frame, which reads as a character sliding along
        // frozen mid-stride. Loop it, and rewind it if a previous pass already
        // completed, since `play` never restarts an active animation.
        if active.is_finished() {
            active.replay();
        }
        active.set_speed(rate).repeat();
    }
}

#[allow(clippy::type_complexity)]
/// Keeps the breaker's ball and bricks glued to its sim.
fn sync_breaker_entities(
    state: Res<LightcycleState>,
    mut ball: Query<&mut Transform, (With<BallEntity>, Without<BrickEntity>)>,
    mut bricks: Query<
        (&BrickEntity, &mut Visibility),
        (Without<BallEntity>, Without<CharacterEntity>),
    >,
) {
    let Some(level) = state.run.as_ref().and_then(|run| run.source_breaker()) else {
        return;
    };
    for mut transform in &mut ball {
        transform.translation = Vec3::new(level.ball.x, level.ball.y, 0.0);
    }
    for (brick, mut visibility) in &mut bricks {
        *visibility = match level.bricks.get(brick.index) {
            Some(brick) if brick.alive => Visibility::Visible,
            _ => Visibility::Hidden,
        };
    }
}

#[allow(clippy::type_complexity)]
/// Walks the patrols and swings their vision cones.
fn sync_stealth_entities(
    state: Res<LightcycleState>,
    mut guards: Query<
        (&GuardEntity, &mut Transform),
        (Without<GuardConeEntity>, Without<CharacterEntity>),
    >,
    mut cones: Query<
        (&GuardConeEntity, &mut Transform),
        (Without<GuardEntity>, Without<CharacterEntity>),
    >,
) {
    let Some(room) = state.run.as_ref().and_then(|run| run.source_stealth()) else {
        return;
    };
    for (guard, mut transform) in &mut guards {
        if let Some(guard) = room.guards.get(guard.index) {
            let cell = guard.cell();
            transform.translation = config::ground_position(cell.0, cell.1);
            transform.rotation =
                Quat::from_rotation_y(std::f32::consts::FRAC_PI_2 - guard.patrol.heading().angle());
        }
    }
    for (cone, mut transform) in &mut cones {
        if let Some(guard) = room.guards.get(cone.index) {
            let cell = guard.cell();
            transform.translation = config::ground_position(cell.0, cell.1) + Vec3::Y * 0.08;
            transform.rotation =
                Quat::from_rotation_y(std::f32::consts::FRAC_PI_2 - guard.vision_angle());
        }
    }
}

fn disc_entity_position(cell: (i32, i32)) -> Vec3 {
    config::ground_position(cell.0, cell.1) + Vec3::Y * 0.35
}

/// Spin and bob the waiting pickups so they read as collectible.
fn animate_disc_pickups(time: Res<Time>, mut pickups: Query<(&DiscPickupEntity, &mut Transform)>) {
    let elapsed = time.elapsed_secs();
    for (pickup, mut transform) in &mut pickups {
        let bob = (elapsed * 2.2 + pickup.phase * std::f32::consts::TAU).sin() * 0.12;
        transform.translation.y = 0.45 + bob;
        transform.rotate_y(0.03);
    }
}

/// Tracks which alcove the rider is beside, for the ring's folio panel.
fn update_disc_focus(
    mut state: ResMut<LightcycleState>,
    mut marker: Query<&mut Transform, With<DocumentFocusMarker>>,
) {
    let Some(run) = state.run.as_mut() else {
        return;
    };
    let RunEnvironment::Source {
        layout,
        focused_block,
        ..
    } = &mut run.environment
    else {
        return;
    };
    *focused_block = layout.focused_block(run.sim.cell);
    let Some(index) = *focused_block else {
        return;
    };
    let landmark = layout.blocks[index].landmark;
    if let Ok(mut transform) = marker.single_mut() {
        transform.translation = config::ground_position(landmark.0, landmark.1) + Vec3::Y * 0.08;
    }
}

fn spawn_city_structures(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    arena: &Arena,
) {
    let all: Vec<_> = arena.structures.iter().collect();
    spawn_structure_layer(
        commands,
        meshes,
        &all,
        assets.city_foundation_material.clone(),
        city_foundation_mesh,
    );

    let glass: Vec<_> = arena
        .structures
        .iter()
        .filter(|structure| structure.kind == CityStructureKind::GlassFin)
        .collect();
    spawn_structure_layer(
        commands,
        meshes,
        &glass,
        assets.city_glass_material.clone(),
        city_body_mesh,
    );

    let theme = city_theme_index(arena.city_theme);
    for accent in 0..2 {
        let solid: Vec<_> = arena
            .structures
            .iter()
            .filter(|structure| {
                structure.accent as usize == accent && structure.kind != CityStructureKind::GlassFin
            })
            .collect();
        spawn_structure_layer(
            commands,
            meshes,
            &solid,
            assets.city_foundation_material.clone(),
            city_body_mesh,
        );

        let lit: Vec<_> = arena
            .structures
            .iter()
            .filter(|structure| structure.accent as usize == accent)
            .collect();
        spawn_structure_layer(
            commands,
            meshes,
            &lit,
            assets.city_accent_materials[theme][accent].clone(),
            city_cap_mesh,
        );
    }

    // One color for every ground seam, distinct from the lane markings, so the
    // edge you can crash into never reads as a stripe you can drive along.
    spawn_structure_layer(
        commands,
        meshes,
        &all,
        assets.city_accent_materials[theme][CITY_TRIM_ACCENT].clone(),
        city_base_trim_mesh,
    );

    for structure in arena
        .structures
        .iter()
        .filter(|structure| structure.kind == CityStructureKind::Pylon)
        .take(config::LIGHTCYCLE_CITY_BEACON_LIMIT)
    {
        let body_height = city_body_height(structure);
        let (_, emissive) = city_palette()[theme][structure.accent as usize];
        let phase = structure.pulse_phase as f32 / 3.0;
        commands.spawn((
            LightcycleSceneRoot,
            CityBeacon {
                base_height: config::LIGHTCYCLE_CITY_FOUNDATION_HEIGHT + body_height + 0.32,
                phase,
            },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.city_accent_materials[theme][structure.accent as usize].clone()),
            Transform::from_translation(
                config::ground_position(structure.cell.0, structure.cell.1)
                    + Vec3::Y * (config::LIGHTCYCLE_CITY_FOUNDATION_HEIGHT + body_height + 0.32),
            )
            .with_scale(Vec3::splat(0.22 + emissive.red.min(1.0) * 0.04)),
            Pickable::IGNORE,
        ));
    }
}

fn spawn_structure_layer(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    structures: &[&CityStructure],
    material: Handle<StandardMaterial>,
    build: fn(&CityStructure) -> Mesh,
) {
    for chunk in structures.chunks(config::MESH_CHUNK_SIZE) {
        let mut chunk = chunk.iter();
        let Some(first) = chunk.next() else {
            continue;
        };
        let mut mesh = build(first);
        for structure in chunk {
            mesh.merge(&build(structure))
                .expect("city structure meshes must be merge-compatible");
        }
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Pickable::IGNORE,
        ));
    }
}

fn city_foundation_mesh(structure: &CityStructure) -> Mesh {
    let height = config::LIGHTCYCLE_CITY_FOUNDATION_HEIGHT;
    Mesh::from(Cuboid::default()).transformed_by(
        Transform::from_translation(config::world_position(
            structure.cell.0,
            structure.cell.1,
            height,
        ))
        .with_scale(Vec3::new(
            config::LIGHTCYCLE_CITY_STRUCTURE_SIZE,
            height,
            config::LIGHTCYCLE_CITY_STRUCTURE_SIZE,
        )),
    )
}

fn city_body_height(structure: &CityStructure) -> f32 {
    let tier = structure.height_tier as f32;
    match structure.kind {
        CityStructureKind::Barrier => config::LIGHTCYCLE_CITY_BARRIER_HEIGHT + tier * 0.24,
        CityStructureKind::GlassFin => config::LIGHTCYCLE_CITY_GLASS_HEIGHT + tier * 0.4,
        CityStructureKind::Pylon => config::LIGHTCYCLE_CITY_PYLON_HEIGHT + tier * 0.7,
    }
}

fn city_body_scale(structure: &CityStructure, height: f32) -> Vec3 {
    let size = config::LIGHTCYCLE_CITY_STRUCTURE_SIZE;
    match structure.kind {
        CityStructureKind::Barrier => {
            if structure.along_x {
                Vec3::new(size, height, size * 0.42)
            } else {
                Vec3::new(size * 0.42, height, size)
            }
        }
        CityStructureKind::GlassFin => {
            if structure.along_x {
                Vec3::new(size, height, config::LIGHTCYCLE_CITY_FIN_THICKNESS)
            } else {
                Vec3::new(config::LIGHTCYCLE_CITY_FIN_THICKNESS, height, size)
            }
        }
        CityStructureKind::Pylon => Vec3::new(0.5, height, 0.5),
    }
}

fn city_body_mesh(structure: &CityStructure) -> Mesh {
    let height = city_body_height(structure);
    let position = config::ground_position(structure.cell.0, structure.cell.1)
        + Vec3::Y * (config::LIGHTCYCLE_CITY_FOUNDATION_HEIGHT + height * 0.5);
    Mesh::from(Cuboid::default()).transformed_by(
        Transform::from_translation(position).with_scale(city_body_scale(structure, height)),
    )
}

fn city_cap_mesh(structure: &CityStructure) -> Mesh {
    let body_height = city_body_height(structure);
    let cap_height = config::LIGHTCYCLE_CITY_CAP_HEIGHT;
    let mut scale = city_body_scale(structure, body_height);
    scale.y = cap_height;
    scale.x += 0.08;
    scale.z += 0.08;
    let position = config::ground_position(structure.cell.0, structure.cell.1)
        + Vec3::Y * (config::LIGHTCYCLE_CITY_FOUNDATION_HEIGHT + body_height + cap_height * 0.5);
    Mesh::from(Cuboid::default())
        .transformed_by(Transform::from_translation(position).with_scale(scale))
}

/// Glowing skirt around a structure's footprint. It is wider than the
/// foundation, so the visible part is a neon border tracing where the wall
/// stops and the floor starts.
fn city_base_trim_mesh(structure: &CityStructure) -> Mesh {
    let height = config::LIGHTCYCLE_CITY_BASE_TRIM_HEIGHT;
    let size = config::LIGHTCYCLE_CITY_STRUCTURE_SIZE + config::LIGHTCYCLE_CITY_BASE_TRIM_OVERHANG;
    let position =
        config::ground_position(structure.cell.0, structure.cell.1) + Vec3::Y * (height * 0.5);
    Mesh::from(Cuboid::default()).transformed_by(
        Transform::from_translation(position).with_scale(Vec3::new(size, height, size)),
    )
}

fn spawn_document_page(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    arena: &Arena,
    layout: &DocumentLayout,
) {
    spawn_document_rules(commands, assets, meshes, arena);
    spawn_document_walls(commands, assets, meshes, arena);
    spawn_document_arches(commands, assets, meshes, layout);
    spawn_document_glyphs(commands, assets, meshes, layout);
}

fn spawn_document_rules(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    arena: &Arena,
) {
    let Some(mesh) = document_rule_mesh(arena) else {
        return;
    };
    commands.spawn((
        LightcycleSceneRoot,
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(assets.document_rule_material.clone()),
        Pickable::IGNORE,
    ));
    commands.spawn((
        LightcycleSceneRoot,
        Mesh3d(meshes.add(document_margin_mesh(arena))),
        MeshMaterial3d(assets.document_margin_material.clone()),
        Pickable::IGNORE,
    ));
}

fn document_rule_mesh(arena: &Arena) -> Option<Mesh> {
    let spacing = config::GRID_SPACING;
    let mut merged: Option<Mesh> = None;
    let mut push = |mesh: Mesh| {
        if let Some(existing) = &mut merged {
            existing
                .merge(&mesh)
                .expect("document rule meshes must be merge-compatible");
        } else {
            merged = Some(mesh);
        }
    };

    for z in arena.min.1..=arena.max.1 {
        let center = Vec3::new(
            (arena.min.0 + arena.max.0) as f32 * spacing * 0.5,
            0.02,
            z as f32 * spacing,
        );
        let width = (arena.max.0 - arena.min.0 + 1) as f32 * spacing;
        push(Mesh::from(Cuboid::default()).transformed_by(
            Transform::from_translation(center).with_scale(Vec3::new(width, 0.03, 0.06)),
        ));
    }
    merged
}

fn document_margin_mesh(arena: &Arena) -> Mesh {
    let spacing = config::GRID_SPACING;
    let margin_x = (arena.min.0 as f32 - 0.15) * spacing;
    let center = Vec3::new(
        margin_x,
        0.03,
        (arena.min.1 + arena.max.1) as f32 * spacing * 0.5,
    );
    let depth = (arena.max.1 - arena.min.1 + 1) as f32 * spacing;
    Mesh::from(Cuboid::default()).transformed_by(
        Transform::from_translation(center).with_scale(Vec3::new(0.08, 0.04, depth)),
    )
}

fn spawn_document_walls(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    arena: &Arena,
) {
    let cells: Vec<_> = arena.street_walls.iter().copied().collect();
    for chunk in cells.chunks(config::MESH_CHUNK_SIZE) {
        let mut chunk = chunk.iter();
        let Some(&first) = chunk.next() else {
            continue;
        };
        let mut mesh = document_wall_mesh(first);
        for &cell in chunk {
            mesh.merge(&document_wall_mesh(cell))
                .expect("document wall meshes must be merge-compatible");
        }
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(assets.document_ink_material.clone()),
            Pickable::IGNORE,
        ));
    }
}

fn document_wall_mesh(cell: (i32, i32)) -> Mesh {
    let height = 1.35;
    Mesh::from(Cuboid::default()).transformed_by(
        Transform::from_translation(config::world_position(cell.0, cell.1, height))
            .with_scale(Vec3::new(1.7, height, 0.55)),
    )
}

fn spawn_document_arches(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    layout: &DocumentLayout,
) {
    let headings: Vec<_> = layout
        .blocks
        .iter()
        .filter(|block| matches!(block.kind, crate::document::DocBlockKind::Heading(_)))
        .collect();
    for chunk in headings.chunks(config::MESH_CHUNK_SIZE) {
        let mut chunk = chunk.iter();
        let Some(first) = chunk.next() else {
            continue;
        };
        let mut mesh = document_arch_mesh(first);
        for block in chunk {
            mesh.merge(&document_arch_mesh(block))
                .expect("document arch meshes must be merge-compatible");
        }
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(assets.document_heading_material.clone()),
            Pickable::IGNORE,
        ));
    }
}

fn document_arch_mesh(block: &crate::document::PlacedBlock) -> Mesh {
    let (x, z) = block.landmark;
    let origin = config::ground_position(x, z);
    let (span, depth) = if block.along_x {
        (2.4, 0.28)
    } else {
        (0.28, 2.4)
    };
    let left = Mesh::from(Cuboid::default()).transformed_by(
        Transform::from_translation(origin + Vec3::new(-span * 0.35, 1.1, -depth * 0.35))
            .with_scale(Vec3::new(0.22, 2.2, 0.22)),
    );
    let mut mesh = left;
    mesh.merge(
        &Mesh::from(Cuboid::default()).transformed_by(
            Transform::from_translation(origin + Vec3::new(span * 0.35, 1.1, depth * 0.35))
                .with_scale(Vec3::new(0.22, 2.2, 0.22)),
        ),
    )
    .expect("arch posts must merge");
    mesh.merge(&Mesh::from(Cuboid::default()).transformed_by(
        Transform::from_translation(origin + Vec3::Y * 2.25).with_scale(Vec3::new(
            span,
            0.22,
            depth.max(0.4),
        )),
    ))
    .expect("arch lintel must merge");
    mesh
}

fn spawn_document_glyphs(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    layout: &DocumentLayout,
) {
    let mut remaining = config::DOCUMENT_MAX_GLYPHS;
    let mut heading_mesh: Option<Mesh> = None;
    let mut plaque_mesh: Option<Mesh> = None;
    for block in &layout.blocks {
        if remaining == 0 {
            break;
        }
        let (mesh, used) = document_glyph_line_mesh(block, remaining);
        remaining = remaining.saturating_sub(used);
        if used == 0 {
            continue;
        }
        let target = if matches!(block.kind, crate::document::DocBlockKind::Heading(_)) {
            &mut heading_mesh
        } else {
            &mut plaque_mesh
        };
        if let Some(existing) = target {
            existing
                .merge(&mesh)
                .expect("glyph meshes must be merge-compatible");
        } else {
            *target = Some(mesh);
        }
    }

    if let Some(mesh) = heading_mesh {
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(assets.document_heading_material.clone()),
            Pickable::IGNORE,
        ));
    }
    if let Some(mesh) = plaque_mesh {
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(assets.document_ink_material.clone()),
            Pickable::IGNORE,
        ));
    }
}

fn document_glyph_line_mesh(
    block: &crate::document::PlacedBlock,
    remaining: usize,
) -> (Mesh, usize) {
    let origin = config::ground_position(block.landmark.0, block.landmark.1);
    let heading = matches!(block.kind, crate::document::DocBlockKind::Heading(_));
    let pixel = if heading { 0.09 } else { 0.055 };
    let height = if heading { 2.55 } else { 0.85 };
    let advance = document_line_advance(block.along_x);
    let max_chars = remaining.min(if heading {
        config::DOCUMENT_HEADING_GLYPHS
    } else {
        config::DOCUMENT_PARAGRAPH_GLYPHS
    });
    let chars: Vec<char> = block.preview.chars().take(max_chars).collect();
    let mut mesh: Option<Mesh> = None;
    let mut used = 0usize;
    for (index, ch) in chars.iter().enumerate() {
        let glyph = glyph_pixels(*ch);
        used += 1;
        let offset = glyph_char_offset(advance, index, chars.len(), pixel);
        for (row, row_bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if row_bits & (1 << col) == 0 {
                    continue;
                }
                let local = glyph_pixel_offset(advance, col, row, pixel);
                let cube = Mesh::from(Cuboid::default()).transformed_by(
                    Transform::from_translation(origin + Vec3::Y * height + offset + local)
                        .with_scale(Vec3::splat(pixel * 0.85)),
                );
                if let Some(existing) = &mut mesh {
                    existing
                        .merge(&cube)
                        .expect("glyph pixels must be merge-compatible");
                } else {
                    mesh = Some(cube);
                }
            }
        }
    }
    (
        mesh.unwrap_or_else(|| collapsed_document_glyph(origin)),
        used,
    )
}

/// Direction a block's text reads in, which is also the axis its glyph columns
/// run along.
///
/// A paragraph walls off one side of its spine cells, so the reader always
/// arrives from the other side: `+Z` for a row laid along X, `+X` for one laid
/// along Z. Screen right for those two viewpoints is `+X` and `-Z`, and text has
/// to read toward screen right.
fn document_line_advance(along_x: bool) -> Vec3 {
    if along_x { Vec3::X } else { Vec3::NEG_Z }
}

/// Offset of one character's origin from the middle of its line.
fn glyph_char_offset(advance: Vec3, index: usize, count: usize, pixel: f32) -> Vec3 {
    advance * ((index as f32 - (count as f32 - 1.0) * 0.5) * pixel * 9.0)
}

/// Offset of one glyph pixel from its own character's origin.
///
/// Columns run along the same `advance` the characters are placed along, so a
/// letterform cannot end up mirrored against the order of the line it sits in.
/// font8x8 packs each row least-significant bit first, making column 0 the
/// letter's leftmost pixel, so it belongs at the near end of `advance`. Row 0 is
/// the top of the glyph and belongs at the top of the line.
fn glyph_pixel_offset(advance: Vec3, col: usize, row: usize, pixel: f32) -> Vec3 {
    advance * (col as f32 * pixel) + Vec3::Y * ((7 - row) as f32 * pixel)
}

fn collapsed_document_glyph(origin: Vec3) -> Mesh {
    Mesh::from(Cuboid::default()).transformed_by(
        Transform::from_translation(origin + Vec3::Y * 0.2).with_scale(Vec3::splat(0.08)),
    )
}

fn glyph_pixels(character: char) -> [u8; 8] {
    if (character as u32) < 128 {
        font8x8::legacy::BASIC_LEGACY[character as usize]
    } else {
        // Unsupported glyphs keep a diamond placeholder; the folio panel shows
        // the original Unicode.
        [
            0b00011000, 0b00111100, 0b01111110, 0b11111111, 0b01111110, 0b00111100, 0b00011000,
            0b00000000,
        ]
    }
}

fn spawn_document_focus_marker(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    run: &ActiveRun,
) {
    let pose = cycle_cell_pose(&run.sim);
    commands.spawn((
        LightcycleSceneRoot,
        DocumentFocusMarker,
        Mesh3d(assets.unit_cube.clone()),
        MeshMaterial3d(assets.document_focus_material.clone()),
        Transform::from_translation(pose_world_position(&pose) + Vec3::Y * 0.08)
            .with_scale(Vec3::new(1.4, 0.08, 1.4)),
        Pickable::IGNORE,
    ));
}

fn spawn_towers(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    run: &ActiveRun,
) {
    let RunEnvironment::Directory { nodes, .. } = &run.environment else {
        return;
    };
    for (filter, material) in [
        (
            (|node: &FileNode| node.is_dir) as fn(&FileNode) -> bool,
            assets.dir_tower_material.clone(),
        ),
        (
            |node: &FileNode| !node.is_dir && node.is_source(),
            assets.source_tower_material.clone(),
        ),
        (
            |node: &FileNode| !node.is_dir && node.is_markdown(),
            assets.markdown_tower_material.clone(),
        ),
        (
            |node: &FileNode| !node.is_dir && !node.is_markdown() && !node.is_source(),
            assets.file_tower_material.clone(),
        ),
    ] {
        let matching: Vec<&FileNode> = nodes.iter().filter(|node| filter(node)).collect();
        for chunk in matching.chunks(config::MESH_CHUNK_SIZE) {
            let Some(mesh) = build_tower_chunk_mesh(chunk) else {
                continue;
            };
            commands.spawn((
                LightcycleSceneRoot,
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material.clone()),
                Pickable::IGNORE,
            ));
        }
    }
}

fn build_tower_chunk_mesh(nodes: &[&FileNode]) -> Option<Mesh> {
    let mut nodes = nodes.iter();
    let first = tower_cube_mesh(nodes.next()?);
    let mut mesh = first;
    for node in nodes {
        mesh.merge(&tower_cube_mesh(node))
            .expect("tower cuboid meshes must be merge-compatible");
    }
    Some(mesh)
}

fn tower_cube_mesh(node: &FileNode) -> Mesh {
    let (x, z) = tower_position(node.grid_pos);
    let height = node.calculate_height();
    Mesh::from(Cuboid::default()).transformed_by(
        Transform::from_translation(config::world_position(x, z, height)).with_scale(Vec3::new(
            config::LIGHTCYCLE_TOWER_SIZE,
            height,
            config::LIGHTCYCLE_TOWER_SIZE,
        )),
    )
}

fn spawn_road_markings(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    arena: &Arena,
) {
    let mut cells: Vec<_> = arena.roads.iter().copied().collect();
    let center = arena.center();
    cells.sort_unstable_by_key(|cell| {
        ((cell.0 - center.0).abs() + (cell.1 - center.1).abs(), *cell)
    });
    cells.truncate(config::LIGHTCYCLE_CITY_ROAD_RENDER_LIMIT);

    let theme = city_theme_index(arena.city_theme);
    for chunk in cells.chunks(config::MESH_CHUNK_SIZE) {
        let mut chunk = chunk.iter();
        let Some(&first) = chunk.next() else {
            continue;
        };
        let mut mesh = road_marking_mesh(first, &arena.roads);
        for &cell in chunk {
            mesh.merge(&road_marking_mesh(cell, &arena.roads))
                .expect("road marking meshes must be merge-compatible");
        }
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(assets.city_accent_materials[theme][0].clone()),
            Pickable::IGNORE,
        ));
    }
}

fn road_marking_mesh(cell: (i32, i32), roads: &std::collections::BTreeSet<(i32, i32)>) -> Mesh {
    let spacing = config::GRID_SPACING;
    let line_width = 0.075;
    let line_height = 0.035;
    let center = config::ground_position(cell.0, cell.1) + Vec3::Y * 0.025;
    let mut mesh = Mesh::from(Cuboid::default()).transformed_by(
        Transform::from_translation(center).with_scale(Vec3::new(
            line_width * 2.5,
            line_height,
            line_width * 2.5,
        )),
    );

    if roads.contains(&(cell.0 + 1, cell.1)) {
        let segment =
            Mesh::from(Cuboid::default()).transformed_by(
                Transform::from_translation(center + Vec3::X * spacing * 0.5)
                    .with_scale(Vec3::new(spacing, line_height, line_width)),
            );
        mesh.merge(&segment)
            .expect("road marking cuboids must be merge-compatible");
    }
    if roads.contains(&(cell.0, cell.1 + 1)) {
        let segment =
            Mesh::from(Cuboid::default()).transformed_by(
                Transform::from_translation(center + Vec3::Z * spacing * 0.5)
                    .with_scale(Vec3::new(line_width, line_height, spacing)),
            );
        mesh.merge(&segment)
            .expect("road marking cuboids must be merge-compatible");
    }
    mesh
}

/// World-space plane each wall rail sits in, half a cell outside the playable area.
fn wall_plane(arena: &Arena, wall: Wall) -> f32 {
    let spacing = config::GRID_SPACING;
    match wall {
        Wall::NegX => (arena.min.0 as f32 - 0.5) * spacing,
        Wall::PosX => (arena.max.0 as f32 + 0.5) * spacing,
        Wall::NegZ => (arena.min.1 as f32 - 0.5) * spacing,
        Wall::PosZ => (arena.max.1 as f32 + 0.5) * spacing,
    }
}

/// World-space extent of a wall along its own axis, corner to corner.
fn wall_extent(arena: &Arena, wall: Wall) -> (f32, f32) {
    match wall {
        Wall::NegZ | Wall::PosZ => (wall_plane(arena, Wall::NegX), wall_plane(arena, Wall::PosX)),
        Wall::NegX | Wall::PosX => (wall_plane(arena, Wall::NegZ), wall_plane(arena, Wall::PosZ)),
    }
}

/// Splits a rail's extent around an optional gap, dropping segments too short to
/// be worth drawing.
fn rail_segments(min: f32, max: f32, gap: Option<(f32, f32)>) -> Vec<(f32, f32)> {
    let Some((gap_min, gap_max)) = gap else {
        return vec![(min, max)];
    };

    [(min, gap_min.min(max)), (gap_max.max(min), max)]
        .into_iter()
        .filter(|(start, end)| end - start > 0.01)
        .collect()
}

fn spawn_arena_walls(commands: &mut Commands, assets: &LightcycleAssets, arena: &Arena) {
    for wall in [Wall::NegX, Wall::PosX, Wall::NegZ, Wall::PosZ] {
        spawn_wall_rail(commands, assets, arena, wall);
    }

    spawn_parent_gate(commands, assets, arena);
}

/// Draws one arena wall, leaving a real opening where the parent gate cuts
/// through it so the gate can be ridden through rather than looked at.
fn spawn_wall_rail(commands: &mut Commands, assets: &LightcycleAssets, arena: &Arena, wall: Wall) {
    let height = config::LIGHTCYCLE_WALL_HEIGHT;
    let thickness = config::LIGHTCYCLE_WALL_THICKNESS;
    let plane = wall_plane(arena, wall);
    let (min, max) = wall_extent(arena, wall);

    let gap = arena
        .parent_portal
        .filter(|portal| portal.wall == wall)
        .map(|portal| gate_world_span(&portal));

    let trim_height = config::LIGHTCYCLE_CITY_BASE_TRIM_HEIGHT;
    let trim_thickness = thickness + config::LIGHTCYCLE_CITY_BASE_TRIM_OVERHANG;
    let accent = if arena.kind == ArenaKind::Document {
        assets.document_folio_material.clone()
    } else {
        assets.city_accent_materials[city_theme_index(arena.city_theme)][CITY_TRIM_ACCENT].clone()
    };

    for (start, end) in rail_segments(min, max, gap) {
        let center = (start + end) * 0.5;
        let length = end - start;
        let (translation, scale) = match wall {
            Wall::NegZ | Wall::PosZ => (
                Vec3::new(center, height * 0.5, plane),
                Vec3::new(length, height, thickness),
            ),
            Wall::NegX | Wall::PosX => (
                Vec3::new(plane, height * 0.5, center),
                Vec3::new(thickness, height, length),
            ),
        };

        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.wall_material.clone()),
            Transform::from_translation(translation).with_scale(scale),
            Pickable::IGNORE,
        ));

        // Same light-line the structures get, so the perimeter reads as a wall
        // standing on the floor rather than the floor fading into darkness.
        let (trim_translation, trim_scale) = match wall {
            Wall::NegZ | Wall::PosZ => (
                Vec3::new(center, trim_height * 0.5, plane),
                Vec3::new(length, trim_height, trim_thickness),
            ),
            Wall::NegX | Wall::PosX => (
                Vec3::new(plane, trim_height * 0.5, center),
                Vec3::new(trim_thickness, trim_height, length),
            ),
        };

        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(accent.clone()),
            Transform::from_translation(trim_translation).with_scale(trim_scale),
            Pickable::IGNORE,
        ));
    }
}

/// World-space extent of the gate along its wall, covering exactly the cells the
/// simulation accepts.
fn gate_world_span(portal: &ParentPortal) -> (f32, f32) {
    let spacing = config::GRID_SPACING;
    let (from, _) = portal.along_span();
    let start = (from as f32 - 0.5) * spacing;
    (start, start + portal.width_cells() as f32 * spacing)
}

/// Builds the animated parent gate: two posts, a lintel, and light bars that
/// sweep up through the opening.
///
/// Everything is parented to a root whose rotation puts the wall's axis on local
/// +X, so the pieces below are laid out once instead of per wall.
fn spawn_parent_gate(commands: &mut Commands, assets: &LightcycleAssets, arena: &Arena) {
    let Some(portal) = arena.parent_portal else {
        return;
    };

    let height = config::LIGHTCYCLE_PORTAL_HEIGHT;
    let frame = config::LIGHTCYCLE_PORTAL_FRAME_THICKNESS;
    let depth = config::LIGHTCYCLE_WALL_THICKNESS * 3.0;
    let (span_min, span_max) = gate_world_span(&portal);
    let opening = span_max - span_min;
    let center = (span_min + span_max) * 0.5;
    let plane = wall_plane(arena, portal.wall);

    let (translation, rotation) = match portal.wall {
        Wall::NegZ | Wall::PosZ => (Vec3::new(center, 0.0, plane), Quat::IDENTITY),
        Wall::NegX | Wall::PosX => (
            Vec3::new(plane, 0.0, center),
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        ),
    };

    let frame_material = if arena.kind == ArenaKind::Document {
        assets.document_folio_material.clone()
    } else {
        assets.portal_material.clone()
    };
    let bar_material = if arena.kind == ArenaKind::Document {
        assets.document_focus_material.clone()
    } else {
        assets.portal_bar_material.clone()
    };

    let half = opening * 0.5;
    let mut gate = commands.spawn((
        LightcycleSceneRoot,
        Transform::from_translation(translation).with_rotation(rotation),
        Visibility::default(),
    ));

    gate.with_children(|frames| {
        let mut piece = |translation: Vec3, scale: Vec3| {
            frames.spawn((
                GateFrame,
                Mesh3d(assets.unit_cube.clone()),
                MeshMaterial3d(frame_material.clone()),
                Transform::from_translation(translation).with_scale(scale),
                Pickable::IGNORE,
            ));
        };

        // Posts on the cell boundaries the gate starts and ends at.
        piece(
            Vec3::new(-half, height * 0.5, 0.0),
            Vec3::new(frame, height, depth),
        );
        piece(
            Vec3::new(half, height * 0.5, 0.0),
            Vec3::new(frame, height, depth),
        );
        // Lintel spanning them.
        piece(
            Vec3::new(0.0, height, 0.0),
            Vec3::new(opening + frame, frame, depth),
        );

        let bars = config::LIGHTCYCLE_PORTAL_BAR_COUNT;
        for index in 0..bars {
            frames.spawn((
                GateScanBar {
                    offset: index as f32 / bars as f32,
                    travel: height,
                },
                Mesh3d(assets.unit_cube.clone()),
                MeshMaterial3d(bar_material.clone()),
                Transform::from_scale(Vec3::new(
                    opening - frame,
                    config::LIGHTCYCLE_PORTAL_BAR_HEIGHT,
                    depth * 0.5,
                )),
                Pickable::IGNORE,
            ));
        }
    });
}

/// Brightness of the gate frame at `elapsed`, from 0 at the pulse's trough to 1
/// at its peak.
fn gate_pulse(elapsed: f32) -> f32 {
    0.5 + 0.5 * (elapsed * config::LIGHTCYCLE_PORTAL_PULSE_SPEED).sin()
}

/// Height a bar has swept to within its opening, wrapping back to the ground
/// once it reaches the lintel.
fn gate_bar_height(bar: &GateScanBar, elapsed: f32) -> f32 {
    (bar.offset + elapsed * config::LIGHTCYCLE_PORTAL_BAR_SPEED).fract() * bar.travel
}

/// Pulses the gate frame and sweeps its light bars upward, so a gate reads as
/// live and is easy to pick out from the surrounding wall.
fn animate_parent_gate(
    time: Res<Time>,
    assets: Res<LightcycleAssets>,
    state: Res<LightcycleState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bars: Query<(&GateScanBar, &mut Transform)>,
) {
    let elapsed = time.elapsed_secs();
    let document = state.run.as_ref().is_some_and(ActiveRun::is_document);
    let (handle, dim, bright) = if document {
        (
            &assets.document_folio_material,
            config::DOCUMENT_FOLIO_DIM_COLOR,
            config::DOCUMENT_FOLIO_COLOR,
        )
    } else {
        (
            &assets.portal_material,
            config::LIGHTCYCLE_PORTAL_DIM_COLOR,
            config::LIGHTCYCLE_PORTAL_COLOR,
        )
    };

    if let Some(mut material) = materials.get_mut(handle) {
        material.base_color = dim.mix(&bright, gate_pulse(elapsed));
    }

    for (bar, mut transform) in &mut bars {
        transform.translation.y = gate_bar_height(bar, elapsed);
    }
}

fn animate_city_beacons(time: Res<Time>, mut beacons: Query<(&CityBeacon, &mut Transform)>) {
    let elapsed = time.elapsed_secs();
    for (beacon, mut transform) in &mut beacons {
        let wave = (elapsed * 2.4 + beacon.phase * std::f32::consts::TAU).sin();
        transform.translation.y = beacon.base_height + wave * 0.18;
        transform.scale = Vec3::splat(0.2 + (wave * 0.5 + 0.5) * 0.08);
        transform.rotate_y(0.018);
    }
}

fn update_document_focus(
    mut state: ResMut<LightcycleState>,
    mut marker: Query<&mut Transform, With<DocumentFocusMarker>>,
) {
    let Some(run) = state.run.as_mut() else {
        return;
    };
    let RunEnvironment::Document {
        layout,
        focused_block,
        ..
    } = &mut run.environment
    else {
        return;
    };
    *focused_block = layout.focused_block(run.sim.cell);
    let Some(index) = *focused_block else {
        return;
    };
    let landmark = layout.blocks[index].landmark;
    if let Ok(mut transform) = marker.single_mut() {
        transform.translation = config::ground_position(landmark.0, landmark.1) + Vec3::Y * 0.08;
    }
}

#[allow(clippy::type_complexity)]
fn reset_on_directory_loaded(
    mut loaded: MessageReader<DirectoryLoaded>,
    mode: Res<InteractionMode>,
    mut state: ResMut<LightcycleState>,
    assets: Res<LightcycleAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
    old_lightcycle_entities: Query<Entity, Or<(With<LightcycleSceneRoot>, With<TrailSceneRoot>)>>,
) {
    if *mode != InteractionMode::Lightcycle {
        return;
    }

    for event in loaded.read() {
        despawn_lightcycle_entities(&mut commands, &old_lightcycle_entities);

        let run = build_active_run(&event.path, event.contents.nodes.clone());
        spawn_run_entities(&mut commands, &assets, &mut meshes, &run);

        state.clock = 0.0;
        state.crash_fx = None;
        state.entry_fx = None;
        state.restore_directory = false;
        state.run = Some(run);
    }
}

fn start_document_loads(
    mut requests: MessageReader<DocumentRequested>,
    mut documents: ResMut<DocumentLoadState>,
) {
    for request in requests.read() {
        let generation = documents.next_generation();
        documents.begin_load(generation, request.path.clone());
    }
}

fn poll_document_loads(
    mut documents: ResMut<DocumentLoadState>,
    mut loaded: MessageWriter<DocumentLoaded>,
    mut failed: MessageWriter<DocumentLoadFailed>,
) {
    while let Some(result) = documents.poll() {
        if result.generation != documents.generation {
            continue;
        }
        match result.result {
            Ok(bytes) => {
                loaded.write(DocumentLoaded {
                    path: result.path,
                    bytes,
                });
            }
            Err(message) => {
                failed.write(DocumentLoadFailed {
                    path: result.path,
                    message,
                });
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn reset_on_document_loaded(
    mut loaded: MessageReader<DocumentLoaded>,
    mode: Res<InteractionMode>,
    mut state: ResMut<LightcycleState>,
    assets: Res<LightcycleAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
    old_lightcycle_entities: Query<Entity, Or<(With<LightcycleSceneRoot>, With<TrailSceneRoot>)>>,
) {
    if *mode != InteractionMode::Lightcycle {
        return;
    }

    for event in loaded.read() {
        despawn_lightcycle_entities(&mut commands, &old_lightcycle_entities);
        let run = build_document_run(&event.path, &event.bytes);
        spawn_run_entities(&mut commands, &assets, &mut meshes, &run);
        state.clock = 0.0;
        state.crash_fx = None;
        state.entry_fx = None;
        state.restore_directory = false;
        state.run = Some(run);
    }
}

fn apply_load_failure(
    mut failed: MessageReader<DirectoryLoadFailed>,
    mut state: ResMut<LightcycleState>,
    mut commands: Commands,
    effect_entities: Query<Entity, With<EntryTransportEntity>>,
) {
    if failed.read().next().is_none() {
        return;
    }

    if let Some(run) = state.run.as_mut()
        && run.sim.phase == RunPhase::EnteringDir
    {
        run.sim.pending_request = None;
        run.entering_label = None;
        run.sim.phase = RunPhase::Running;
    }
    state.entry_fx = None;
    for entity in &effect_entities {
        commands.entity(entity).despawn();
    }
}

fn apply_document_load_failure(
    mut failed: MessageReader<DocumentLoadFailed>,
    mut documents: ResMut<DocumentLoadState>,
    mut state: ResMut<LightcycleState>,
) {
    let Some(event) = failed.read().next() else {
        return;
    };
    documents.last_error = Some(format!("{}: {}", event.path.display(), event.message));
    if let Some(run) = state.run.as_mut() {
        run.sim.pending_request = None;
        run.entering_label = None;
        run.sim.phase = RunPhase::Running;
        run.crash_label = Some(format!("could not open {}", event.path.display()));
    }
}

fn start_source_loads(
    mut requests: MessageReader<SourceRequested>,
    mut sources: ResMut<SourceLoadState>,
) {
    for request in requests.read() {
        let generation = sources.next_generation();
        sources.begin_load(generation, request.path.clone());
    }
}

fn poll_source_loads(
    mut sources: ResMut<SourceLoadState>,
    mut loaded: MessageWriter<SourceLoaded>,
    mut failed: MessageWriter<SourceLoadFailed>,
) {
    while let Some(result) = sources.poll() {
        if result.generation != sources.generation {
            continue;
        }
        match result.result {
            Ok(bytes) => {
                loaded.write(SourceLoaded {
                    path: result.path,
                    bytes,
                });
            }
            Err(message) => {
                failed.write(SourceLoadFailed {
                    path: result.path,
                    message,
                });
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn reset_on_source_loaded(
    mut loaded: MessageReader<SourceLoaded>,
    mode: Res<InteractionMode>,
    mut state: ResMut<LightcycleState>,
    assets: Res<LightcycleAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
    old_lightcycle_entities: Query<Entity, Or<(With<LightcycleSceneRoot>, With<TrailSceneRoot>)>>,
) {
    if *mode != InteractionMode::Lightcycle {
        return;
    }

    for event in loaded.read() {
        let Some(language) = SourceLanguage::from_path(&event.path) else {
            continue;
        };
        despawn_lightcycle_entities(&mut commands, &old_lightcycle_entities);
        let run = build_source_run(&event.path, language, &event.bytes);
        spawn_run_entities(&mut commands, &assets, &mut meshes, &run);
        state.clock = 0.0;
        state.crash_fx = None;
        state.entry_fx = None;
        state.restore_directory = false;
        state.run = Some(run);
    }
}

fn apply_source_load_failure(
    mut failed: MessageReader<SourceLoadFailed>,
    mut sources: ResMut<SourceLoadState>,
    mut state: ResMut<LightcycleState>,
) {
    let Some(event) = failed.read().next() else {
        return;
    };
    sources.last_error = Some(format!("{}: {}", event.path.display(), event.message));
    if let Some(run) = state.run.as_mut() {
        run.sim.pending_request = None;
        run.entering_label = None;
        run.sim.phase = RunPhase::Running;
        run.crash_label = Some(format!("could not open {}", event.path.display()));
    }
}

#[allow(clippy::too_many_arguments)]
fn read_lightcycle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    transition: Res<ModeTransition>,
    mut state: ResMut<LightcycleState>,
    mut navigator: ResMut<NavigatorResource>,
    mut requests: MessageWriter<DirectoryRequested>,
    mut effects: MessageWriter<MusicSfx>,
) {
    // Riding controls belong to the run, not to the flight arriving at it.
    if transition.is_active() {
        state.slow_motion = false;
        return;
    }

    let left = keys.just_pressed(KeyCode::KeyA) || keys.just_pressed(KeyCode::ArrowLeft);
    let right = keys.just_pressed(KeyCode::KeyD) || keys.just_pressed(KeyCode::ArrowRight);
    let restart = keys.just_pressed(KeyCode::KeyR);
    let go_up = keys.just_pressed(KeyCode::KeyU) || keys.just_pressed(KeyCode::Minus);
    let throw = keys.just_pressed(KeyCode::Space) || mouse.just_pressed(MouseButton::Left);
    let recall = keys.just_pressed(KeyCode::KeyQ);
    // Stealth walks on WASD: `steer` is the held east/west axis, so only the
    // other pair is needed here.
    let move_z = i32::from(keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown))
        - i32::from(keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp));
    // Hop inputs for the arcade block: one cell per tap, like Frogger and
    // Q*bert need.
    let hop_z = i32::from(keys.just_pressed(KeyCode::KeyW) || keys.just_pressed(KeyCode::ArrowUp))
        - i32::from(keys.just_pressed(KeyCode::KeyS) || keys.just_pressed(KeyCode::ArrowDown));
    // Steering is continuous: the asteroid field pivots while the key is held.
    let steer = i32::from(keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight))
        - i32::from(keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft));
    // The surfer's throttle is always open; boost is held, on the same keys
    // that would otherwise walk or jump.
    let boost = keys.pressed(KeyCode::Space)
        || keys.pressed(KeyCode::KeyW)
        || keys.pressed(KeyCode::ArrowUp);
    // Bullet time: hold Shift to slow the ring while lining up a turn or shot.
    state.slow_motion = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    let entering_tower = state.entry_fx.is_some();
    let Some(mut run) = state.run.take() else {
        return;
    };

    // The field is only a turret fight while the rocks are live; after it is won
    // or lost the bike is handed back and drives normally.
    let field_active = run.asteroid_field_active();

    // Which runs steer the shared bike grid this frame? Directories and
    // documents always do, disc wars and snake always do, the field only once it
    // has handed the bike back, and the off-grid games never.
    let drives_grid = match run.source_game() {
        Some(SourceGame::DiscWars | SourceGame::Snake) | None => true,
        Some(SourceGame::Asteroids) => !field_active,
        Some(
            SourceGame::Platformer
            | SourceGame::Breaker
            | SourceGame::Stealth
            | SourceGame::RiverSurfer
            | SourceGame::Galaga
            | SourceGame::PacMan
            | SourceGame::Columns
            | SourceGame::Tetris
            | SourceGame::Frogger
            | SourceGame::Qbert
            | SourceGame::Bomberman
            | SourceGame::Plinko,
        ) => false,
    };
    if drives_grid && run.sim.phase == RunPhase::Running && (left || right) {
        run.sim.queue_turn_input(left, right);
        effects.write(MusicSfx::Turn);
    }

    if run.is_source() && run.sim.phase == RunPhase::Running {
        if field_active {
            // Pivot and shoot. `set_turn` persists for the fixed sub-steps that
            // follow this frame.
            let mut fired = false;
            if let Some(field) = run.source_asteroids_mut() {
                field.set_turn(steer as f32);
                if throw {
                    fired = field.fire();
                }
            }
            if fired {
                effects.write(MusicSfx::Zap);
            }
        } else if let Some(level) = run.source_platformer_mut() {
            // Held to run, tapped to jump.
            level.set_input(steer as f32, throw);
        } else if let Some(level) = run.source_breaker_mut() {
            // Held to slide the bike along the bottom, tapped to serve.
            level.set_input(steer as f32, throw);
        } else if let Some(room) = run.source_stealth_mut() {
            // Hold a direction to keep walking it; let go to stop.
            room.set_input(steer, move_z);
        } else if let Some(surfer) = run.source_surfer_mut() {
            // Steer the hoverbike; the throttle is always open and boost is held.
            surfer.set_input(steer as f32, boost);
        } else if let Some(sim) = run.source_galaga_mut() {
            // Slide along the bottom; the -Z camera mirrors X, so negate steer.
            sim.set_input(-steer as f32, throw);
        } else if let Some(sim) = run.source_pacman_mut() {
            // Hold a direction to keep walking the corridor.
            sim.set_input(steer, move_z);
        } else if let Some(sim) = run.source_columns_mut() {
            // A/D slides the piece, W rotates, Space hard-drops.
            sim.set_input(i32::from(right) - i32::from(left), hop_z > 0, throw);
        } else if let Some(sim) = run.source_tetris_mut() {
            // A/D slides, W rotates, S soft-drops, Space hard-drops.
            sim.set_input(
                i32::from(right) - i32::from(left),
                hop_z > 0,
                move_z > 0,
                throw,
            );
        } else if let Some(sim) = run.source_frogger_mut() {
            // One hop per keypress in any of the four directions.
            sim.hop(i32::from(right) - i32::from(left), hop_z);
        } else if let Some(sim) = run.source_qbert_mut() {
            // Diagonal hops: A/D/W/S each map to a pyramid direction. The -Z
            // camera mirrors X, so swap the east/west edges.
            sim.hop(i32::from(left) - i32::from(right), hop_z);
        } else if let Some(sim) = run.source_bomberman_mut() {
            // Walk on the room grid and plant bombs with Space or click.
            if (steer != 0 || move_z != 0) && sim.phase == BomberPhase::Walking {
                sim.step(steer, move_z);
            }
            if throw {
                sim.plant();
            }
        } else if let Some(sim) = run.source_plinko_mut() {
            // Slide the rail and drop balls with Space or click.
            let slide = sim.aim + steer as f32 * 6.0;
            sim.set_aim(slide);
            if throw {
                sim.drop_ball();
            }
        } else if run.source_game() == Some(SourceGame::DiscWars) {
            // Disc wars: throw and recall. The cycle's movement is unchanged.
            let snapshot = PlayerSnapshot {
                cell: run.sim.cell,
                heading: run.sim.heading,
                running: true,
            };
            if throw {
                let mut events = DiscEvents::default();
                // Aimed at the opponent, so riding and aiming stay separate.
                if let RunEnvironment::Source { sim, .. } = &mut run.environment
                    && let Some(disc) = sim.as_disc_mut()
                {
                    disc.throw_player(snapshot, &run.arena, &mut events);
                }
                if events.player_threw {
                    effects.write(MusicSfx::Beam);
                }
            }
            if recall {
                let mut events = DiscEvents::default();
                if let Some(disc) = run.source_disc_mut() {
                    disc.recall_player(&mut events);
                }
                if events.player_recalled {
                    effects.write(MusicSfx::Turn);
                }
            }
        }
    }

    if restart {
        restart_run(&mut run);
        state.clock = 0.0;
        state.crash_fx = None;
        state.entry_fx = None;
    }

    if go_up && !entering_tower {
        effects.write(MusicSfx::Portal);
        if run.is_document() || run.is_source() {
            state.restore_directory = true;
        } else if let Some(parent) = navigator.0.begin_go_to_parent() {
            run.sim.pause_for_directory_change();
            run.entering_label = Some("parent directory".to_string());
            run.crash_label = None;
            requests.write(DirectoryRequested { path: parent });
        }
    }

    state.run = Some(run);
}

fn restart_run(run: &mut ActiveRun) {
    match &mut run.environment {
        RunEnvironment::Directory { cells, .. } => {
            let cells = cells.clone();
            run.sim = spawn_sim(&run.arena, &cells);
        }
        RunEnvironment::Document { .. } => {
            run.sim = spawn_sim(&run.arena, &HashMap::new());
        }
        RunEnvironment::Source { layout, sim, .. } => {
            let spawn = layout.player_spawn;
            let heading = layout.player_spawn_heading;
            let seed = layout.seed;
            let center = ring_center_world(layout);
            let radius = ring_radius_world(layout);
            let food_cells = ring_food_cells(&run.arena);
            run.sim = LightcycleSim::start(spawn, heading);
            match sim {
                SourceSim::DiscWars(disc) => *disc = DiscSim::new(layout),
                SourceSim::Asteroids(field) => {
                    let mut fresh = AsteroidsSim::new(seed, center, radius);
                    fresh.angle = heading_facing(heading);
                    **field = fresh;
                }
                SourceSim::Snake(snake) => {
                    *snake = SnakeSim::new(seed, spawn, &food_cells, config::SNAKE_FOOD_TARGET);
                }
                // Each sim re-rolls itself from its own stored seed, so a
                // restart lays out exactly the same level.
                SourceSim::Platformer(level) => level.restart(),
                SourceSim::Breaker(level) => level.restart(),
                SourceSim::Stealth(room) => room.restart(),
                SourceSim::Surfer(surfer) => surfer.restart(),
                SourceSim::Galaga(sim) => sim.restart(),
                SourceSim::PacMan(sim) => sim.restart(),
                SourceSim::Columns(sim) => sim.restart(),
                SourceSim::Tetris(sim) => sim.restart(),
                SourceSim::Frogger(sim) => sim.restart(),
                SourceSim::Qbert(sim) => sim.restart(),
                SourceSim::Bomberman(sim) => sim.restart(),
                SourceSim::Plinko(sim) => sim.restart(),
            }
        }
    }
    run.crash_label = None;
    run.entering_label = None;
}

#[allow(clippy::type_complexity)]
fn restore_directory_arena(
    mut state: ResMut<LightcycleState>,
    navigator: Res<NavigatorResource>,
    assets: Res<LightcycleAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
    old_lightcycle_entities: Query<Entity, Or<(With<LightcycleSceneRoot>, With<TrailSceneRoot>)>>,
) {
    if !state.restore_directory {
        return;
    }
    state.restore_directory = false;
    despawn_lightcycle_entities(&mut commands, &old_lightcycle_entities);
    let run = build_active_run(&navigator.0.current_path, navigator.0.entries.clone());
    spawn_run_entities(&mut commands, &assets, &mut meshes, &run);
    state.clock = 0.0;
    state.crash_fx = None;
    state.entry_fx = None;
    state.run = Some(run);
}

#[allow(clippy::too_many_arguments)]
fn step_lightcycle(
    time: Res<Time>,
    transition: Res<ModeTransition>,
    mut state: ResMut<LightcycleState>,
    mut navigator: ResMut<NavigatorResource>,
    mut requests: MessageWriter<DirectoryRequested>,
    mut documents: MessageWriter<DocumentRequested>,
    mut sources: MessageWriter<SourceRequested>,
    mut effects: MessageWriter<MusicSfx>,
) {
    // The arena exists from the top of the climb onward, but the camera is
    // still diving toward it. Hold the cycle on its spawn cell until it lands,
    // so the run starts from the shot the player is given rather than partway
    // down the first street.
    if transition.is_active() {
        state.clock = 0.0;
        return;
    }

    let Some(mut run) = state.run.take() else {
        return;
    };

    // A ring keeps ticking even while the player is crashed or waiting between
    // rounds, so the fight can pay out its respawn. Other arenas hold still.
    let source_run = run.is_source();
    if run.sim.phase != RunPhase::Running && !source_run {
        state.run = Some(run);
        return;
    }

    state.clock += time.delta_secs();
    let max_catch_up = config::LIGHTCYCLE_FIXED_STEP * config::LIGHTCYCLE_MAX_SUBSTEPS as f32;
    if state.clock > max_catch_up {
        state.clock = max_catch_up;
    }

    let fixed_step = config::LIGHTCYCLE_FIXED_STEP;
    // Bullet time stretches the simulated step without changing the real-time
    // cadence, so the bike, its disc, and the opponent all slow together.
    let step = if source_run && state.slow_motion {
        fixed_step * config::DISC_BULLET_TIME_SCALE
    } else {
        fixed_step
    };
    let mut substeps = 0;

    while state.clock >= fixed_step && substeps < config::LIGHTCYCLE_MAX_SUBSTEPS {
        state.clock -= fixed_step;
        substeps += 1;

        let outcome = {
            let arena = &run.arena;
            let sim = &mut run.sim;
            match &run.environment {
                RunEnvironment::Directory { nodes, cells } => {
                    sim.advance(step * config::LIGHTCYCLE_CELLS_PER_SEC, |next, sim| {
                        classify_next_content(
                            next,
                            arena,
                            sim,
                            cells,
                            |index| nodes[index].is_dir,
                            |index| nodes[index].is_markdown(),
                            |index| nodes[index].is_source(),
                        )
                    })
                }
                RunEnvironment::Document { .. } => {
                    sim.advance(step * config::LIGHTCYCLE_CELLS_PER_SEC, |next, sim| {
                        classify_next_content(
                            next,
                            arena,
                            sim,
                            &HashMap::new(),
                            |_| false,
                            |_| false,
                            |_| false,
                        )
                    })
                }
                RunEnvironment::Source { sim: source, .. } => match source {
                    // Parked while the rocks are live: nothing to advance, and
                    // the field itself is stepped after the loop.
                    SourceSim::Asteroids(field) if field.is_active() => StepOutcome::Moved,
                    // Once the field is decided the cycle is handed back, so it
                    // drives again and can ride out through the gate.
                    SourceSim::Asteroids(_) => {
                        sim.advance(step * config::LIGHTCYCLE_CELLS_PER_SEC, |next, state| {
                            classify_next_content(
                                next,
                                arena,
                                state,
                                &HashMap::new(),
                                |_| false,
                                |_| false,
                                |_| false,
                            )
                        })
                    }
                    // Snake drives the ordinary grid, but the exit is a solid
                    // wall until enough power-ups have been eaten. The tail is
                    // capped after every step so it stays finite.
                    SourceSim::Snake(snake) => {
                        let locked = !snake.exit_open;
                        let outcome =
                            sim.advance(step * config::LIGHTCYCLE_CELLS_PER_SEC, |next, state| {
                                if locked && is_ring_gate(arena, next) {
                                    return CellContent::Wall;
                                }
                                classify_next_content(
                                    next,
                                    arena,
                                    state,
                                    &HashMap::new(),
                                    |_| false,
                                    |_| false,
                                    |_| false,
                                )
                            });
                        snake.trim_tail(sim);
                        outcome
                    }
                    // The off-grid games drive their own sims, so the shared
                    // grid has nothing to advance.
                    SourceSim::Platformer(_)
                    | SourceSim::Breaker(_)
                    | SourceSim::Stealth(_)
                    | SourceSim::Surfer(_)
                    | SourceSim::Galaga(_)
                    | SourceSim::PacMan(_)
                    | SourceSim::Columns(_)
                    | SourceSim::Tetris(_)
                    | SourceSim::Frogger(_)
                    | SourceSim::Qbert(_)
                    | SourceSim::Bomberman(_)
                    | SourceSim::Plinko(_) => StepOutcome::Moved,
                    SourceSim::DiscWars(disc) => {
                        // The opponent's body and its live disc are lethal cells
                        // in the same grid model the cycle already uses.
                        let opponent = disc.opponent_cell();
                        let opponent_disc = disc.opponent_disc_cell();
                        sim.advance(step * config::LIGHTCYCLE_CELLS_PER_SEC, |next, state| {
                            if Some(next) == opponent {
                                return CellContent::Opponent;
                            }
                            if Some(next) == opponent_disc {
                                return CellContent::OpponentDisc;
                            }
                            classify_next_content(
                                next,
                                arena,
                                state,
                                &HashMap::new(),
                                |_| false,
                                |_| false,
                                |_| false,
                            )
                        })
                    }
                },
            }
        };

        match outcome {
            StepOutcome::Moved => {}
            StepOutcome::Crashed(reason) => {
                let crash_cell = run.sim.next_cell();
                let label = match reason {
                    CrashReason::File => run
                        .directory_cells()
                        .and_then(|cells| cells.get(&crash_cell).copied())
                        .and_then(|index| {
                            run.directory_nodes()
                                .and_then(|nodes| nodes.get(index))
                                .map(|node| format!("file {}", node.name))
                        })
                        .unwrap_or_else(|| "file".to_string()),
                    CrashReason::Trail => "your trail".to_string(),
                    CrashReason::Opponent => "the recognizer".to_string(),
                    CrashReason::Disc => "a disc".to_string(),
                    CrashReason::Hazard => "a hazard tile".to_string(),
                    // Snake's gate is solid until the exit opens.
                    CrashReason::Wall
                        if run.source_snake().is_some() && is_ring_gate(&run.arena, crash_cell) =>
                    {
                        "the locked exit".to_string()
                    }
                    CrashReason::Wall if run.arena.street_walls.contains(&crash_cell) => {
                        if run.is_document() {
                            "paragraph".to_string()
                        } else if run.is_source() {
                            "ring wall".to_string()
                        } else {
                            "street barrier".to_string()
                        }
                    }
                    CrashReason::Wall => "arena wall".to_string(),
                };
                run.crash_label = Some(label);
                run.entering_label = None;
            }
            StepOutcome::EnteringDir(index) => {
                let details = run
                    .directory_nodes()
                    .and_then(|nodes| nodes.get(index))
                    .map(|node| (node.name.clone(), node.path.clone()));
                if let Some((name, path)) = details {
                    run.entering_label = Some(name);
                    run.crash_label = None;
                    effects.write(MusicSfx::Beam);
                    state.entry_fx = Some(crate::lightcycle::EntryFx::new(
                        path,
                        config::LIGHTCYCLE_ENTRY_FX_DURATION,
                    ));
                } else {
                    run.sim.phase = RunPhase::Crashed;
                    run.crash_label = Some("missing directory".to_string());
                }
            }
            StepOutcome::EnteringDocument(index) => {
                let details = run
                    .directory_nodes()
                    .and_then(|nodes| nodes.get(index))
                    .map(|node| (node.name.clone(), node.path.clone()));
                if let Some((name, path)) = details {
                    run.entering_label = Some(name);
                    run.crash_label = None;
                    effects.write(MusicSfx::Beam);
                    documents.write(DocumentRequested { path });
                } else {
                    run.sim.phase = RunPhase::Running;
                    run.crash_label = Some("missing document".to_string());
                }
            }
            StepOutcome::EnteringSource(index) => {
                let details = run
                    .directory_nodes()
                    .and_then(|nodes| nodes.get(index))
                    .map(|node| (node.name.clone(), node.path.clone()));
                if let Some((name, path)) = details {
                    run.entering_label = Some(name);
                    run.crash_label = None;
                    effects.write(MusicSfx::Beam);
                    sources.write(SourceRequested { path });
                } else {
                    run.sim.phase = RunPhase::Running;
                    run.crash_label = Some("missing source".to_string());
                }
            }
            StepOutcome::GoToParent => {
                if let Some(parent) = navigator.0.begin_go_to_parent() {
                    run.entering_label = Some("parent directory".to_string());
                    run.crash_label = None;
                    effects.write(MusicSfx::Portal);
                    requests.write(DirectoryRequested { path: parent });
                } else {
                    run.sim.phase = RunPhase::Crashed;
                    run.crash_label = Some("arena wall".to_string());
                }
            }
            StepOutcome::CloseDocument => {
                state.restore_directory = true;
            }
        }

        if source_run {
            let cleared = match run.source_game() {
                Some(SourceGame::Asteroids) => {
                    step_asteroid_field(&mut run, step, &mut effects);
                    false
                }
                Some(SourceGame::Snake) => {
                    step_snake(&mut run, &mut effects);
                    false
                }
                Some(SourceGame::Platformer) => step_platformer(&mut run, step, &mut effects),
                Some(SourceGame::Breaker) => step_breaker(&mut run, step, &mut effects),
                Some(SourceGame::Stealth) => step_stealth(&mut run, step, &mut effects),
                Some(SourceGame::RiverSurfer) => step_surfer(&mut run, step, &mut effects),
                Some(SourceGame::Galaga) => step_galaga(&mut run, step, &mut effects),
                Some(SourceGame::PacMan) => step_pacman(&mut run, step, &mut effects),
                Some(SourceGame::Columns) => step_columns(&mut run, step, &mut effects),
                Some(SourceGame::Tetris) => step_tetris(&mut run, step, &mut effects),
                Some(SourceGame::Frogger) => step_frogger(&mut run, step, &mut effects),
                Some(SourceGame::Qbert) => step_qbert(&mut run, step, &mut effects),
                Some(SourceGame::Bomberman) => step_bomberman(&mut run, step, &mut effects),
                Some(SourceGame::Plinko) => step_plinko(&mut run, step, &mut effects),
                Some(SourceGame::DiscWars) => {
                    step_disc_fight(&mut run, step, &mut effects);
                    false
                }
                None => false,
            };
            // Clearing a level is this run's version of riding out the gate.
            if cleared {
                state.restore_directory = true;
            }
        }

        if run.sim.phase == RunPhase::Crashed && state.crash_fx.is_none() {
            state.crash_fx = Some(crate::lightcycle::CrashFx::new(
                config::LIGHTCYCLE_CRASH_FX_DURATION,
            ));
            effects.write(MusicSfx::Crash);
        }

        if run.sim.phase != RunPhase::Running && !source_run {
            break;
        }
    }

    state.run = Some(run);
}

/// Steps the asteroid field and routes its feedback into sound and labels.
fn step_asteroid_field(run: &mut ActiveRun, dt: f32, effects: &mut MessageWriter<MusicSfx>) {
    let events = {
        let Some(field) = run.source_asteroids_mut() else {
            return;
        };
        field.update(dt)
    };

    for _ in 0..events.destroyed {
        effects.write(MusicSfx::Portal);
    }
    if events.lost_life {
        effects.write(MusicSfx::Crash);
    }
    if events.cleared {
        run.crash_label = None;
        run.entering_label = None;
        effects.write(MusicSfx::Victory);
    }

    let lost = run
        .source_asteroids()
        .is_some_and(|field| field.phase == AsteroidsPhase::Lost);
    if lost {
        run.crash_label = Some("the rock field".to_string());
        run.entering_label = None;
    }

    if events.ended {
        // Hand the wheel back on the frame the field is decided: the bike keeps
        // the facing the player was holding and can drive to the gate to leave.
        let facing = run.source_asteroids_mut().map(|field| {
            field.set_turn(0.0);
            nearest_heading(field.angle)
        });
        if let Some(facing) = facing {
            run.sim.heading = facing;
        }
    }
}

/// Steps a snake run: collects power-ups, opens the exit and cues the death.
fn step_snake(run: &mut ActiveRun, effects: &mut MessageWriter<MusicSfx>) {
    let cell = run.sim.cell;
    let Some(events) = run.source_snake_mut().map(|snake| snake.eat(cell)) else {
        return;
    };
    if events.ate {
        effects.write(MusicSfx::Portal);
    }
    if events.opened_exit {
        effects.write(MusicSfx::Beam);
    }

    // The base grid crash ends the run; the rider keeps their crash FX, and the
    // snake only adds the sound once.
    if run.sim.phase == RunPhase::Crashed
        && run
            .source_snake_mut()
            .is_some_and(|snake| snake.note_crash())
    {
        effects.write(MusicSfx::Crash);
    }
}

/// Steps a platformer level. Returns `true` on the frame the exit is reached,
/// which hands the run back to the directory it came from.
fn step_platformer(run: &mut ActiveRun, dt: f32, effects: &mut MessageWriter<MusicSfx>) -> bool {
    let (events, fell) = {
        let Some(level) = run.source_platformer_mut() else {
            return false;
        };
        let events = level.update(dt);
        (events, level.phase == PlatformerPhase::Lost)
    };
    if events.jumped {
        effects.write(MusicSfx::Zap);
    }
    if events.won {
        effects.write(MusicSfx::Victory);
    }
    // A fall ends the run through the shared crash path, so the burst, the
    // shake, the label and `R` all behave like any other crash.
    if fell {
        crash_source(run, "the void under the level", effects);
    }
    events.won
}

/// Steps a breaker court. Returns `true` when the wall is cleared, which hands
/// the run back to the directory.
fn step_breaker(run: &mut ActiveRun, dt: f32, effects: &mut MessageWriter<MusicSfx>) -> bool {
    let (events, missed) = {
        let Some(level) = run.source_breaker_mut() else {
            return false;
        };
        let events = level.update(dt);
        (events, level.phase == BreakerPhase::Missed)
    };
    if events.launched {
        effects.write(MusicSfx::Beam);
    }
    if events.bounced_off_paddle {
        effects.write(MusicSfx::Turn);
    }
    for _ in 0..events.broke_bricks {
        effects.write(MusicSfx::Portal);
    }
    if events.cleared {
        effects.write(MusicSfx::Victory);
    }
    // The wall below the bike is the one that ends it.
    if missed {
        crash_source(run, "the ball past the bike", effects);
    }
    events.cleared
}

/// Steps a stealth run. Returns `true` when the character reaches the door.
fn step_stealth(run: &mut ActiveRun, dt: f32, effects: &mut MessageWriter<MusicSfx>) -> bool {
    let (events, caught) = {
        let Some(room) = run.source_stealth_mut() else {
            return false;
        };
        let events = room.update(dt);
        (events, room.phase == StealthPhase::Caught)
    };
    if events.spotted {
        effects.write(MusicSfx::Zap);
    }
    if events.escaped {
        effects.write(MusicSfx::Victory);
    }
    if caught {
        crash_source(run, "a patrol", effects);
    }
    events.escaped
}

/// Steps a river surfer run. Returns `true` when the bike crosses the finish.
fn step_surfer(run: &mut ActiveRun, dt: f32, effects: &mut MessageWriter<MusicSfx>) -> bool {
    let (events, crashed) = {
        let Some(surfer) = run.source_surfer_mut() else {
            return false;
        };
        let events = surfer.update(dt);
        (events, surfer.phase == SurferPhase::Crashed)
    };
    if events.boosted {
        effects.write(MusicSfx::Beam);
    }
    if events.finished {
        effects.write(MusicSfx::Victory);
    }
    if crashed {
        let label = if events.banked {
            "the riverbank"
        } else {
            "a rock in the river"
        };
        crash_source(run, label, effects);
    }
    events.finished
}

/// Steps a Galaga field. Returns `true` when the formation is cleared, which
/// hands the run back to the directory.
fn step_galaga(run: &mut ActiveRun, dt: f32, effects: &mut MessageWriter<MusicSfx>) -> bool {
    let (events, lost) = {
        let Some(sim) = run.source_galaga_mut() else {
            return false;
        };
        let events = sim.update(dt);
        (events, sim.phase == GalagaPhase::Lost)
    };
    if events.fired {
        effects.write(MusicSfx::Beam);
    }
    for _ in 0..events.killed {
        effects.write(MusicSfx::Portal);
    }
    if events.lost_life {
        effects.write(MusicSfx::Crash);
    }
    if events.cleared {
        effects.write(MusicSfx::Victory);
    }
    if lost {
        let label = if events.overrun {
            "the swarm reached the cycle"
        } else {
            "the swarm"
        };
        crash_source(run, label, effects);
    }
    events.cleared
}

/// Steps a Pac-Man maze. Returns `true` when every dot is eaten.
fn step_pacman(run: &mut ActiveRun, dt: f32, effects: &mut MessageWriter<MusicSfx>) -> bool {
    let (events, caught) = {
        let Some(sim) = run.source_pacman_mut() else {
            return false;
        };
        let events = sim.update(dt);
        (events, sim.phase == PacPhase::Caught)
    };
    if events.dots > 0 {
        effects.write(MusicSfx::Portal);
    }
    if events.lost_life {
        effects.write(MusicSfx::Crash);
    }
    if events.cleared {
        effects.write(MusicSfx::Victory);
    }
    if caught {
        crash_source(run, "a ghost in the maze", effects);
    }
    events.cleared
}

/// Steps a Columns well. Returns `true` when the well is empty.
fn step_columns(run: &mut ActiveRun, dt: f32, effects: &mut MessageWriter<MusicSfx>) -> bool {
    let (events, lost) = {
        let Some(sim) = run.source_columns_mut() else {
            return false;
        };
        let events = sim.update(dt);
        (events, sim.phase == ColumnsPhase::Lost)
    };
    if events.matched > 0 {
        effects.write(MusicSfx::Portal);
    }
    if events.landed {
        effects.write(MusicSfx::Beam);
    }
    if events.cleared {
        effects.write(MusicSfx::Victory);
    }
    if lost {
        crash_source(run, "the gem well", effects);
    }
    events.cleared
}

/// Steps a Tetris board. Returns `true` once the line target is met.
fn step_tetris(run: &mut ActiveRun, dt: f32, effects: &mut MessageWriter<MusicSfx>) -> bool {
    let (events, lost) = {
        let Some(sim) = run.source_tetris_mut() else {
            return false;
        };
        let events = sim.update(dt);
        (events, sim.phase == TetrisPhase::Lost)
    };
    if events.lines > 0 {
        effects.write(MusicSfx::Portal);
    }
    if events.locked {
        effects.write(MusicSfx::Beam);
    }
    if events.cleared {
        effects.write(MusicSfx::Victory);
    }
    if lost {
        crash_source(run, "the stack of indentation", effects);
    }
    events.cleared
}

/// Steps a Frogger highway. Returns `true` when the far row is reached.
fn step_frogger(run: &mut ActiveRun, dt: f32, effects: &mut MessageWriter<MusicSfx>) -> bool {
    let (events, splatted) = {
        let Some(sim) = run.source_frogger_mut() else {
            return false;
        };
        let events = sim.update(dt);
        (events, sim.phase == FroggerPhase::Splatted)
    };
    if events.splatted {
        effects.write(MusicSfx::Crash);
    }
    if events.cleared {
        effects.write(MusicSfx::Victory);
    }
    if splatted {
        crash_source(run, "the async highway", effects);
    }
    events.cleared
}

/// Steps a Q*bert pyramid. Returns `true` once every cube is lit.
fn step_qbert(run: &mut ActiveRun, dt: f32, effects: &mut MessageWriter<MusicSfx>) -> bool {
    let (events, lost) = {
        let Some(sim) = run.source_qbert_mut() else {
            return false;
        };
        let events = sim.update(dt);
        (events, sim.phase == QbertPhase::Lost)
    };
    if events.lost_life {
        effects.write(MusicSfx::Crash);
    }
    if events.cleared {
        effects.write(MusicSfx::Victory);
    }
    if lost {
        crash_source(run, "the pyramid edge", effects);
    }
    events.cleared
}

/// Steps a Bomberman room. Returns `true` once the exit is reached.
fn step_bomberman(run: &mut ActiveRun, dt: f32, effects: &mut MessageWriter<MusicSfx>) -> bool {
    let (events, lost) = {
        let Some(sim) = run.source_bomberman_mut() else {
            return false;
        };
        let events = sim.update(dt);
        (events, sim.phase == BomberPhase::Lost)
    };
    if events.crates > 0 {
        effects.write(MusicSfx::Portal);
    }
    if events.lost_life {
        effects.write(MusicSfx::Crash);
    }
    if events.cleared {
        effects.write(MusicSfx::Victory);
    }
    if lost {
        crash_source(run, "your own bomb", effects);
    }
    events.cleared
}

/// Steps a Plinko board. Returns `true` when the rack beats the target.
fn step_plinko(run: &mut ActiveRun, dt: f32, effects: &mut MessageWriter<MusicSfx>) -> bool {
    let (events, lost) = {
        let Some(sim) = run.source_plinko_mut() else {
            return false;
        };
        let events = sim.update(dt);
        (events, sim.phase == PlinkoPhase::Lost)
    };
    if events.scored > 0 {
        effects.write(MusicSfx::Beam);
    }
    if events.cleared {
        effects.write(MusicSfx::Victory);
    }
    if lost {
        crash_source(run, "the data", effects);
    }
    events.cleared
}

/// Ends a source run through the shared crash path, so the burst, the shake, the
/// label and `R` behave the same as a grid crash.
fn crash_source(run: &mut ActiveRun, label: &str, effects: &mut MessageWriter<MusicSfx>) {
    if run.sim.phase != RunPhase::Running {
        return;
    }
    effects.write(MusicSfx::Crash);
    run.sim.phase = RunPhase::Crashed;
    run.crash_label = Some(label.to_string());
    run.entering_label = None;
}

/// Steps one ring's fight and folds its events back into the shared run.
fn step_disc_fight(run: &mut ActiveRun, dt: f32, effects: &mut MessageWriter<MusicSfx>) {
    let snapshot = PlayerSnapshot {
        cell: run.sim.cell,
        heading: run.sim.heading,
        running: run.sim.phase == RunPhase::Running,
    };
    let events = {
        let arena = &run.arena;
        let RunEnvironment::Source { sim, layout, .. } = &mut run.environment else {
            return;
        };
        let Some(disc) = sim.as_disc_mut() else {
            return;
        };
        disc.update(dt, snapshot, arena, layout)
    };

    // On the final blow the fanfare replaces the crash, so the win lands clean
    // instead of the derezz thud sitting on top of it.
    let match_won = events.match_over == Some(DiscPhase::Won);
    if (events.opponent_hit || events.player_derezz.is_some()) && !match_won {
        effects.write(MusicSfx::Crash);
    }
    if events.shielded || events.player_recalled || events.opponent_threw {
        effects.write(MusicSfx::Turn);
    }
    for _ in &events.collected {
        effects.write(MusicSfx::Portal);
    }

    if let Some(reason) = events.player_derezz
        && run.sim.phase == RunPhase::Running
    {
        run.sim.phase = RunPhase::Crashed;
        run.sim.crash_reason = Some(reason);
        run.crash_label = Some(disc_crash_label(reason));
        run.entering_label = None;
    }

    if let Some((cell, heading)) = events.respawn {
        run.sim = LightcycleSim::start(cell, heading);
        run.crash_label = None;
        run.entering_label = None;
    }

    if let Some(phase) = events.match_over {
        match phase {
            DiscPhase::Lost => {
                if let RunEnvironment::Source { language, .. } = &run.environment {
                    run.crash_label = Some(language.crash_flavor(0).to_string());
                    run.entering_label = None;
                }
            }
            DiscPhase::Won => {
                run.crash_label = None;
                run.entering_label = None;
                effects.write(MusicSfx::Victory);
            }
            DiscPhase::Fighting => {}
        }
    }
}

fn disc_crash_label(reason: CrashReason) -> String {
    match reason {
        CrashReason::Hazard => "a hazard tile".to_string(),
        CrashReason::Disc => "a disc".to_string(),
        CrashReason::Opponent => "the recognizer".to_string(),
        CrashReason::Wall => "ring wall".to_string(),
        CrashReason::Trail => "your trail".to_string(),
        CrashReason::File => "file".to_string(),
    }
}

fn spawn_crash_effect(
    mut state: ResMut<LightcycleState>,
    assets: Res<LightcycleAssets>,
    mut commands: Commands,
) {
    let Some(fx) = state.crash_fx.as_mut() else {
        return;
    };
    if fx.spawned {
        return;
    }
    fx.spawned = true;

    let Some(run) = state.run.as_ref() else {
        return;
    };
    let origin = cycle_world_position(&run.sim) + Vec3::Y * config::LIGHTCYCLE_CYCLE_HEIGHT * 0.5;
    let count = 18;

    for index in 0..count {
        let angle = index as f32 / count as f32 * std::f32::consts::TAU;
        let speed = 5.0 + (index % 5) as f32 * 1.3;
        let horizontal = Vec3::new(angle.cos(), 0.0, angle.sin());
        let velocity = horizontal * speed + Vec3::Y * (4.0 + (index % 4) as f32 * 1.1);
        let initial_scale = 0.18 + (index % 4) as f32 * 0.05;
        let life = 0.55 + (index % 3) as f32 * 0.1;

        commands.spawn((
            LightcycleSceneRoot,
            CrashDebris {
                velocity,
                life,
                max_life: life,
                initial_scale,
            },
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(if index % 3 == 0 {
                assets.trail_material.clone()
            } else {
                assets.crash_material.clone()
            }),
            Transform::from_translation(origin)
                .with_rotation(Quat::from_rotation_y(angle))
                .with_scale(Vec3::splat(initial_scale)),
            Pickable::IGNORE,
        ));
    }
}

fn update_crash_effects(
    time: Res<Time>,
    mut state: ResMut<LightcycleState>,
    mut commands: Commands,
    mut debris: Query<(Entity, &mut Transform, &mut CrashDebris)>,
) {
    let delta = time.delta_secs();
    let gravity = -18.0;

    for (entity, mut transform, mut piece) in &mut debris {
        piece.life -= delta;
        piece.velocity.y += gravity * delta;
        transform.translation += piece.velocity * delta;

        let life_ratio = (piece.life / piece.max_life).max(0.0);
        let scale = piece.initial_scale * life_ratio + 0.02;
        transform.scale = Vec3::splat(scale);

        if piece.life <= 0.0 {
            commands.entity(entity).despawn();
        }
    }

    if let Some(fx) = state.crash_fx.as_mut() {
        fx.timer -= delta;
        if fx.timer <= 0.0 {
            state.crash_fx = None;
        }
    }
}

/// Removes transport geometry when a restart cancels the timeline before a new
/// arena arrives. Normal directory loads remove it with the rest of the old
/// lightcycle scene.
fn cleanup_orphaned_entry_effect(
    state: Res<LightcycleState>,
    mut commands: Commands,
    effects: Query<Entity, With<EntryTransportEntity>>,
) {
    if state.entry_fx.is_some() {
        return;
    }
    for entity in &effects {
        commands.entity(entity).despawn();
    }
}

/// Creates a translucent column and a stack of independent neon rings around
/// the stopped cycle. Navigation is deliberately not started here: the update
/// system below waits until the beam reaches its apex.
fn spawn_entry_effect(
    mut state: ResMut<LightcycleState>,
    assets: Res<LightcycleAssets>,
    mut commands: Commands,
) {
    let Some(fx) = state.entry_fx.as_mut() else {
        return;
    };
    if fx.spawned {
        return;
    }
    fx.spawned = true;

    let Some(run) = state.run.as_ref() else {
        return;
    };
    let origin = cycle_world_position(&run.sim);
    commands.spawn((
        LightcycleSceneRoot,
        EntryTransportEntity,
        EntryBeam,
        Mesh3d(assets.entry_beam_mesh.clone()),
        MeshMaterial3d(assets.entry_beam_material.clone()),
        Transform::from_translation(
            origin + Vec3::Y * (config::LIGHTCYCLE_ENTRY_BEAM_HEIGHT * 0.5),
        )
        .with_scale(Vec3::new(0.02, 1.0, 0.02)),
        Pickable::IGNORE,
    ));

    for index in 0..config::LIGHTCYCLE_ENTRY_HALO_COUNT {
        let phase = index as f32 / config::LIGHTCYCLE_ENTRY_HALO_COUNT as f32;
        commands.spawn((
            LightcycleSceneRoot,
            EntryTransportEntity,
            EntryHalo { phase },
            Mesh3d(assets.entry_halo_mesh.clone()),
            MeshMaterial3d(assets.entry_halo_material.clone()),
            Transform::from_translation(origin),
            Pickable::IGNORE,
        ));
    }
}

/// Brightness/size envelope for the whole transport. The quick rise makes the
/// collision read as a capture; the tail collapses as the old arena disappears.
fn entry_effect_envelope(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    if progress < 0.18 {
        smoothstep(progress / 0.18)
    } else {
        1.0 - smoothstep((progress - 0.18) / 0.82)
    }
}

/// Height and scale of one halo in the repeating upward sweep.
fn entry_halo_pose(progress: f32, phase: f32) -> (f32, f32) {
    let sweep = (progress * 2.0 + phase).fract();
    let height = 0.35 + sweep * config::LIGHTCYCLE_ENTRY_HALO_HEIGHT;
    let ring_envelope = (std::f32::consts::PI * sweep).sin().max(0.0);
    let scale = entry_effect_envelope(progress) * (0.35 + ring_envelope * 0.85);
    (height, scale)
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn animate_entry_effect(
    time: Res<Time>,
    mut state: ResMut<LightcycleState>,
    mut navigator: ResMut<NavigatorResource>,
    mut requests: MessageWriter<DirectoryRequested>,
    mut beam: Query<&mut Transform, (With<EntryBeam>, Without<EntryHalo>, Without<CycleEntity>)>,
    mut halos: Query<(&EntryHalo, &mut Transform), (Without<EntryBeam>, Without<CycleEntity>)>,
    mut cycle: Query<&mut Transform, (With<CycleEntity>, Without<EntryBeam>, Without<EntryHalo>)>,
) {
    let Some(fx) = state.entry_fx.as_mut() else {
        return;
    };
    fx.elapsed += time.delta_secs();
    let progress = fx.progress();
    let envelope = entry_effect_envelope(progress);

    if let Ok(mut transform) = beam.single_mut() {
        let radius = 0.15 + envelope * 0.85;
        transform.scale = Vec3::new(radius, 1.0, radius);
    }
    for (halo, mut transform) in &mut halos {
        let (height, scale) = entry_halo_pose(progress, halo.phase);
        transform.translation.y = height;
        transform.scale = Vec3::splat(scale.max(0.001));
        transform.rotate_y(time.delta_secs() * (1.8 + halo.phase));
    }

    // The cycle rises into the beam only after capture is established. Its base
    // transform is restored by update_cycle_transform immediately before this
    // system each frame, so this offset cannot accumulate.
    if let Ok(mut transform) = cycle.single_mut() {
        let lift = smoothstep((progress - 0.28) / 0.72);
        transform.translation.y += lift * config::LIGHTCYCLE_ENTRY_HALO_HEIGHT * 0.72;
        transform.scale = Vec3::splat(1.0 - lift * 0.72);
    }

    if !fx.requested && progress >= config::LIGHTCYCLE_ENTRY_FX_REQUEST_AT {
        fx.requested = true;
        let target = fx.target.clone();
        navigator.0.begin_navigate_to(&target);
        requests.write(DirectoryRequested { path: target });
    }
}

/// Rewrites the trail mesh every frame so the live end stays glued to the
/// cycle's tail instead of snapping to the last cell center.
fn update_trail_mesh(
    state: Res<LightcycleState>,
    mut meshes: ResMut<Assets<Mesh>>,
    trail: Query<&Mesh3d, With<TrailSceneRoot>>,
) {
    let Some(run) = state.run.as_ref() else {
        return;
    };
    let Ok(mesh3d) = trail.single() else {
        return;
    };
    let Some(mut mesh) = meshes.get_mut(mesh3d.id()) else {
        return;
    };
    *mesh = build_trail_mesh(&run.sim);
}

fn build_trail_mesh(sim: &LightcycleSim) -> Mesh {
    let points = trail_centerline(sim);
    let heights = trail_heights(&points);
    trail_glass_mesh(&points, &heights)
}

/// Cell-space polyline of the wall: committed trail, the same corner arc the
/// cycle is riding, then trimmed so the live end sits at the tail.
fn trail_centerline(sim: &LightcycleSim) -> Vec<(f32, f32)> {
    let mut points = if sim.trail.is_empty() {
        vec![cell_to_point(sim.cell)]
    } else {
        rounded_polyline(&sim.trail)
    };

    if let Some(arc) = corner_arc(sim) {
        if sim.queued_turn.is_some() {
            points.push(cell_to_point(sim.cell));
        }
        let samples = ((arc.u * 10.0).ceil() as usize).max(2);
        for step in 0..=samples {
            let t = arc.u * step as f32 / samples as f32;
            points.push(arc.sample(t).position);
        }
    } else {
        let pose = cycle_cell_pose(sim);
        points.push(pose.position);
    }

    let points = collapse_near_duplicates(points);
    trim_polyline_end(points, config::LIGHTCYCLE_TRAIL_TAIL)
}

fn trail_heights(points: &[(f32, f32)]) -> Vec<f32> {
    let from_end = distances_from_end(points);
    let emanate = config::LIGHTCYCLE_TRAIL_EMANATE;
    let full = config::LIGHTCYCLE_TRAIL_HEIGHT;
    let spawn = config::LIGHTCYCLE_TRAIL_SPAWN_HEIGHT;

    from_end
        .into_iter()
        .map(|distance| {
            if distance >= emanate {
                full
            } else {
                let t = (distance / emanate).clamp(0.0, 1.0);
                let smooth = t * t * (3.0 - 2.0 * t);
                spawn + (full - spawn) * smooth
            }
        })
        .collect()
}

fn distances_from_end(points: &[(f32, f32)]) -> Vec<f32> {
    if points.is_empty() {
        return Vec::new();
    }

    let mut from_start = vec![0.0; points.len()];
    for index in 1..points.len() {
        from_start[index] =
            from_start[index - 1] + point_distance(points[index - 1], points[index]);
    }
    let total = *from_start.last().unwrap_or(&0.0);
    from_start.into_iter().map(|d| total - d).collect()
}

fn point_distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = b.0 - a.0;
    let dz = b.1 - a.1;
    (dx * dx + dz * dz).sqrt()
}

fn collapse_near_duplicates(points: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    let mut collapsed = Vec::with_capacity(points.len());
    for point in points {
        if collapsed
            .last()
            .is_none_or(|previous| point_distance(*previous, point) > 1e-4)
        {
            collapsed.push(point);
        }
    }
    collapsed
}

/// Shortens the live end of a polyline by `trim` cells so the wall stops at
/// the tail instead of the cycle's origin.
fn trim_polyline_end(mut points: Vec<(f32, f32)>, trim: f32) -> Vec<(f32, f32)> {
    let mut remaining = trim;
    while points.len() >= 2 && remaining > 1e-4 {
        let last = points.len() - 1;
        let a = points[last - 1];
        let b = points[last];
        let length = point_distance(a, b);
        if length <= 1e-4 {
            points.pop();
            continue;
        }
        if remaining >= length {
            points.pop();
            remaining -= length;
        } else {
            let t = 1.0 - remaining / length;
            points[last] = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            remaining = 0.0;
        }
    }
    points
}

/// Extrudes a thin glass slab along `points`. Heights vary so the live end is
/// a meniscus at the tail rather than a chopped cuboid.
fn trail_glass_mesh(points: &[(f32, f32)], heights: &[f32]) -> Mesh {
    if points.len() < 2 || heights.len() != points.len() {
        return collapsed_trail_mesh(points.first().copied().unwrap_or_default());
    }

    let spacing = config::GRID_SPACING;
    let half_thick = config::LIGHTCYCLE_TRAIL_THICKNESS * 0.5;
    let stations: Vec<(Vec3, Vec3, f32)> = points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let origin = Vec3::new(point.0 * spacing, 0.0, point.1 * spacing);
            let tangent = polyline_tangent(points, index);
            let side = Vec3::Y.cross(tangent).normalize_or_zero() * half_thick;
            (origin, side, heights[index])
        })
        .collect();

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    for window in stations.windows(2) {
        let (a_origin, a_side, a_height) = window[0];
        let (b_origin, b_side, b_height) = window[1];

        let a_left = a_origin - a_side;
        let a_right = a_origin + a_side;
        let b_left = b_origin - b_side;
        let b_right = b_origin + b_side;
        let a_left_top = a_left + Vec3::Y * a_height;
        let a_right_top = a_right + Vec3::Y * a_height;
        let b_left_top = b_left + Vec3::Y * b_height;
        let b_right_top = b_right + Vec3::Y * b_height;

        push_quad(
            &mut positions,
            &mut normals,
            &mut indices,
            a_left,
            b_left,
            b_left_top,
            a_left_top,
        );
        push_quad(
            &mut positions,
            &mut normals,
            &mut indices,
            a_right,
            a_right_top,
            b_right_top,
            b_right,
        );
        push_quad(
            &mut positions,
            &mut normals,
            &mut indices,
            a_left_top,
            b_left_top,
            b_right_top,
            a_right_top,
        );
        push_quad(
            &mut positions,
            &mut normals,
            &mut indices,
            a_left,
            a_right,
            b_right,
            b_left,
        );
    }

    let (origin, side, height) = stations[0];
    push_quad(
        &mut positions,
        &mut normals,
        &mut indices,
        origin - side,
        origin - side + Vec3::Y * height,
        origin + side + Vec3::Y * height,
        origin + side,
    );
    let (origin, side, height) = stations[stations.len() - 1];
    push_quad(
        &mut positions,
        &mut normals,
        &mut indices,
        origin - side,
        origin + side,
        origin + side + Vec3::Y * height,
        origin - side + Vec3::Y * height,
    );

    trail_mesh_from(positions, normals, indices)
}

/// An invisible, zero-area quad standing in for a ribbon too short to draw.
///
/// The trail mesh must never be zero-vertex. Bevy's mesh allocator skips
/// allocating a mesh with an empty vertex buffer but still runs the upload for
/// it, which logs `Use-after-free: attempted to copy element data for an
/// unallocated key` every frame. A run has no ribbon yet for the fraction of a
/// cell it takes the tail to clear its spawn, and again after every restart and
/// folder entry, so this is the common case rather than an edge case.
fn collapsed_trail_mesh(anchor: (f32, f32)) -> Mesh {
    let spacing = config::GRID_SPACING;
    let point = Vec3::new(anchor.0 * spacing, 0.0, anchor.1 * spacing);

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    push_quad(
        &mut positions,
        &mut normals,
        &mut indices,
        point,
        point,
        point,
        point,
    );
    trail_mesh_from(positions, normals, indices)
}

fn trail_mesh_from(positions: Vec<[f32; 3]>, normals: Vec<[f32; 3]>, indices: Vec<u32>) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}

fn polyline_tangent(points: &[(f32, f32)], index: usize) -> Vec3 {
    let previous = if index == 0 {
        points[0]
    } else {
        points[index - 1]
    };
    let next = if index + 1 == points.len() {
        points[index]
    } else {
        points[index + 1]
    };
    Vec3::new(next.0 - previous.0, 0.0, next.1 - previous.1).normalize_or_zero()
}

fn push_quad(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    a: Vec3,
    b: Vec3,
    c: Vec3,
    d: Vec3,
) {
    let normal = (b - a).cross(d - a).normalize_or_zero();
    let start = positions.len() as u32;
    for vertex in [a, b, c, d] {
        positions.push(vertex.to_array());
        normals.push(normal.to_array());
    }
    indices.extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
}

fn rounded_polyline(path: &[(i32, i32)]) -> Vec<(f32, f32)> {
    let radius = config::LIGHTCYCLE_TURN_RADIUS;
    let mut points = vec![cell_to_point(path[0])];

    for index in 1..path.len().saturating_sub(1) {
        let previous = path[index - 1];
        let corner = path[index];
        let next = path[index + 1];

        if is_path_turn(previous, corner, next) {
            let incoming = (corner.0 - previous.0, corner.1 - previous.1);
            let outgoing = (next.0 - corner.0, next.1 - corner.1);
            let arc_start = offset_cell_point(corner, incoming, -radius);
            points.push(arc_start);

            let samples = 10;
            for step in 1..=samples {
                let u = step as f32 / samples as f32;
                points.push(arc_cell_pose(corner, incoming, outgoing, u, radius).position);
            }
        } else {
            points.push(cell_to_point(corner));
        }
    }

    if let Some(last) = path.last() {
        points.push(cell_to_point(*last));
    }
    points
}

fn is_path_turn(a: (i32, i32), b: (i32, i32), c: (i32, i32)) -> bool {
    let incoming = (b.0 - a.0, b.1 - a.1);
    let outgoing = (c.0 - b.0, c.1 - b.1);
    incoming.0 * outgoing.0 + incoming.1 * outgoing.1 == 0
}

fn cell_to_point(cell: (i32, i32)) -> (f32, f32) {
    (cell.0 as f32, cell.1 as f32)
}

fn offset_cell_point(cell: (i32, i32), direction: (i32, i32), distance: f32) -> (f32, f32) {
    (
        cell.0 as f32 + direction.0 as f32 * distance,
        cell.1 as f32 + direction.1 as f32 * distance,
    )
}

/// Continuous render pose for the cycle, in cell coordinates.
struct CyclePose {
    position: (f32, f32),
    /// Unit travel direction; the cycle's nose points along it.
    direction: Vec2,
    /// Bank angle about the travel direction, in radians. Zero outside corners.
    lean: f32,
}

/// Ground-level world position of the pose; the model's wheels sit at its origin.
fn pose_world_position(pose: &CyclePose) -> Vec3 {
    Vec3::new(
        pose.position.0 * config::GRID_SPACING,
        0.0,
        pose.position.1 * config::GRID_SPACING,
    )
}

fn pose_forward(pose: &CyclePose) -> Vec3 {
    Vec3::new(pose.direction.x, 0.0, pose.direction.y)
}

/// Yaw along the travel direction, then bank into the corner. The bank rotates
/// about the cycle's own +X, which is its direction of travel, so it leaves the
/// forward vector untouched.
fn pose_rotation(pose: &CyclePose) -> Quat {
    let yaw = match pose_forward(pose).try_normalize() {
        Some(forward) => Quat::from_rotation_arc(Vec3::X, forward),
        None => Quat::IDENTITY,
    };
    yaw * Quat::from_rotation_x(pose.lean)
}

fn cycle_world_position(sim: &LightcycleSim) -> Vec3 {
    pose_world_position(&cycle_cell_pose(sim))
}

/// Continuous cell-space pose for the rendered cycle.
///
/// Straight segments use the raw simulation position and heading. Near
/// queued/applied turns the pose follows a rounded 90-degree arc around the
/// intersection, taking its facing from the arc's tangent, so the cycle steers
/// through the corner instead of sliding around it and rotating afterwards.
fn cycle_cell_pose(sim: &LightcycleSim) -> CyclePose {
    if let Some(arc) = corner_arc(sim) {
        return arc.sample(arc.u);
    }

    let (dx, dz) = sim.heading.delta();
    CyclePose {
        position: (
            sim.cell.0 as f32 + dx as f32 * sim.cell_t,
            sim.cell.1 as f32 + dz as f32 * sim.cell_t,
        ),
        direction: Vec2::new(dx as f32, dz as f32),
        lean: 0.0,
    }
}

/// The live corner the cycle is riding, if any.
#[derive(Clone, Copy)]
struct CornerArc {
    corner: (i32, i32),
    incoming: (i32, i32),
    outgoing: (i32, i32),
    u: f32,
    radius: f32,
}

impl CornerArc {
    fn sample(self, u: f32) -> CyclePose {
        arc_cell_pose(self.corner, self.incoming, self.outgoing, u, self.radius)
    }
}

fn corner_arc(sim: &LightcycleSim) -> Option<CornerArc> {
    let radius = config::LIGHTCYCLE_TURN_RADIUS;

    // Approaching a queued turn: the first half of the arc happens just before
    // the cycle reaches the intersection cell.
    if let Some(turn) = sim.queued_turn
        && sim.cell_t >= 1.0 - radius
    {
        let incoming = sim.heading.delta();
        let outgoing = sim.heading.turn(turn).delta();
        let u = ((sim.cell_t - (1.0 - radius)) / radius) * 0.5;
        return Some(CornerArc {
            corner: sim.next_cell(),
            incoming,
            outgoing,
            u,
            radius,
        });
    }

    // Just applied a turn: render the second half of the arc after leaving the
    // intersection cell. The previous trail cell tells us the incoming heading.
    if sim.queued_turn.is_none()
        && sim.cell_t <= radius
        && let Some(&previous) = sim.trail.last()
    {
        let incoming = (sim.cell.0 - previous.0, sim.cell.1 - previous.1);
        let outgoing = sim.heading.delta();
        let is_turn = incoming.0 * outgoing.0 + incoming.1 * outgoing.1 == 0;
        if is_turn {
            let u = 0.5 + (sim.cell_t / radius) * 0.5;
            return Some(CornerArc {
                corner: sim.cell,
                incoming,
                outgoing,
                u,
                radius,
            });
        }
    }

    None
}

/// Samples the rounded corner centered on `corner` at `u`, where 0 is the arc
/// entry (`radius` before the corner, travelling along `incoming`) and 1 is the
/// exit (`radius` past it, travelling along `outgoing`).
fn arc_cell_pose(
    corner: (i32, i32),
    incoming: (i32, i32),
    outgoing: (i32, i32),
    u: f32,
    radius: f32,
) -> CyclePose {
    let center_x = corner.0 as f32 - incoming.0 as f32 * radius + outgoing.0 as f32 * radius;
    let center_z = corner.1 as f32 - incoming.1 as f32 * radius + outgoing.1 as f32 * radius;

    let start_angle = (-outgoing.1 as f32).atan2(-outgoing.0 as f32);
    let end_angle = (incoming.1 as f32).atan2(incoming.0 as f32);

    let mut sweep = end_angle - start_angle;
    if sweep > std::f32::consts::PI {
        sweep -= std::f32::consts::TAU;
    } else if sweep < -std::f32::consts::PI {
        sweep += std::f32::consts::TAU;
    }

    // Positive sweep curves toward the cycle's right, which is also the
    // direction it should bank.
    let u = u.clamp(0.0, 1.0);
    let theta = start_angle + sweep * u;
    let turn_sign = sweep.signum();

    CyclePose {
        position: (
            center_x + radius * theta.cos(),
            center_z + radius * theta.sin(),
        ),
        direction: Vec2::new(-theta.sin(), theta.cos()) * turn_sign,
        // Peaks mid-corner and returns upright by the exit.
        lean: turn_sign * config::LIGHTCYCLE_LEAN_ANGLE * (std::f32::consts::PI * u).sin(),
    }
}

fn update_cycle_transform(
    state: Res<LightcycleState>,
    mut cycle: Query<(&mut Transform, &mut Visibility), With<CycleEntity>>,
) {
    let Ok((mut transform, mut visibility)) = cycle.single_mut() else {
        return;
    };
    let Some(run) = state.run.as_ref() else {
        return;
    };

    // The on-foot games park the bike out of sight and pose their own character.
    if run.source_platformer().is_some() || run.source_stealth().is_some() {
        *visibility = Visibility::Hidden;
        return;
    }
    *visibility = Visibility::Visible;

    // In the breaker the bike is the paddle: it slides along the bottom of the
    // court and rebounds the ball.
    if let Some(level) = run.source_breaker() {
        transform.translation = Vec3::new(level.paddle_x, config::BREAKER_PADDLE_Y, 0.0);
        transform.rotation = Quat::IDENTITY;
        transform.scale = Vec3::splat(config::BREAKER_PADDLE_SCALE);
        return;
    }
    transform.scale = Vec3::ONE;

    // The surfer rides the shared bike as a hovercraft over the river, bobbing
    // with the waves and leaning into the steering heading.
    if let Some(surfer) = run.source_surfer() {
        transform.translation = Vec3::new(surfer.x, surfer.height, surfer.z);
        transform.rotation = Quat::from_rotation_arc(
            Vec3::X,
            Vec3::new(surfer.heading.cos(), 0.0, surfer.heading.sin()),
        );
        return;
    }

    // The Galaga field parks the bike on the bottom edge, facing up the field,
    // and slides it side to side.
    if let Some(sim) = run.source_galaga() {
        transform.translation = Vec3::new(sim.player_x, 0.0, config::GALAGA_PLAYER_Z);
        transform.rotation = Quat::from_rotation_arc(Vec3::X, Vec3::Z);
        return;
    }

    // Pac-Man rides the maze corridors.
    if let Some(sim) = run.source_pacman() {
        transform.translation = Vec3::new(sim.x, 0.7, sim.z);
        transform.rotation = Quat::from_rotation_arc(Vec3::X, Vec3::Z);
        return;
    }

    // Columns and Tetris park the bike at the foot of the well; Plinko parks
    // it on the top rail.
    if run.source_columns().is_some() {
        transform.translation = Vec3::new(0.0, 0.0, 3.0);
        transform.rotation = Quat::from_rotation_arc(Vec3::X, Vec3::Y);
        return;
    }
    if run.source_tetris().is_some() {
        transform.translation = Vec3::new(0.0, 0.0, 4.0);
        transform.rotation = Quat::from_rotation_arc(Vec3::X, Vec3::Y);
        return;
    }
    if let Some(sim) = run.source_plinko() {
        transform.translation = Vec3::new(sim.aim, config::PLINKO_HEIGHT * 0.5 - 1.0, 1.5);
        transform.rotation = Quat::from_rotation_arc(Vec3::X, Vec3::Y);
        return;
    }

    // Frogger and Bomberman walk the cycle on their X/Z grids.
    if let Some(sim) = run.source_frogger() {
        let (x, z) = FroggerSim::center(sim.cell);
        transform.translation = Vec3::new(x, 0.7, z);
        transform.rotation = Quat::from_rotation_arc(Vec3::X, Vec3::Z);
        return;
    }
    if let Some(sim) = run.source_bomberman() {
        let (x, z) = BomberSim::center(sim.cell);
        transform.translation = Vec3::new(x, 0.7, z);
        transform.rotation = Quat::from_rotation_arc(Vec3::X, Vec3::Z);
        return;
    }

    // Q*bert perches the bike on its current cube.
    if let Some(sim) = run.source_qbert() {
        let (x, z) = QbertSim::cube_position(sim.row, sim.index);
        let y =
            (config::QBERT_ROWS as f32 - 1.0 - sim.row as f32) * config::QBERT_CUBE_HEIGHT * 0.5
                + config::QBERT_CUBE_HEIGHT * 0.6;
        transform.translation = Vec3::new(x, y, z);
        transform.rotation = Quat::from_rotation_arc(Vec3::X, Vec3::Z);
        return;
    }

    let pose = cycle_cell_pose(&run.sim);
    transform.translation = pose_world_position(&pose);
    // A parked cycle pivots on the spot: its facing is the field's aim angle,
    // not a grid heading. Once the field ends it drives again, so the grid
    // heading takes over — and a disc-wars ring never leaves it in the first
    // place.
    let aiming = if run.asteroid_field_active() {
        run.source_asteroids()
    } else {
        None
    };
    transform.rotation = match aiming {
        Some(sim) => {
            Quat::from_rotation_arc(Vec3::X, Vec3::new(sim.angle.cos(), 0.0, sim.angle.sin()))
        }
        None => pose_rotation(&pose),
    };
}

/// Eases the camera's follow direction toward `target` with a frame-rate
/// independent time constant.
fn advance_chase_forward(current: Vec3, target: Vec3, delta: f32) -> Vec3 {
    let blend = 1.0 - (-delta / config::LIGHTCYCLE_CAMERA_TURN_LAG).exp();
    current
        .lerp(target, blend.clamp(0.0, 1.0))
        .try_normalize()
        .unwrap_or(target)
}

/// Places the chase rig around the cycle for a follow direction and free-look
/// offset, returning the camera's offset from the cycle and the direction it
/// views along.
///
/// A zero `look` reproduces the fixed rig: [`config::LIGHTCYCLE_CAMERA_DISTANCE`]
/// behind the direction of travel and [`config::LIGHTCYCLE_CAMERA_HEIGHT`] above
/// it. Free look orbits that same radius so dragging never pushes the camera
/// through the floor or into the cycle.
fn chase_camera_rig(forward: Vec3, look: Vec2) -> (Vec3, Vec3) {
    let view_forward = Quat::from_rotation_y(look.x) * forward;
    let pitch = (chase_base_pitch() + look.y).clamp(
        config::LIGHTCYCLE_CAMERA_MIN_PITCH,
        config::LIGHTCYCLE_CAMERA_MAX_PITCH,
    );
    let radius = chase_rig_radius();
    let offset = Vec3::Y * (radius * pitch.sin()) - view_forward * (radius * pitch.cos());
    (offset, view_forward)
}

/// Pitch of the default chase rig above the cycle, in radians.
fn chase_base_pitch() -> f32 {
    config::LIGHTCYCLE_CAMERA_HEIGHT.atan2(config::LIGHTCYCLE_CAMERA_DISTANCE)
}

/// Distance from the cycle to the default chase rig.
fn chase_rig_radius() -> f32 {
    Vec2::new(
        config::LIGHTCYCLE_CAMERA_DISTANCE,
        config::LIGHTCYCLE_CAMERA_HEIGHT,
    )
    .length()
}

/// The river surfer's chase rig: lower and closer than the street rig, so the
/// water and the gates read as a course rather than a flyover. Same free-look
/// orbit, same pitch clamps.
fn surfer_camera_rig(forward: Vec3, look: Vec2) -> (Vec3, Vec3) {
    let view_forward = Quat::from_rotation_y(look.x) * forward;
    let pitch = (config::SURFER_CAMERA_HEIGHT.atan2(config::SURFER_CAMERA_DISTANCE) + look.y)
        .clamp(
            config::LIGHTCYCLE_CAMERA_MIN_PITCH,
            config::LIGHTCYCLE_CAMERA_MAX_PITCH,
        );
    let radius = Vec2::new(config::SURFER_CAMERA_DISTANCE, config::SURFER_CAMERA_HEIGHT).length();
    let offset = Vec3::Y * (radius * pitch.sin()) - view_forward * (radius * pitch.cos());
    (offset, view_forward)
}

/// Facing, in radians, for a grid heading, matching the field's aim convention
/// (`0` is `+X`, growing toward `+Z`).
fn heading_facing(heading: Heading) -> f32 {
    heading_angle(heading)
}

/// Facing of a grid heading, in radians.
fn heading_angle(heading: Heading) -> f32 {
    heading.angle()
}

/// The grid heading closest to an aim angle. Used when the field ends so the
/// bike drives off in the direction the player was holding.
fn nearest_heading(angle: f32) -> Heading {
    let (x, z) = (angle.cos(), angle.sin());
    if x.abs() >= z.abs() {
        if x >= 0.0 {
            Heading::PosX
        } else {
            Heading::NegX
        }
    } else if z >= 0.0 {
        Heading::PosZ
    } else {
        Heading::NegZ
    }
}

/// Focus point and ring radius while the field is live. Once it is decided the
/// camera returns to the chase rig so the player can drive out, and a disc-wars
/// ring keeps the chase rig throughout.
fn field_camera_focus(run: &ActiveRun) -> Option<(Vec3, f32)> {
    if !run.asteroid_field_active() {
        return None;
    }
    let sim = run.source_asteroids()?;
    Some((Vec3::new(sim.center.0, 0.0, sim.center.1), sim.radius))
}

/// One wall-hug camera pose: where the camera stands and what it looks at, both
/// as offsets from the character's cell centre, plus the camera height.
struct HugShot {
    offset: Vec3,
    look: Vec3,
    height: f32,
}

/// Picks the wall-hug camera pose for a character with its back to a wall.
///
/// The camera is treated as an imaginary second figure standing off the wall
/// and looking back at the real one. Standing past the corner on the open side
/// and aiming back across it is what keeps every element of the shot in frame
/// at once: the character sits on one side, the wall he is hugging runs across
/// the middle as a low edge, and the corner with the corridor around it opens
/// on the other side.
///
/// When the wall runs on without a corner in reach, the camera trails the
/// character instead and looks down the corridor ahead of him.
fn hug_camera_shot(room: &StealthSim) -> Option<HugShot> {
    let wall = room.hug?;
    let across = room.peek?;
    let (px, pz) = unit_of(across);
    let (wx, wz) = unit_of(wall);
    let spacing = config::GRID_SPACING;

    // Follow the wall toward the peek until it ends. `run` counts the solid
    // wall cells passed, so the first open cell behind the wall's end is
    // `run * spacing` along the wall from the character.
    let mut cell = room.character;
    let mut run = 0;
    while run < config::STEALTH_PEEK_STEPS && room.is_solid(step_cell(cell, wall)) {
        cell = step_cell(cell, across);
        run += 1;
    }

    if run <= config::STEALTH_HUG_CORNER_STEPS {
        // A reachable corner: stand past it and out from the hugged face. The
        // farther the corner is, the farther out the camera has to stand for
        // the corner and the corridor behind it to stay inside the frame.
        let gap = run as f32 * spacing;
        let out = config::STEALTH_HUG_CAMERA_OUT
            + run.saturating_sub(1) as f32 * config::STEALTH_HUG_CAMERA_OUT_STEP;
        let offset = Vec3::new(
            px * (gap + config::STEALTH_HUG_CAMERA_PAST) - wx * out,
            0.0,
            pz * (gap + config::STEALTH_HUG_CAMERA_PAST) - wz * out,
        );
        // Aim at the wall-top corner halfway to the gap cell centre: the
        // character is then on one side of the view and the corridor around
        // the corner on the other.
        let look = Vec3::new(
            (px * gap + wx * spacing) * 0.5,
            config::STEALTH_WALL_HEIGHT,
            (pz * gap + wz * spacing) * 0.5,
        );
        Some(HugShot {
            offset,
            look,
            height: config::STEALTH_HUG_CAMERA_HEIGHT,
        })
    } else {
        // No corner in reach: trail the character along the wall and look down
        // the corridor ahead, with the wall beside him sharing the frame.
        let offset = Vec3::new(
            -px * config::STEALTH_HUG_CAMERA_BACK - wx * config::STEALTH_HUG_CAMERA_OUT,
            0.0,
            -pz * config::STEALTH_HUG_CAMERA_BACK - wz * config::STEALTH_HUG_CAMERA_OUT,
        );
        let look = Vec3::new(
            px * config::STEALTH_HUG_CAMERA_AIM,
            config::STEALTH_CAMERA_LOOK,
            pz * config::STEALTH_HUG_CAMERA_AIM,
        );
        Some(HugShot {
            offset,
            look,
            height: config::STEALTH_HUG_CAMERA_HEIGHT,
        })
    }
}

// A Bevy system: the queries are the reason for both of these, and folding them
// into a SystemParam struct would only move the noise.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_chase_camera(
    state: Res<LightcycleState>,
    transition: Res<ModeTransition>,
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut camera: Single<&mut Transform, (With<Camera3d>, Without<CycleEntity>)>,
    character: Query<
        &Transform,
        (
            With<CharacterEntity>,
            Without<Camera3d>,
            Without<ChaseCamera>,
        ),
    >,
    mut cycle: Query<(&Transform, &mut ChaseCamera), Without<Camera3d>>,
) {
    let Ok((cycle, mut chase)) = cycle.single_mut() else {
        return;
    };
    // The flight owns the camera until it lands on this rig; easing the follow
    // direction or taking a look drag now would move the pose it is aiming for.
    if state.run.is_none() || transition.is_active() {
        return;
    }

    // The platformer is played from the side, riding along with the runner.
    if let Some(level) = state.run.as_ref().and_then(|run| run.source_platformer()) {
        let focus = Vec3::new(
            level.runner.x + config::PLATFORMER_CAMERA_AHEAD,
            (level.runner.y + config::PLATFORMER_CAMERA_HEIGHT).max(2.0),
            0.0,
        );
        let target = Vec3::new(focus.x, focus.y, config::PLATFORMER_CAMERA_BACK);
        let blend = 1.0 - (-config::PLATFORMER_CAMERA_LERP * time.delta_secs()).exp();
        camera.translation = camera.translation.lerp(target, blend);
        camera.look_at(focus, Vec3::Y);
        return;
    }

    // The breaker is played head-on: the whole court stays in frame while the
    // bike slides along the bottom.
    if let Some(level) = state.run.as_ref().and_then(|run| run.source_breaker()) {
        let centre = Vec3::new(0.0, level.court.1 * 0.5, 0.0);
        camera.translation = Vec3::new(0.0, centre.y, config::BREAKER_CAMERA_BACK);
        camera.look_at(centre, Vec3::Y);
        return;
    }

    // The stealth run is played from above, like a stakeout.
    if let Some(room) = state.run.as_ref().and_then(|run| run.source_stealth()) {
        // Follow where the figure is actually drawn, not the cell it is walking
        // toward: the sim moves in whole cells, so tracking the cell would lurch
        // the whole view once per step.
        let focus = character
            .single()
            .map(|transform| transform.translation)
            .unwrap_or_else(|_| config::ground_position(room.character.0, room.character.1));
        // The camera holds a bearing round the figure and turns steadily toward
        // whatever the view should be aimed along: round the far side of the peek
        // direction when the player is backed against a wall, and plain +Z
        // otherwise.
        //
        // It turns at a fixed rate rather than easing, because that is what makes
        // the swing watchable: an ease puts nearly all the movement in the first
        // few frames, which is why the perspective read as changing instantly.
        // The radius and height still ease, so entering a run flies in as before.
        // Where the view should sit, and what it should look at, both as offsets
        // from the figure.
        //
        // Backed against a wall, the pose comes from [`hug_camera_shot`]: the
        // camera acts like an imaginary second figure standing off the wall and
        // looking back at the real one, so the figure, the wall he is hugging,
        // the corner and the corridor around it all share the frame.
        let (want_x, want_z, want_height, look) = match hug_camera_shot(room) {
            Some(shot) => (shot.offset.x, shot.offset.z, shot.height, shot.look),
            None => (
                0.0,
                config::STEALTH_CAMERA_DISTANCE,
                config::STEALTH_CAMERA_HEIGHT,
                Vec3::Y * config::STEALTH_CAMERA_LOOK,
            ),
        };
        let want_radius = (want_x * want_x + want_z * want_z).sqrt();
        let aim = want_z.atan2(want_x);
        let offset = camera.translation - focus;
        let bearing = offset.z.atan2(offset.x);
        let radius = (offset.x * offset.x + offset.z * offset.z).sqrt();
        let turn = config::STEALTH_SWING_RATE * time.delta_secs();
        let to_aim = (aim - bearing + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let bearing = bearing + to_aim.clamp(-turn, turn);
        let blend = 1.0 - (-config::STEALTH_CAMERA_LERP * time.delta_secs()).exp();
        let radius = radius + (want_radius - radius) * blend;
        let height = offset.y + (want_height - offset.y) * blend;
        camera.translation =
            focus + Vec3::new(bearing.cos() * radius, height, bearing.sin() * radius);
        camera.look_at(focus + look, Vec3::Y);
        return;
    }

    // The asteroid field is played from above: the whole ring stays in frame, so
    // pivoting the parked cycle does not whip the camera around with it.
    if let Some((center, radius)) = state.run.as_ref().and_then(field_camera_focus) {
        let height = radius * config::ASTEROIDS_CAMERA_FIT + 3.0;
        camera.translation =
            center + Vec3::new(0.0, height, height * config::ASTEROIDS_CAMERA_LEAN);
        camera.look_at(center, Vec3::Y);
        return;
    }

    // The Galaga field is played from above too: the whole formation stays in
    // frame while the cycle slides along the bottom.
    if state
        .run
        .as_ref()
        .and_then(|run| run.source_galaga())
        .is_some()
    {
        let center = Vec3::ZERO;
        let height = config::GALAGA_CAMERA_HEIGHT;
        // Lean the camera in from -Z so the cycle (parked at -Z) sits at the
        // bottom of the screen and the formation hangs above it.
        camera.translation = center + Vec3::new(0.0, height, -height * config::GALAGA_CAMERA_LEAN);
        camera.look_at(center, Vec3::Y);
        return;
    }

    // The arcade block: each game gets a small fixed camera tailored to its
    // board, independent of the parked cycle. Every other source game (and
    // plain directory riding) keeps the chase camera below.
    let arcade_game = state
        .run
        .as_ref()
        .and_then(|run| run.source_game())
        .filter(|game| {
            matches!(
                game,
                SourceGame::PacMan
                    | SourceGame::Columns
                    | SourceGame::Tetris
                    | SourceGame::Frogger
                    | SourceGame::Qbert
                    | SourceGame::Bomberman
                    | SourceGame::Plinko
            )
        });
    if let Some(game) = arcade_game {
        let (translation, target) = match game {
            SourceGame::PacMan => (
                Vec3::new(
                    0.0,
                    config::PAC_CAMERA_HEIGHT,
                    config::PAC_CAMERA_HEIGHT * config::PAC_CAMERA_LEAN,
                ),
                Vec3::ZERO,
            ),
            SourceGame::Frogger => (
                Vec3::new(
                    0.0,
                    config::FROGGER_CAMERA_HEIGHT,
                    config::FROGGER_CAMERA_HEIGHT * config::FROGGER_CAMERA_LEAN,
                ),
                Vec3::ZERO,
            ),
            SourceGame::Qbert => (
                Vec3::new(
                    0.0,
                    config::QBERT_CAMERA_HEIGHT,
                    -config::QBERT_CAMERA_HEIGHT * config::QBERT_CAMERA_LEAN,
                ),
                Vec3::new(0.0, 1.0, 0.0),
            ),
            SourceGame::Bomberman => (
                Vec3::new(
                    0.0,
                    config::BOMBER_CAMERA_HEIGHT,
                    config::BOMBER_CAMERA_HEIGHT * config::BOMBER_CAMERA_LEAN,
                ),
                Vec3::ZERO,
            ),
            SourceGame::Columns => (
                Vec3::new(0.0, 10.4, config::COLUMNS_CAMERA_BACK),
                Vec3::new(0.0, 10.4, 0.0),
            ),
            SourceGame::Tetris => (
                Vec3::new(0.0, 12.0, config::TETRIS_CAMERA_BACK),
                Vec3::new(0.0, 12.0, 0.0),
            ),
            SourceGame::Plinko => (Vec3::new(0.0, 0.0, config::PLINKO_CAMERA_BACK), Vec3::ZERO),
            _ => unreachable!("filtered to the arcade block above"),
        };
        camera.translation = translation;
        camera.look_at(target, Vec3::Y);
        return;
    }

    let travel = cycle.rotation * Vec3::X;
    chase.forward = advance_chase_forward(
        chase.forward,
        Vec3::new(travel.x, 0.0, travel.z),
        time.delta_secs(),
    );

    if mouse_buttons.pressed(MouseButton::Right) {
        if mouse_motion.delta != Vec2::ZERO {
            chase.apply_look_drag(mouse_motion.delta);
        }
    } else {
        chase.recenter_look(time.delta_secs());
    }

    let cycle_pos = cycle.translation;
    let surfing = state
        .run
        .as_ref()
        .and_then(|run| run.source_surfer())
        .is_some();
    let (offset, view_forward) = if surfing {
        surfer_camera_rig(chase.forward, chase.look)
    } else {
        chase_camera_rig(chase.forward, chase.look)
    };
    let lookahead = if surfing {
        config::SURFER_CAMERA_LOOKAHEAD
    } else {
        config::LIGHTCYCLE_CAMERA_LOOKAHEAD
    };
    let look_target = cycle_pos + view_forward * lookahead;
    let mut camera_position = cycle_pos + offset;

    if let Some(fx) = state.crash_fx.as_ref() {
        let intensity = (fx.timer / fx.duration).clamp(0.0, 1.0);
        let t = time.elapsed_secs();
        let shake =
            Vec3::new((t * 83.0).sin(), (t * 97.0).sin(), (t * 71.0).sin()) * (intensity * 0.9);
        camera_position += shake;
    }

    camera.translation = camera_position;
    camera.look_at(look_target, Vec3::Y);
}

#[cfg(test)]
mod tests {
    use super::{
        CITY_TRIM_ACCENT, ChaseCamera, GateScanBar, arc_cell_pose, build_trail_mesh,
        chase_camera_rig, chase_rig_radius, city_base_trim_mesh, city_body_height, city_body_mesh,
        city_cap_mesh, city_foundation_mesh, city_palette, city_theme_index, cycle_cell_pose,
        document_line_advance, entry_effect_envelope, entry_halo_pose, gate_bar_height, gate_pulse,
        glyph_char_offset, glyph_pixel_offset, glyph_pixels, heading_facing, hug_camera_shot,
        nearest_heading, pose_forward, pose_rotation, pose_world_position, rail_segments,
        road_marking_mesh, trail_centerline, trail_heights, trim_polyline_end, wrap_angle,
    };
    use crate::config;
    use crate::lightcycle::logic::{
        CityStructure, CityStructureKind, CityTheme, Heading, LightcycleSim, Turn,
    };
    use crate::stealth::StealthSim;
    use bevy::camera::primitives::MeshAabb;
    use bevy::prelude::{Vec2, Vec3};
    use std::collections::BTreeSet;

    const RADIUS: f32 = config::LIGHTCYCLE_TURN_RADIUS;

    /// Direction the cycle asset's nose points in its own model space.
    ///
    /// The glTF is Y-up with its length on X, and its canopy peaks near the
    /// origin then slopes down to a point toward +X, so the nose is +X.
    /// `LIGHTCYCLE_MODEL_YAW` has to rotate this onto the entity's forward
    /// axis, and the heading test keeps the two in agreement.
    const MODEL_NOSE_AXIS: Vec3 = Vec3::X;

    /// A right turn at cell (1, 0): entering along +X, leaving along +Z.
    fn right_corner(u: f32) -> super::CyclePose {
        arc_cell_pose((1, 0), (1, 0), (0, 1), u, RADIUS)
    }

    fn city_structure(kind: CityStructureKind, along_x: bool, tier: u8) -> CityStructure {
        CityStructure {
            cell: (2, -3),
            kind,
            along_x,
            height_tier: tier,
            accent: 0,
            pulse_phase: 0,
        }
    }

    /// The horizontal view a camera must keep its subjects inside. Half a
    /// frame at the narrowest aspect the stealth run has to survive.
    const HUG_SHOT_HALF_VIEW: f32 = 30.0 * std::f32::consts::PI / 180.0;

    /// Ground-plane angle from `from` to `to`, in radians.
    fn horizontal_angle(from: Vec3, to: Vec3) -> f32 {
        let d = to - from;
        d.z.atan2(d.x)
    }

    /// A room whose character hugs an east wall made of `wall_cells`.
    fn hugging_room(wall_cells: &[(i32, i32)]) -> StealthSim {
        let mut room = StealthSim::new(1);
        room.guards.clear();
        room.cover.clear();
        room.character = (0, 0);
        room.cover.extend(wall_cells.iter().copied());
        room.hug = Some(Heading::PosX);
        room.peek = Some(Heading::PosZ);
        room
    }

    /// Asserts that the imaginary figure at `shot.offset` looking at `shot.look`
    /// has every `subject` inside its view.
    fn assert_shot_frames(shot: &super::HugShot, subjects: &[(&str, Vec3)]) {
        let camera = shot.offset;
        let view = horizontal_angle(shot.offset, shot.look);
        for (label, point) in subjects {
            let delta = wrap_angle(horizontal_angle(camera, *point) - view).abs();
            assert!(
                delta <= HUG_SHOT_HALF_VIEW,
                "{label} sits {delta:.3} rad from the view centre, past the half-view \
                 {HUG_SHOT_HALF_VIEW:.3}",
            );
        }
    }

    #[test]
    fn the_wall_hug_camera_frames_everything_round_the_corner() {
        let room = hugging_room(&[(1, 0)]);
        let shot = hug_camera_shot(&room).expect("a hugging character gets a shot");
        let s = config::GRID_SPACING;
        // The five things the shot has to show at once: the figure, the wall he
        // is hugging, the corner, around the corner, and the corridor right
        // behind the corner.
        assert_shot_frames(
            &shot,
            &[
                ("figure", Vec3::ZERO),
                ("hugged wall", Vec3::new(s * 0.5, 0.0, 0.0)),
                ("corner", Vec3::new(s * 0.5, 0.0, s * 0.5)),
                ("gap", Vec3::new(s, 0.0, s)),
                ("corridor", Vec3::new(s * 2.0, 0.0, s)),
            ],
        );
    }

    #[test]
    fn a_corner_one_cell_on_stands_the_camera_out_farther() {
        let room = hugging_room(&[(1, 0), (1, 1)]);
        let shot = hug_camera_shot(&room).expect("a hugging character gets a shot");
        let s = config::GRID_SPACING;
        // For a wall running east of the figure, the camera's west offset is
        // how far out it stands, and it must grow for a corner that is a cell
        // away or the corridor behind it slips out of frame.
        assert!(
            -shot.offset.x > config::STEALTH_HUG_CAMERA_OUT,
            "the camera should stand out farther than at the corner, got {:?}",
            shot.offset
        );
        assert_shot_frames(
            &shot,
            &[
                ("figure", Vec3::ZERO),
                ("hugged wall", Vec3::new(s * 0.5, 0.0, 0.0)),
                ("corner", Vec3::new(s * 0.5, 0.0, s * 1.5)),
                ("gap", Vec3::new(s, 0.0, s * 2.0)),
                ("corridor", Vec3::new(s * 2.0, 0.0, s * 2.0)),
            ],
        );
    }

    #[test]
    fn a_wall_that_runs_on_gets_a_corridor_shot() {
        let room = hugging_room(&[(1, 0), (1, 1), (1, 2), (1, 3), (1, 4)]);
        let shot = hug_camera_shot(&room).expect("a hugging character gets a shot");
        let s = config::GRID_SPACING;
        // No corner in reach: the camera trails the figure along the wall and
        // looks down the corridor ahead instead of standing out to peek.
        assert!(
            shot.offset.z < 0.0,
            "the camera trails the figure along the wall, got {:?}",
            shot.offset
        );
        assert_shot_frames(
            &shot,
            &[
                ("figure", Vec3::ZERO),
                ("hugged wall", Vec3::new(s * 0.5, 0.0, 0.0)),
                ("corridor ahead", Vec3::new(0.0, 0.0, s * 2.0)),
                ("corridor around the wall", Vec3::new(s, 0.0, s * 2.0)),
            ],
        );
    }

    #[test]
    fn every_city_structure_builds_all_visual_layers() {
        for kind in [
            CityStructureKind::Barrier,
            CityStructureKind::GlassFin,
            CityStructureKind::Pylon,
        ] {
            let structure = city_structure(kind, true, 1);
            assert!(city_foundation_mesh(&structure).count_vertices() > 0);
            assert!(city_body_mesh(&structure).count_vertices() > 0);
            assert!(city_cap_mesh(&structure).count_vertices() > 0);
        }
    }

    #[test]
    fn city_height_tiers_and_silhouettes_are_distinct() {
        for kind in [
            CityStructureKind::Barrier,
            CityStructureKind::GlassFin,
            CityStructureKind::Pylon,
        ] {
            let low = city_structure(kind, true, 0);
            let tall = city_structure(kind, false, 2);
            assert!(city_body_height(&tall) > city_body_height(&low));
        }
    }

    /// The skirt only delineates the wall/floor seam if it is wider than the
    /// foundation it surrounds and sits flat on the ground.
    #[test]
    fn the_base_trim_outlines_the_footprint_at_ground_level() {
        for kind in [
            CityStructureKind::Barrier,
            CityStructureKind::GlassFin,
            CityStructureKind::Pylon,
        ] {
            let structure = city_structure(kind, true, 1);
            let trim = city_base_trim_mesh(&structure).compute_aabb().unwrap();
            let foundation = city_foundation_mesh(&structure).compute_aabb().unwrap();

            assert!(trim.half_extents.x > foundation.half_extents.x);
            assert!(trim.half_extents.z > foundation.half_extents.z);
            assert!(
                trim.max().y < foundation.max().y,
                "trim should hug the floor, not cover the foundation"
            );
            assert!((trim.min().y).abs() < 1e-5, "trim must start at the ground");
        }
    }

    #[test]
    fn road_markings_join_neighboring_cells() {
        let isolated = BTreeSet::from([(0, 0)]);
        let connected = BTreeSet::from([(0, 0), (1, 0), (0, 1)]);
        assert!(
            road_marking_mesh((0, 0), &connected).count_vertices()
                > road_marking_mesh((0, 0), &isolated).count_vertices()
        );
    }

    /// Ground seams must never be drawn in the same color as a lane marking.
    #[test]
    fn ground_seams_contrast_with_lane_markings_in_every_theme() {
        let palette = city_palette();
        for theme in palette {
            assert_ne!(theme[CITY_TRIM_ACCENT].0, theme[0].0);
        }
    }

    #[test]
    fn every_theme_maps_to_a_two_color_neon_palette() {
        let palette = city_palette();
        for (index, theme) in [
            CityTheme::Cyan,
            CityTheme::Magenta,
            CityTheme::Violet,
            CityTheme::Amber,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(city_theme_index(theme), index);
            assert_ne!(palette[index][0].0, palette[index][1].0);
        }
    }

    #[test]
    fn corner_arc_runs_from_the_incoming_heading_to_the_outgoing_one() {
        let entry = right_corner(0.0);
        let exit = right_corner(1.0);

        assert!((entry.direction - Vec2::new(1.0, 0.0)).length() < 1e-5);
        assert!((exit.direction - Vec2::new(0.0, 1.0)).length() < 1e-5);

        // The arc starts `RADIUS` short of the corner and ends `RADIUS` past it.
        assert!((entry.position.0 - (1.0 - RADIUS)).abs() < 1e-5);
        assert!(entry.position.1.abs() < 1e-5);
        assert!((exit.position.0 - 1.0).abs() < 1e-5);
        assert!((exit.position.1 - RADIUS).abs() < 1e-5);

        // Upright at both ends so straight segments join without a pop.
        assert!(entry.lean.abs() < 1e-5);
        assert!(exit.lean.abs() < 1e-5);
    }

    #[test]
    fn corner_pose_is_continuous_across_the_cell_boundary() {
        let mut before = LightcycleSim::start((0, 0), Heading::PosX);
        before.queue_turn(Turn::Right);
        before.cell_t = 1.0;

        // The simulation applies the turn at the boundary and starts the next cell.
        let mut after = LightcycleSim::start((1, 0), Heading::PosZ);
        after.trail.push((0, 0));

        let before = cycle_cell_pose(&before);
        let after = cycle_cell_pose(&after);

        assert!((before.position.0 - after.position.0).abs() < 1e-5);
        assert!((before.position.1 - after.position.1).abs() < 1e-5);
        assert!((before.direction - after.direction).length() < 1e-5);
        assert!((before.lean - after.lean).abs() < 1e-5);
    }

    #[test]
    fn the_trail_stops_at_the_cycle_tail_not_the_cell_center() {
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);
        sim.cell_t = 0.7;
        let pose = cycle_cell_pose(&sim);
        let points = trail_centerline(&sim);
        let tail = *points.last().unwrap();

        let expected = (
            pose.position.0 - pose.direction.x * config::LIGHTCYCLE_TRAIL_TAIL,
            pose.position.1 - pose.direction.y * config::LIGHTCYCLE_TRAIL_TAIL,
        );
        assert!((tail.0 - expected.0).abs() < 1e-4);
        assert!((tail.1 - expected.1).abs() < 1e-4);
        assert!(
            (tail.0 - pose.position.0).abs() > 0.2,
            "trail still ends at the bike origin"
        );
    }

    #[test]
    fn the_live_trail_follows_the_same_arc_as_the_cycle() {
        // Second half of a right turn: the tail has already entered the corner,
        // so the ribbon itself must leave the incoming axis.
        let mut sim = LightcycleSim::start((1, 0), Heading::PosZ);
        sim.trail.push((0, 0));
        sim.cell_t = 0.35;
        let points = trail_centerline(&sim);

        let leaves_axis = points.windows(2).any(|pair| {
            let dx = (pair[1].0 - pair[0].0).abs();
            let dz = (pair[1].1 - pair[0].1).abs();
            dx > 1e-4 && dz > 1e-4
        });
        assert!(
            leaves_axis,
            "trail stayed axis-aligned through a corner: {points:?}"
        );
    }

    #[test]
    fn the_trail_is_a_meniscus_at_the_tail_and_full_height_behind_it() {
        let points = vec![(0.0, 0.0), (2.0, 0.0)];
        let heights = trail_heights(&points);
        assert!(
            heights[0] > heights[1],
            "oldest point should be full height"
        );
        assert!((heights[0] - config::LIGHTCYCLE_TRAIL_HEIGHT).abs() < 1e-4);
        assert!((heights[1] - config::LIGHTCYCLE_TRAIL_SPAWN_HEIGHT).abs() < 1e-4);
    }

    /// A zero-vertex mesh makes Bevy's allocator skip the allocation but still
    /// run the upload, logging a use-after-free every frame.
    #[test]
    fn the_trail_mesh_always_has_vertices() {
        let mut fresh = LightcycleSim::start((0, 0), Heading::PosX);
        assert!(
            build_trail_mesh(&fresh).count_vertices() > 0,
            "a freshly spawned run has no ribbon yet"
        );

        // The whole stretch where the tail has not yet cleared its spawn cell.
        for step in 0..20 {
            fresh.cell_t = step as f32 / 20.0;
            assert!(
                build_trail_mesh(&fresh).count_vertices() > 0,
                "empty mesh at cell_t {}",
                fresh.cell_t
            );
        }

        let mut riding = LightcycleSim::start((2, 0), Heading::PosX);
        riding.trail.extend([(0, 0), (1, 0)]);
        riding.cell_t = 0.5;
        assert!(build_trail_mesh(&riding).count_vertices() > 0);
    }

    #[test]
    fn trimming_the_polyline_end_shortens_it_by_the_asked_distance() {
        let points = trim_polyline_end(vec![(0.0, 0.0), (1.0, 0.0)], 0.25);
        assert_eq!(points.len(), 2);
        assert!((points[1].0 - 0.75).abs() < 1e-5);
        assert!(trim_polyline_end(vec![(0.0, 0.0), (0.1, 0.0)], 0.4).len() < 2);
    }

    #[test]
    fn a_wall_without_a_gate_is_one_unbroken_rail() {
        assert_eq!(rail_segments(-5.0, 5.0, None), vec![(-5.0, 5.0)]);
    }

    #[test]
    fn a_gate_splits_its_wall_into_the_rails_on_either_side() {
        assert_eq!(
            rail_segments(-5.0, 5.0, Some((-1.0, 2.0))),
            vec![(-5.0, -1.0), (2.0, 5.0)]
        );
    }

    #[test]
    fn a_gate_at_a_wall_corner_drops_the_empty_side() {
        assert_eq!(
            rail_segments(-5.0, 5.0, Some((-5.0, -2.0))),
            vec![(-2.0, 5.0)]
        );
        assert_eq!(
            rail_segments(-5.0, 5.0, Some((2.0, 5.0))),
            vec![(-5.0, 2.0)]
        );
    }

    #[test]
    fn a_gate_spanning_the_whole_wall_leaves_no_rail() {
        assert!(rail_segments(-5.0, 5.0, Some((-5.0, 5.0))).is_empty());
    }

    #[test]
    fn gate_frame_pulses_between_its_trough_and_peak() {
        let samples: Vec<_> = (0..64).map(|step| gate_pulse(step as f32 * 0.05)).collect();

        assert!(samples.iter().all(|value| (0.0..=1.0).contains(value)));
        assert!(samples.iter().any(|value| *value > 0.9), "never brightens");
        assert!(samples.iter().any(|value| *value < 0.1), "never dims");
    }

    #[test]
    fn gate_bars_sweep_up_the_opening_and_wrap_at_the_lintel() {
        let travel = config::LIGHTCYCLE_PORTAL_HEIGHT;
        let bar = GateScanBar {
            offset: 0.0,
            travel,
        };

        // Long enough to cover more than one full sweep of the opening.
        let steps = (2.5 / config::LIGHTCYCLE_PORTAL_BAR_SPEED / 0.05) as usize;
        let heights: Vec<_> = (0..steps)
            .map(|step| gate_bar_height(&bar, step as f32 * 0.05))
            .collect();

        assert!(heights.iter().all(|height| (0.0..=travel).contains(height)));
        assert!(heights[1] > heights[0], "bars should rise, not fall");
        assert!(
            heights.windows(2).any(|pair| pair[1] < pair[0]),
            "bars should wrap back to the ground"
        );
    }

    #[test]
    fn gate_bars_stay_evenly_spaced_up_the_opening() {
        let travel = config::LIGHTCYCLE_PORTAL_HEIGHT;
        let count = config::LIGHTCYCLE_PORTAL_BAR_COUNT;
        let bars: Vec<_> = (0..count)
            .map(|index| GateScanBar {
                offset: index as f32 / count as f32,
                travel,
            })
            .collect();

        let mut heights: Vec<_> = bars.iter().map(|bar| gate_bar_height(bar, 0.37)).collect();
        heights.sort_by(f32::total_cmp);

        let expected = travel / count as f32;
        for pair in heights.windows(2) {
            assert!((pair[1] - pair[0] - expected).abs() < 1e-4);
        }
    }

    /// The rendered cycle must point along its travel direction and stay
    /// upright in every heading, including the one antipodal to the model's
    /// reference axis.
    #[test]
    fn cycle_faces_travel_direction_and_stays_upright_in_every_heading() {
        for heading in [Heading::PosX, Heading::NegX, Heading::PosZ, Heading::NegZ] {
            let (dx, dz) = heading.delta();
            let pose = super::CyclePose {
                position: (0.0, 0.0),
                direction: Vec2::new(dx as f32, dz as f32),
                lean: 0.0,
            };
            let rotation = pose_rotation(&pose);
            let model_yaw = bevy::prelude::Quat::from_rotation_y(config::LIGHTCYCLE_MODEL_YAW);

            let travel = super::pose_forward(&pose);
            let nose = rotation * model_yaw * MODEL_NOSE_AXIS;
            assert!(
                nose.dot(travel) > 0.99,
                "{heading:?}: nose {nose:?} should point along travel {travel:?}"
            );

            let up = rotation * model_yaw * Vec3::Y;
            assert!(
                up.y > 0.99,
                "{heading:?}: cycle should stay upright, got {up:?}"
            );
        }
    }

    #[test]
    fn cycle_banks_toward_the_inside_of_the_corner() {
        let right_up = pose_rotation(&right_corner(0.5)) * Vec3::Y;
        let left_up = pose_rotation(&arc_cell_pose((1, 0), (1, 0), (0, -1), 0.5, RADIUS)) * Vec3::Y;

        assert!(right_up.z > 0.1, "a right turn should bank toward +Z");
        assert!(left_up.z < -0.1, "a left turn should bank toward -Z");
    }

    fn heading_block(along_x: bool, preview: &str) -> crate::document::PlacedBlock {
        crate::document::PlacedBlock {
            kind: crate::document::DocBlockKind::Heading(1),
            text: preview.to_string(),
            preview: preview.to_string(),
            spine: vec![(0, 0)],
            walls: vec![],
            landmark: (0, 0),
            along_x,
        }
    }

    #[test]
    fn heading_glyph_meshes_are_non_empty() {
        let (mesh, used) = super::document_glyph_line_mesh(&heading_block(true, "Title"), 24);
        assert!(used > 0);
        assert!(mesh.count_vertices() > 0);
    }

    #[test]
    fn glyph_budget_caps_characters_per_line_and_overall() {
        let long = "A".repeat(80);
        let heading = heading_block(true, &long);
        let (_, used) = super::document_glyph_line_mesh(&heading, config::DOCUMENT_MAX_GLYPHS);
        assert_eq!(used, config::DOCUMENT_HEADING_GLYPHS);

        let paragraph = crate::document::PlacedBlock {
            kind: crate::document::DocBlockKind::Paragraph,
            text: long.clone(),
            preview: long,
            spine: vec![(0, 0)],
            walls: vec![],
            landmark: (1, 0),
            along_x: true,
        };
        let (_, used) = super::document_glyph_line_mesh(&paragraph, config::DOCUMENT_MAX_GLYPHS);
        assert_eq!(used, config::DOCUMENT_PARAGRAPH_GLYPHS);

        let leftover = super::document_glyph_line_mesh(&heading, 3).1;
        assert_eq!(leftover, 3);
    }

    #[test]
    fn heading_glyphs_follow_block_orientation() {
        let along_x = super::document_glyph_line_mesh(&heading_block(true, "HEADING"), 24)
            .0
            .compute_aabb()
            .unwrap();
        let along_z = super::document_glyph_line_mesh(&heading_block(false, "HEADING"), 24)
            .0
            .compute_aabb()
            .unwrap();
        assert!(along_x.half_extents.x > along_x.half_extents.z);
        assert!(along_z.half_extents.z > along_z.half_extents.x);
    }

    /// A glyph's columns and its line's characters have to run the same way. When
    /// they disagreed the text rendered mirrored from one side of the page and in
    /// reverse character order from the other.
    #[test]
    fn glyph_columns_read_the_same_way_as_the_characters() {
        for along_x in [true, false] {
            let advance = document_line_advance(along_x);
            let pixel = 0.09;

            let next_char =
                glyph_char_offset(advance, 1, 3, pixel) - glyph_char_offset(advance, 0, 3, pixel);
            let next_column =
                glyph_pixel_offset(advance, 7, 0, pixel) - glyph_pixel_offset(advance, 0, 0, pixel);
            assert!(next_char.dot(advance) > 0.0, "characters must read forward");
            assert!(next_column.dot(advance) > 0.0, "columns must read forward");
            assert!(
                next_char.dot(advance) > next_column.dot(advance),
                "one character has to advance further than one glyph is wide"
            );

            // Row 0 is the top of the glyph, so it must sit highest.
            let top = glyph_pixel_offset(advance, 0, 0, pixel);
            let bottom = glyph_pixel_offset(advance, 0, 7, pixel);
            assert!(top.y > bottom.y);
        }
    }

    /// Text reads toward screen right for a reader standing on the open side of a
    /// paragraph's ink wall: `+X` seen from `+Z`, and `-Z` seen from `+X`.
    #[test]
    fn text_reads_from_the_side_its_wall_leaves_open() {
        for (along_x, viewer_forward, expected) in [
            (true, Vec3::NEG_Z, Vec3::X),
            (false, Vec3::NEG_X, Vec3::NEG_Z),
        ] {
            let advance = document_line_advance(along_x);
            assert_eq!(advance, expected);
            assert!(
                viewer_forward.cross(Vec3::Y).abs_diff_eq(advance, 1e-6),
                "reading direction must be screen right for that viewpoint"
            );
        }
    }

    /// [`glyph_pixel_offset`] maps column 0 to the left of the letter, which only
    /// holds while font8x8 packs rows least-significant bit first. 'F' pins the
    /// order down: its top bar runs from the left edge and stops short of the
    /// right, so bit 0 is set and bit 7 is not. Under the opposite convention
    /// both would flip and every glyph would render mirrored.
    #[test]
    fn font_rows_pack_the_leftmost_pixel_in_the_lowest_bit() {
        let top_bar = glyph_pixels('F')[0];
        assert!(top_bar & 1 != 0, "the top bar must start at bit 0");
        assert!(top_bar & (1 << 7) == 0, "and stop short of bit 7");
    }

    #[test]
    fn page_rules_span_the_document_arena() {
        let (arena, _) = crate::document::layout::build_document_arena(
            std::path::Path::new("/docs/page.md"),
            "# A\n\nB\n",
        );
        let aabb = super::document_rule_mesh(&arena)
            .unwrap()
            .compute_aabb()
            .unwrap();
        let spacing = config::GRID_SPACING;
        assert!(aabb.half_extents.x * 2.0 >= (arena.max.0 - arena.min.0) as f32 * spacing * 0.9);
        assert!(aabb.half_extents.z * 2.0 >= (arena.max.1 - arena.min.1) as f32 * spacing * 0.9);
    }

    #[test]
    fn directory_and_document_palettes_and_portals_differ() {
        assert_ne!(
            config::DOCUMENT_FLOOR_COLOR,
            config::LIGHTCYCLE_CITY_FLOOR_COLOR
        );
        assert_ne!(
            config::DOCUMENT_FOLIO_COLOR,
            config::LIGHTCYCLE_PORTAL_COLOR
        );
        assert_ne!(config::MARKDOWN_TOWER_COLOR, config::FILE_COLOR);
        assert_ne!(
            config::DOCUMENT_INK_COLOR,
            config::LIGHTCYCLE_CITY_FLOOR_COLOR
        );
    }

    #[test]
    fn document_arenas_have_no_city_skyline() {
        let run =
            super::build_document_run(std::path::Path::new("/tmp/note.md"), b"# Hi\n\nHello\n");
        assert_eq!(
            run.arena.kind,
            crate::lightcycle::logic::ArenaKind::Document
        );
        assert!(run.arena.structures.is_empty());
        assert!(!run.arena.roads.is_empty());
        assert!(run.is_document());
    }

    #[test]
    fn closing_a_document_can_rebuild_the_containing_directory() {
        let path = std::path::PathBuf::from("/tmp");
        let nodes = vec![crate::filesystem::FileNode::new(
            "note.md".into(),
            path.join("note.md"),
            false,
            12,
            0,
        )];
        let directory = super::build_active_run(&path, nodes.clone());
        let document = super::build_document_run(&nodes[0].path, b"# Hi\n");
        assert!(document.is_document());
        assert!(!directory.is_document());
        assert_eq!(
            directory.arena.kind,
            crate::lightcycle::logic::ArenaKind::Directory
        );
        let restored = super::build_active_run(&path, nodes);
        assert_eq!(restored.arena, directory.arena);
    }

    #[test]
    fn an_unrotated_chase_rig_sits_behind_and_above_the_cycle() {
        let (offset, view_forward) = chase_camera_rig(Vec3::X, Vec2::ZERO);
        assert!(view_forward.abs_diff_eq(Vec3::X, 1e-5));
        assert!(
            offset.abs_diff_eq(
                Vec3::new(
                    -config::LIGHTCYCLE_CAMERA_DISTANCE,
                    config::LIGHTCYCLE_CAMERA_HEIGHT,
                    0.0
                ),
                1e-4
            ),
            "free look at rest must reproduce the fixed rig, got {offset}"
        );
    }

    /// Entering the lightcycle flies the camera to a rig that `update_chase_camera`
    /// then holds on its own, and dives in along the road the cycle is about to
    /// ride. Landing anywhere else would pop on the first frame of the run.
    #[test]
    fn the_flight_lands_on_the_rig_the_chase_camera_will_hold() {
        let path = std::path::PathBuf::from("/tmp");
        let nodes = vec![crate::filesystem::FileNode::new(
            "a.txt".into(),
            path.join("a.txt"),
            false,
            12,
            0,
        )];
        let run = super::build_active_run(&path, nodes);
        let (landing, focus, road) = super::chase_landing_pose(&run);
        let cycle = pose_world_position(&cycle_cell_pose(&run.sim));

        assert!((landing.translation.distance(cycle) - chase_rig_radius()).abs() < 1e-4);
        assert!((landing.translation.y - config::LIGHTCYCLE_CAMERA_HEIGHT).abs() < 1e-4);
        assert!(
            (landing.rotation * Vec3::NEG_Z)
                .abs_diff_eq((focus - landing.translation).normalize(), 1e-5)
        );

        // The road is the cycle's own heading, which the overhead shot leans on
        // for its roll, so it has to be a unit vector along the ground.
        assert!(road.abs_diff_eq(pose_forward(&cycle_cell_pose(&run.sim)), 1e-5));
        assert!((road.length() - 1.0).abs() < 1e-5 && road.y.abs() < 1e-5);
        assert!(
            focus.abs_diff_eq(cycle + road * config::LIGHTCYCLE_CAMERA_LOOKAHEAD, 1e-4),
            "the shot has to be aimed down the road ahead of the cycle"
        );
    }

    #[test]
    fn free_look_yaw_orbits_the_cycle_at_a_constant_radius_and_height() {
        let (rest, _) = chase_camera_rig(Vec3::X, Vec2::ZERO);
        for steps in 1..8 {
            let yaw = steps as f32 * 0.7;
            let (offset, view_forward) = chase_camera_rig(Vec3::X, Vec2::new(yaw, 0.0));
            assert!((offset.length() - chase_rig_radius()).abs() < 1e-3);
            assert!(
                (offset.y - rest.y).abs() < 1e-4,
                "yaw must not change height"
            );
            assert!((view_forward.length() - 1.0).abs() < 1e-4);
        }
    }

    /// A drag that runs past the pitch limits must leave the camera above the
    /// arena floor and still looking at the cycle rather than straight down it.
    #[test]
    fn free_look_pitch_stays_within_its_limits() {
        let mut chase = ChaseCamera {
            forward: Vec3::X,
            look: Vec2::ZERO,
        };

        for _ in 0..200 {
            chase.apply_look_drag(Vec2::new(0.0, -50.0));
        }
        let (up, _) = chase_camera_rig(chase.forward, chase.look);
        assert!(up.y > 0.0 && up.y < chase_rig_radius());

        for _ in 0..400 {
            chase.apply_look_drag(Vec2::new(0.0, 50.0));
        }
        let (down, _) = chase_camera_rig(chase.forward, chase.look);
        assert!(down.y > 0.0, "the camera must not drop below the floor");

        // One frame back the other way has to move the camera immediately, not
        // spend itself unwinding rotation banked up past the limit.
        chase.apply_look_drag(Vec2::new(0.0, -20.0));
        let (recovered, _) = chase_camera_rig(chase.forward, chase.look);
        assert!(recovered.y > down.y);
    }

    #[test]
    fn releasing_the_button_settles_free_look_back_behind_the_cycle() {
        let mut chase = ChaseCamera {
            forward: Vec3::X,
            look: Vec2::ZERO,
        };
        chase.apply_look_drag(Vec2::new(-90.0, -40.0));
        let dragged = chase.look;
        assert_ne!(dragged, Vec2::ZERO);

        // Both axes have to ease toward the default rig, not just shrink overall.
        chase.recenter_look(1.0 / 60.0);
        assert!(chase.look.x.abs() < dragged.x.abs());
        assert!(chase.look.y.abs() < dragged.y.abs());

        // The ease has to land exactly home rather than trail an ever-smaller
        // remainder, and it has to get there in a settling time a rider would
        // read as prompt.
        let mut frames = 1;
        while chase.look != Vec2::ZERO {
            assert!(frames < 60 * 4, "free look never settled");
            chase.recenter_look(1.0 / 60.0);
            frames += 1;
        }
        let (offset, view_forward) = chase_camera_rig(chase.forward, chase.look);
        assert!(view_forward.abs_diff_eq(Vec3::X, 1e-5));
        assert!(offset.abs_diff_eq(
            Vec3::new(
                -config::LIGHTCYCLE_CAMERA_DISTANCE,
                config::LIGHTCYCLE_CAMERA_HEIGHT,
                0.0
            ),
            1e-4
        ));
    }

    /// Spinning the camera several turns must still unwind the short way, rather
    /// than rewinding every revolution the drag wound on.
    #[test]
    fn free_look_yaw_wraps_instead_of_accumulating_revolutions() {
        let mut chase = ChaseCamera {
            forward: Vec3::X,
            look: Vec2::ZERO,
        };
        for _ in 0..300 {
            chase.apply_look_drag(Vec2::new(-25.0, 0.0));
        }
        assert!(chase.look.x.abs() <= std::f32::consts::PI);

        for angle in [-9.0, -3.5, 0.0, 3.5, 9.0] {
            let wrapped = wrap_angle(angle);
            assert!((-std::f32::consts::PI..std::f32::consts::PI).contains(&wrapped));
            let turns = (angle - wrapped) / std::f32::consts::TAU;
            assert!(
                (turns - turns.round()).abs() < 1e-5,
                "wrapping must only remove whole turns"
            );
        }
    }

    #[test]
    fn entry_beam_rises_brightly_then_collapses() {
        assert_eq!(entry_effect_envelope(0.0), 0.0);
        assert_eq!(entry_effect_envelope(1.0), 0.0);
        assert!(entry_effect_envelope(0.18) > 0.99);
        assert!(entry_effect_envelope(0.08) < entry_effect_envelope(0.18));
        assert!(entry_effect_envelope(0.7) < entry_effect_envelope(0.35));
    }

    #[test]
    fn entry_halos_sweep_up_the_beam_at_staggered_heights() {
        let progress = 0.25;
        let poses: Vec<_> = (0..config::LIGHTCYCLE_ENTRY_HALO_COUNT)
            .map(|index| {
                entry_halo_pose(
                    progress,
                    index as f32 / config::LIGHTCYCLE_ENTRY_HALO_COUNT as f32,
                )
            })
            .collect();

        assert!(poses.iter().all(|(height, scale)| {
            *height >= 0.35
                && *height <= config::LIGHTCYCLE_ENTRY_HALO_HEIGHT + 0.35
                && *scale > 0.0
        }));
        assert!(
            poses
                .windows(2)
                .any(|pair| (pair[0].0 - pair[1].0).abs() > 0.5),
            "halos should not collapse into one ring"
        );
    }

    #[test]
    fn directory_request_waits_until_near_the_transport_apex() {
        let mut fx = crate::lightcycle::EntryFx::new(
            std::path::PathBuf::from("/next"),
            config::LIGHTCYCLE_ENTRY_FX_DURATION,
        );
        fx.elapsed = fx.duration * 0.5;
        assert!(fx.progress() < config::LIGHTCYCLE_ENTRY_FX_REQUEST_AT);
        fx.elapsed = fx.duration * 0.9;
        assert!(fx.progress() >= config::LIGHTCYCLE_ENTRY_FX_REQUEST_AT);
    }

    #[test]
    fn the_field_facing_round_trips_through_the_grid_headings() {
        for heading in [Heading::PosX, Heading::PosZ, Heading::NegX, Heading::NegZ] {
            assert_eq!(nearest_heading(heading_facing(heading)), heading);
        }
        // Near a diagonal the nearest cardinal leans toward the dominant axis.
        assert_eq!(
            nearest_heading(std::f32::consts::FRAC_PI_4 * 0.9),
            Heading::PosX
        );
        assert_eq!(
            nearest_heading(std::f32::consts::FRAC_PI_4 * 1.1),
            Heading::PosZ
        );
    }

    /// Each language must build only its own sim, so the field-only behaviour
    /// (parked bike, overhead camera, pivot input) can never leak into another
    /// ring. This is the regression test for the overhead camera that once
    /// appeared in disc wars.
    #[test]
    fn each_source_language_builds_only_its_own_game() {
        use crate::disc::SourceLanguage;
        let run = |name: &str, language: SourceLanguage, body: &[u8]| {
            super::build_source_run(std::path::Path::new(name), language, body)
        };

        let field = run("/tmp/field.c", SourceLanguage::C, b"int main(void) {}\n");
        assert!(field.source_asteroids().is_some());
        assert!(field.source_disc().is_none() && field.source_snake().is_none());
        assert!(field.asteroid_field_active());
        assert!(
            super::field_camera_focus(&field).is_some(),
            "the field plays from above"
        );

        let ring = run("/tmp/ring.rs", SourceLanguage::Rust, b"fn main() {}\n");
        assert!(ring.source_disc().is_some());
        assert!(ring.source_asteroids().is_none() && ring.source_snake().is_none());
        assert!(!ring.asteroid_field_active());
        assert!(
            super::field_camera_focus(&ring).is_none(),
            "a disc-wars ring must keep the chase camera"
        );

        let snake = run("/tmp/snake.py", SourceLanguage::Python, b"print('hi')\n");
        assert!(snake.source_snake().is_some());
        assert!(snake.source_disc().is_none() && snake.source_asteroids().is_none());
        assert!(!snake.asteroid_field_active());
        assert!(
            super::field_camera_focus(&snake).is_none(),
            "snake drives on the grid, so it keeps the chase camera"
        );
        let snake = snake.source_snake().expect("snake state");
        assert_eq!(snake.food.len(), config::SNAKE_FOOD_TARGET);
        assert!(!snake.exit_open, "the exit starts locked");

        // The three games that build their own space off the grid.
        let level = run(
            "/tmp/run.slint",
            SourceLanguage::Slint,
            b"export component App {}\n",
        );
        assert!(level.source_platformer().is_some());
        assert!(!level.asteroid_field_active());
        assert!(
            level.source_platformer().expect("level").platforms.len() > 2,
            "a level should have platforms to run"
        );

        let court = run("/tmp/init.lua", SourceLanguage::Lua, b"local x = 1\n");
        assert!(court.source_breaker().is_some());
        assert!(court.source_breaker().expect("court").remaining() > 0);

        let room = run("/tmp/build.sh", SourceLanguage::Shell, b"set -e\n");
        assert!(room.source_stealth().is_some());
        assert!(!room.source_stealth().expect("room").guards.is_empty());
    }

    #[test]
    fn a_locked_snake_gate_is_a_wall_until_it_opens() {
        let run = super::build_source_run(
            std::path::Path::new("/tmp/snake.py"),
            crate::disc::SourceLanguage::Python,
            b"print('hi')\n",
        );
        let portal = run.arena.parent_portal.as_ref().expect("a close gate");
        assert!(
            super::is_ring_gate(&run.arena, portal.to),
            "the portal cell is the gate"
        );
        assert!(
            !run.source_snake().expect("snake state").exit_open,
            "so the gate is solid at the start of the run"
        );
    }
}
