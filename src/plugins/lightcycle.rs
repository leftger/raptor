use crate::config;
use crate::document::{
    DocumentLayout, DocumentLoadFailed, DocumentLoadState, DocumentLoaded, DocumentRequested,
    build_document_arena_from_parse,
    parse::{ParseLimits, parse_markdown_bytes},
};
use crate::filesystem::FileNode;
use crate::lightcycle::logic::{
    Arena, ArenaKind, CityStructure, CityStructureKind, CityTheme, CrashReason, GatePlacement,
    Heading, LightcycleSim, ParentPortal, RunPhase, StepOutcome, Wall, classify_next_content,
};
use crate::lightcycle::{ActiveRun, LightcycleState, RunEnvironment};
use crate::load::{DirectoryLoadFailed, DirectoryLoaded, DirectoryRequested};
use crate::state::{
    DirectorySceneRoot, InteractionMode, LightcycleSceneRoot, NavigatorResource,
    OrbitCameraResource, TrailSceneRoot,
};
use bevy::asset::RenderAssetUsages;
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
            .add_systems(Startup, setup_lightcycle_assets)
            .add_systems(
                Update,
                (
                    toggle_mode,
                    reset_on_directory_loaded,
                    start_document_loads,
                    poll_document_loads,
                    reset_on_document_loaded,
                    apply_load_failure,
                    apply_document_load_failure,
                    sync_directory_scene_visibility,
                    read_lightcycle_input.run_if(in_lightcycle_mode),
                    step_lightcycle.run_if(in_lightcycle_mode),
                    restore_directory_arena.run_if(in_lightcycle_mode),
                    spawn_crash_effect.run_if(in_lightcycle_mode),
                    update_crash_effects.run_if(in_lightcycle_mode),
                    update_trail_mesh.run_if(in_lightcycle_mode),
                    animate_parent_gate.run_if(in_lightcycle_mode),
                    animate_city_beacons.run_if(in_lightcycle_mode),
                    update_document_focus.run_if(in_lightcycle_mode),
                    update_cycle_transform.run_if(in_lightcycle_mode),
                    update_chase_camera.run_if(in_lightcycle_mode),
                )
                    .chain()
                    .after(crate::plugins::filesystem::apply_loaded),
            );
    }
}

#[derive(Resource)]
struct LightcycleAssets {
    unit_cube: Handle<Mesh>,
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
    document_floor_material: Handle<StandardMaterial>,
    document_rule_material: Handle<StandardMaterial>,
    document_margin_material: Handle<StandardMaterial>,
    document_ink_material: Handle<StandardMaterial>,
    document_heading_material: Handle<StandardMaterial>,
    document_folio_material: Handle<StandardMaterial>,
    document_focus_material: Handle<StandardMaterial>,
    crash_material: Handle<StandardMaterial>,
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

/// Direction the chase camera is currently following.
///
/// This trails the cycle's own heading so a corner reads as the cycle swinging
/// across the frame. Locking the camera to the cycle instead makes the world
/// appear to rotate around a stationary bike. It lives on the cycle so each run
/// starts from the spawn heading.
#[derive(Component)]
struct ChaseCamera {
    forward: Vec3,
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

fn spawn_sim(arena: &Arena, cells: &HashMap<(i32, i32), usize>) -> LightcycleSim {
    let Some(spawn) = arena.nearest_empty_cell(
        |cell| cells.contains_key(&cell) || arena.street_walls.contains(&cell),
        config::LIGHTCYCLE_SPAWN_SEARCH_RADIUS,
    ) else {
        return LightcycleSim::ready(arena.center(), Heading::PosX);
    };

    let heading = Heading::initial_heading(spawn, |cell| {
        arena.contains(cell) && !cells.contains_key(&cell) && !arena.street_walls.contains(&cell)
    });

    match heading {
        Some(heading) => LightcycleSim::start(spawn, heading),
        None => LightcycleSim::ready(spawn, Heading::PosX),
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn toggle_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<InteractionMode>,
    mut state: ResMut<LightcycleState>,
    navigator: Res<NavigatorResource>,
    mut orbit: ResMut<OrbitCameraResource>,
    assets: Res<LightcycleAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
    old_lightcycle_entities: Query<Entity, Or<(With<LightcycleSceneRoot>, With<TrailSceneRoot>)>>,
) {
    if !keys.just_pressed(KeyCode::KeyM) {
        return;
    }

    despawn_lightcycle_entities(&mut commands, &old_lightcycle_entities);
    state.clock = 0.0;
    state.run = None;
    state.crash_fx = None;
    state.restore_directory = false;

    if *mode == InteractionMode::Lightcycle {
        *mode = InteractionMode::Explorer;
        orbit.reset_target();
        return;
    }

    let run = build_active_run(&navigator.0.current_path, navigator.0.entries.clone());
    spawn_run_entities(&mut commands, &assets, &mut meshes, &run);
    state.run = Some(run);
    *mode = InteractionMode::Lightcycle;
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
    let material = if arena.kind == ArenaKind::Document {
        assets.document_floor_material.clone()
    } else {
        assets.city_floor_material.clone()
    };
    commands.spawn((
        LightcycleSceneRoot,
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material),
        Pickable::IGNORE,
    ));
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
    let forward = if block.along_x { Vec3::X } else { Vec3::Z };
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
        let offset = forward * ((index as f32 - (chars.len() as f32 - 1.0) * 0.5) * pixel * 9.0);
        for (row, row_bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if row_bits & (1 << col) == 0 {
                    continue;
                }
                let local = Vec3::new(
                    if block.along_x {
                        (7 - col) as f32 * pixel
                    } else {
                        0.0
                    },
                    (7 - row) as f32 * pixel,
                    if block.along_x {
                        0.0
                    } else {
                        (7 - col) as f32 * pixel
                    },
                );
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
            |node: &FileNode| !node.is_dir && node.is_markdown(),
            assets.markdown_tower_material.clone(),
        ),
        (
            |node: &FileNode| !node.is_dir && !node.is_markdown(),
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
        state.restore_directory = false;
        state.run = Some(run);
    }
}

fn apply_load_failure(
    mut failed: MessageReader<DirectoryLoadFailed>,
    mut state: ResMut<LightcycleState>,
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

#[allow(clippy::too_many_arguments)]
fn read_lightcycle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<LightcycleState>,
    mut navigator: ResMut<NavigatorResource>,
    mut requests: MessageWriter<DirectoryRequested>,
) {
    let left = keys.just_pressed(KeyCode::KeyA) || keys.just_pressed(KeyCode::ArrowLeft);
    let right = keys.just_pressed(KeyCode::KeyD) || keys.just_pressed(KeyCode::ArrowRight);
    let restart = keys.just_pressed(KeyCode::KeyR);
    let go_up = keys.just_pressed(KeyCode::KeyU) || keys.just_pressed(KeyCode::Minus);

    let Some(mut run) = state.run.take() else {
        return;
    };

    if run.sim.phase == RunPhase::Running && (left || right) {
        run.sim.queue_turn_input(left, right);
    }

    if restart {
        restart_run(&mut run);
        state.clock = 0.0;
        state.crash_fx = None;
    }

    if go_up {
        if run.is_document() {
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
    let cells = match &run.environment {
        RunEnvironment::Directory { cells, .. } => cells.clone(),
        RunEnvironment::Document { .. } => HashMap::new(),
    };
    run.sim = spawn_sim(&run.arena, &cells);
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
    state.run = Some(run);
}

fn step_lightcycle(
    time: Res<Time>,
    mut state: ResMut<LightcycleState>,
    mut navigator: ResMut<NavigatorResource>,
    mut requests: MessageWriter<DirectoryRequested>,
    mut documents: MessageWriter<DocumentRequested>,
) {
    let Some(mut run) = state.run.take() else {
        return;
    };

    if run.sim.phase != RunPhase::Running {
        state.run = Some(run);
        return;
    }

    state.clock += time.delta_secs();
    let max_catch_up = config::LIGHTCYCLE_FIXED_STEP * config::LIGHTCYCLE_MAX_SUBSTEPS as f32;
    if state.clock > max_catch_up {
        state.clock = max_catch_up;
    }

    let fixed_step = config::LIGHTCYCLE_FIXED_STEP;
    let mut substeps = 0;

    while state.clock >= fixed_step && substeps < config::LIGHTCYCLE_MAX_SUBSTEPS {
        state.clock -= fixed_step;
        substeps += 1;

        let outcome = {
            let arena = &run.arena;
            let sim = &mut run.sim;
            match &run.environment {
                RunEnvironment::Directory { nodes, cells } => sim.advance(
                    fixed_step * config::LIGHTCYCLE_CELLS_PER_SEC,
                    |next, sim| {
                        classify_next_content(
                            next,
                            arena,
                            sim,
                            cells,
                            |index| nodes[index].is_dir,
                            |index| nodes[index].is_markdown(),
                        )
                    },
                ),
                RunEnvironment::Document { .. } => sim.advance(
                    fixed_step * config::LIGHTCYCLE_CELLS_PER_SEC,
                    |next, sim| {
                        classify_next_content(
                            next,
                            arena,
                            sim,
                            &HashMap::new(),
                            |_| false,
                            |_| false,
                        )
                    },
                ),
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
                    CrashReason::Wall if run.arena.street_walls.contains(&crash_cell) => {
                        if run.is_document() {
                            "paragraph".to_string()
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
                    navigator.0.begin_navigate_to(&path);
                    run.entering_label = Some(name);
                    run.crash_label = None;
                    requests.write(DirectoryRequested { path });
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
                    documents.write(DocumentRequested { path });
                } else {
                    run.sim.phase = RunPhase::Running;
                    run.crash_label = Some("missing document".to_string());
                }
            }
            StepOutcome::GoToParent => {
                if let Some(parent) = navigator.0.begin_go_to_parent() {
                    run.entering_label = Some("parent directory".to_string());
                    run.crash_label = None;
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

        if run.sim.phase == RunPhase::Crashed && state.crash_fx.is_none() {
            state.crash_fx = Some(crate::lightcycle::CrashFx::new(
                config::LIGHTCYCLE_CRASH_FX_DURATION,
            ));
        }

        if run.sim.phase != RunPhase::Running {
            break;
        }
    }

    state.run = Some(run);
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
    mut cycle: Query<&mut Transform, With<CycleEntity>>,
) {
    let Ok(mut transform) = cycle.single_mut() else {
        return;
    };
    let Some(run) = state.run.as_ref() else {
        return;
    };

    let pose = cycle_cell_pose(&run.sim);
    transform.translation = pose_world_position(&pose);
    transform.rotation = pose_rotation(&pose);
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

fn update_chase_camera(
    state: Res<LightcycleState>,
    time: Res<Time>,
    mut camera: Single<&mut Transform, (With<Camera3d>, Without<CycleEntity>)>,
    mut cycle: Query<(&Transform, &mut ChaseCamera), Without<Camera3d>>,
) {
    let Ok((cycle, mut chase)) = cycle.single_mut() else {
        return;
    };
    if state.run.is_none() {
        return;
    }

    let travel = cycle.rotation * Vec3::X;
    chase.forward = advance_chase_forward(
        chase.forward,
        Vec3::new(travel.x, 0.0, travel.z),
        time.delta_secs(),
    );

    let cycle_pos = cycle.translation;
    let forward = chase.forward;
    let look_target = cycle_pos + forward * config::LIGHTCYCLE_CAMERA_LOOKAHEAD;
    let mut camera_position = cycle_pos - forward * config::LIGHTCYCLE_CAMERA_DISTANCE
        + Vec3::Y * config::LIGHTCYCLE_CAMERA_HEIGHT;

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
        CITY_TRIM_ACCENT, GateScanBar, arc_cell_pose, build_trail_mesh, city_base_trim_mesh,
        city_body_height, city_body_mesh, city_cap_mesh, city_foundation_mesh, city_palette,
        city_theme_index, cycle_cell_pose, gate_bar_height, gate_pulse, pose_rotation,
        rail_segments, road_marking_mesh, trail_centerline, trail_heights, trim_polyline_end,
    };
    use crate::config;
    use crate::lightcycle::logic::{
        CityStructure, CityStructureKind, CityTheme, Heading, LightcycleSim, Turn,
    };
    use bevy::camera::primitives::MeshAabb;
    use bevy::prelude::{Vec2, Vec3};
    use std::collections::BTreeSet;

    const RADIUS: f32 = config::LIGHTCYCLE_TURN_RADIUS;

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
}
