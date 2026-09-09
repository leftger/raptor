use crate::config;
use crate::filesystem::FileNode;
use crate::lightcycle::logic::{
    Arena, CrashReason, Heading, LightcycleSim, RunPhase, StepOutcome, classify_next_content,
};
use crate::lightcycle::{ActiveRun, LightcycleState};
use crate::load::{DirectoryLoadFailed, DirectoryLoaded, DirectoryRequested};
use crate::state::{
    DirectorySceneRoot, InteractionMode, LightcycleSceneRoot, NavigatorResource,
    OrbitCameraResource, TrailSceneRoot,
};
use bevy::prelude::*;
use std::collections::HashMap;
use std::path::Path;

pub struct LightcyclePlugin;

impl Plugin for LightcyclePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InteractionMode>()
            .init_resource::<LightcycleState>()
            .add_systems(Startup, setup_lightcycle_assets)
            .add_systems(
                Update,
                (
                    toggle_mode,
                    reset_on_directory_loaded,
                    apply_load_failure,
                    sync_directory_scene_visibility,
                    read_lightcycle_input.run_if(in_lightcycle_mode),
                    step_lightcycle.run_if(in_lightcycle_mode),
                    spawn_crash_effect.run_if(in_lightcycle_mode),
                    update_crash_effects.run_if(in_lightcycle_mode),
                    rebuild_trail_mesh.run_if(in_lightcycle_mode),
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
    cycle_material: Handle<StandardMaterial>,
    trail_material: Handle<StandardMaterial>,
    wall_material: Handle<StandardMaterial>,
    portal_material: Handle<StandardMaterial>,
    dir_tower_material: Handle<StandardMaterial>,
    file_tower_material: Handle<StandardMaterial>,
    street_grid_material: Handle<StandardMaterial>,
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

/// Smooth visual rotation state for the cycle. The simulation heading still
/// changes at the cell boundary; this component eases the rendered yaw over a
/// short duration so the player can see where the turn happened.
#[derive(Component)]
struct CycleVisual {
    from_rotation: Quat,
    target_rotation: Quat,
    progress: f32,
}

fn unlit_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        unlit: true,
        ..default()
    }
}

fn setup_lightcycle_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(LightcycleAssets {
        unit_cube: meshes.add(Cuboid::default()),
        cycle_material: materials.add(unlit_material(config::LIGHTCYCLE_CYCLE_COLOR)),
        trail_material: materials.add(unlit_material(config::LIGHTCYCLE_TRAIL_COLOR)),
        wall_material: materials.add(unlit_material(config::LIGHTCYCLE_WALL_COLOR)),
        portal_material: materials.add(unlit_material(config::LIGHTCYCLE_PORTAL_COLOR)),
        dir_tower_material: materials.add(unlit_material(config::DIR_COLOR)),
        file_tower_material: materials.add(unlit_material(config::FILE_COLOR)),
        street_grid_material: materials.add(StandardMaterial {
            base_color: config::GRID_COLOR.with_alpha(0.3),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
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
    let arena = Arena::from_nodes(
        nodes.iter().map(|node| tower_position(node.grid_pos)),
        path.parent().is_some(),
        config::LIGHTCYCLE_ARENA_PADDING,
        config::LIGHTCYCLE_EMPTY_ARENA_HALF,
    );

    let cells: HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (tower_position(node.grid_pos), index))
        .collect();

    let sim = spawn_sim(&arena, &cells);

    ActiveRun {
        sim,
        arena,
        nodes,
        cells,
        crash_label: None,
        entering_label: None,
        trail_dirty: false,
    }
}

fn spawn_sim(arena: &Arena, cells: &HashMap<(i32, i32), usize>) -> LightcycleSim {
    let Some(spawn) = arena.nearest_empty_cell(
        |cell| cells.contains_key(&cell),
        config::LIGHTCYCLE_SPAWN_SEARCH_RADIUS,
    ) else {
        return LightcycleSim::ready(arena.center(), Heading::PosX);
    };

    let heading = Heading::initial_heading(spawn, |cell| {
        arena.contains(cell) && !cells.contains_key(&cell)
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
    let position = cycle_world_position(&run.sim);
    commands.spawn((
        LightcycleSceneRoot,
        CycleEntity,
        Mesh3d(assets.unit_cube.clone()),
        MeshMaterial3d(assets.cycle_material.clone()),
        Transform::from_translation(position).with_scale(Vec3::new(
            config::LIGHTCYCLE_CYCLE_SIZE,
            config::LIGHTCYCLE_CYCLE_HEIGHT,
            config::LIGHTCYCLE_CYCLE_SIZE,
        )),
        CycleVisual {
            from_rotation: heading_rotation(run.sim.heading),
            target_rotation: heading_rotation(run.sim.heading),
            progress: 1.0,
        },
        Pickable::IGNORE,
    ));

    spawn_arena_walls(commands, assets, &run.arena);
    spawn_towers(commands, assets, meshes, run);
    spawn_street_grid(commands, assets, meshes, &run.arena);
}

fn spawn_towers(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    run: &ActiveRun,
) {
    for is_dir in [true, false] {
        let material = if is_dir {
            assets.dir_tower_material.clone()
        } else {
            assets.file_tower_material.clone()
        };
        let matching: Vec<&FileNode> = run
            .nodes
            .iter()
            .filter(|node| node.is_dir == is_dir)
            .collect();

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

fn spawn_street_grid(
    commands: &mut Commands,
    assets: &LightcycleAssets,
    meshes: &mut Assets<Mesh>,
    arena: &Arena,
) {
    if let Some(mesh) = build_street_grid_mesh(arena) {
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(assets.street_grid_material.clone()),
            Pickable::IGNORE,
        ));
    }
}

fn build_street_grid_mesh(arena: &Arena) -> Option<Mesh> {
    let spacing = config::GRID_SPACING;
    let line_width = 0.05;
    let min_x = (arena.min.0 as f32 - 0.5) * spacing;
    let max_x = (arena.max.0 as f32 + 0.5) * spacing;
    let min_z = (arena.min.1 as f32 - 0.5) * spacing;
    let max_z = (arena.max.1 as f32 + 0.5) * spacing;
    let mid_x = (min_x + max_x) * 0.5;
    let mid_z = (min_z + max_z) * 0.5;
    let width = max_x - min_x;
    let depth = max_z - min_z;

    let mut merged: Option<Mesh> = None;
    let mut push = |mesh: Mesh| {
        if let Some(existing) = &mut merged {
            existing
                .merge(&mesh)
                .expect("street grid meshes must be merge-compatible");
        } else {
            merged = Some(mesh);
        }
    };

    for x in arena.min.0..=arena.max.0 {
        let center = Vec3::new(x as f32 * spacing, 0.01, mid_z);
        push(street_grid_line_mesh(
            center,
            Vec3::new(line_width, 0.02, depth),
        ));
    }
    for z in arena.min.1..=arena.max.1 {
        let center = Vec3::new(mid_x, 0.01, z as f32 * spacing);
        push(street_grid_line_mesh(
            center,
            Vec3::new(width, 0.02, line_width),
        ));
    }

    merged
}

fn street_grid_line_mesh(center: Vec3, scale: Vec3) -> Mesh {
    Mesh::from(Cuboid::default())
        .transformed_by(Transform::from_translation(center).with_scale(scale))
}

fn spawn_arena_walls(commands: &mut Commands, assets: &LightcycleAssets, arena: &Arena) {
    let spacing = config::GRID_SPACING;
    let min_x = (arena.min.0 as f32 - 0.5) * spacing;
    let max_x = (arena.max.0 as f32 + 0.5) * spacing;
    let min_z = (arena.min.1 as f32 - 0.5) * spacing;
    let max_z = (arena.max.1 as f32 + 0.5) * spacing;
    let mid_x = (min_x + max_x) * 0.5;
    let mid_z = (min_z + max_z) * 0.5;
    let width = max_x - min_x;
    let depth = max_z - min_z;
    let wall_height = config::LIGHTCYCLE_WALL_HEIGHT;
    let wall_thickness = config::LIGHTCYCLE_WALL_THICKNESS;

    let spawn_wall = |commands: &mut Commands, translation: Vec3, scale: Vec3| {
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.wall_material.clone()),
            Transform::from_translation(translation).with_scale(scale),
            Pickable::IGNORE,
        ));
    };

    // Two rails parallel to X.
    spawn_wall(
        commands,
        Vec3::new(mid_x, wall_height * 0.5, min_z),
        Vec3::new(width, wall_height, wall_thickness),
    );
    spawn_wall(
        commands,
        Vec3::new(mid_x, wall_height * 0.5, max_z),
        Vec3::new(width, wall_height, wall_thickness),
    );
    // Two rails parallel to Z.
    spawn_wall(
        commands,
        Vec3::new(min_x, wall_height * 0.5, mid_z),
        Vec3::new(wall_thickness, wall_height, depth),
    );
    spawn_wall(
        commands,
        Vec3::new(max_x, wall_height * 0.5, mid_z),
        Vec3::new(wall_thickness, wall_height, depth),
    );

    // Parent gate on the -Z wall.
    if let Some((portal_x, _)) = arena.parent_portal {
        commands.spawn((
            LightcycleSceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.portal_material.clone()),
            Transform::from_translation(Vec3::new(
                portal_x as f32 * spacing,
                config::LIGHTCYCLE_PORTAL_HEIGHT * 0.5,
                min_z,
            ))
            .with_scale(Vec3::new(
                config::LIGHTCYCLE_PORTAL_WIDTH,
                config::LIGHTCYCLE_PORTAL_HEIGHT,
                wall_thickness * 3.0,
            )),
            Pickable::IGNORE,
        ));
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

    // Keep the cycle stopped at the folder/portal; DirectoryLoadState shows the error.
    if let Some(run) = state.run.as_mut()
        && run.sim.phase == RunPhase::EnteringDir
    {
        run.sim.pending_request = None;
        run.entering_label = None;
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

    if go_up && let Some(parent) = navigator.0.begin_go_to_parent() {
        run.sim.pause_for_directory_change();
        run.entering_label = Some("parent directory".to_string());
        run.crash_label = None;
        requests.write(DirectoryRequested { path: parent });
    }

    state.run = Some(run);
}

fn restart_run(run: &mut ActiveRun) {
    run.sim = spawn_sim(&run.arena, &run.cells);
    run.crash_label = None;
    run.entering_label = None;
    run.trail_dirty = true;
}

fn step_lightcycle(
    time: Res<Time>,
    mut state: ResMut<LightcycleState>,
    mut navigator: ResMut<NavigatorResource>,
    mut requests: MessageWriter<DirectoryRequested>,
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

        let trail_before = run.sim.trail.len();
        let outcome = {
            let arena = run.arena.clone();
            let cells = &run.cells;
            let nodes = &run.nodes;

            run.sim.advance(
                fixed_step * config::LIGHTCYCLE_CELLS_PER_SEC,
                |next, sim| {
                    classify_next_content(next, &arena, sim, cells, |index| nodes[index].is_dir)
                },
            )
        };

        if run.sim.trail.len() != trail_before {
            run.trail_dirty = true;
        }

        match outcome {
            StepOutcome::Moved => {}
            StepOutcome::Crashed(reason) => {
                let crash_cell = run.sim.next_cell();
                let label = match reason {
                    CrashReason::File => run
                        .cells
                        .get(&crash_cell)
                        .and_then(|&index| run.nodes.get(index))
                        .map(|node| format!("file {}", node.name))
                        .unwrap_or_else(|| "file".to_string()),
                    CrashReason::Trail => "your trail".to_string(),
                    CrashReason::Wall => "arena wall".to_string(),
                };
                run.crash_label = Some(label);
                run.entering_label = None;
            }
            StepOutcome::EnteringDir(index) => {
                if let Some(node) = run.nodes.get(index) {
                    let path = node.path.clone();
                    navigator.0.begin_navigate_to(&path);
                    run.entering_label = Some(node.name.clone());
                    run.crash_label = None;
                    requests.write(DirectoryRequested { path });
                } else {
                    run.sim.phase = RunPhase::Crashed;
                    run.crash_label = Some("missing directory".to_string());
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
    let origin = cycle_world_position(&run.sim);
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

fn rebuild_trail_mesh(
    mut state: ResMut<LightcycleState>,
    mut commands: Commands,
    assets: Res<LightcycleAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    old_trail: Query<Entity, With<TrailSceneRoot>>,
) {
    let Some(run) = state.run.as_mut() else {
        return;
    };
    if !run.trail_dirty {
        return;
    }
    run.trail_dirty = false;

    for entity in &old_trail {
        commands.entity(entity).despawn();
    }

    // Build a continuous wall ribbon through every cell the cycle has occupied,
    // ending at the current head so the wall visibly trails behind the cycle.
    // Corners are rounded with the same radius used by the rendered cycle path.
    let mut path = run.sim.trail.clone();
    path.push(run.sim.cell);

    let segments = trail_ribbon_segments(&path);
    for chunk in segments.chunks(config::MESH_CHUNK_SIZE) {
        let Some(mesh) = build_trail_chunk_mesh(chunk) else {
            continue;
        };
        commands.spawn((
            TrailSceneRoot,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(assets.trail_material.clone()),
            Pickable::IGNORE,
        ));
    }
}

fn trail_ribbon_segments(path: &[(i32, i32)]) -> Vec<((f32, f32), (f32, f32))> {
    let points = rounded_polyline(path);
    points
        .windows(2)
        .filter_map(|pair| match pair {
            [a, b] => {
                let dx = b.0 - a.0;
                let dz = b.1 - a.1;
                (dx * dx + dz * dz > 0.0001).then_some((*a, *b))
            }
            _ => None,
        })
        .collect()
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

            let samples = 6;
            for step in 1..=samples {
                let u = step as f32 / samples as f32;
                points.push(arc_cell_point(corner, incoming, outgoing, u, radius));
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

#[allow(clippy::type_complexity)]
fn build_trail_chunk_mesh(segments: &[((f32, f32), (f32, f32))]) -> Option<Mesh> {
    let first = *segments.first()?;
    let mut mesh = trail_segment_mesh(first.0, first.1);
    for &(a, b) in &segments[1..] {
        mesh.merge(&trail_segment_mesh(a, b))
            .expect("trail ribbon meshes must be merge-compatible");
    }
    Some(mesh)
}

fn trail_segment_mesh(a: (f32, f32), b: (f32, f32)) -> Mesh {
    let spacing = config::GRID_SPACING;
    let height = config::LIGHTCYCLE_TRAIL_HEIGHT;
    let thickness = config::LIGHTCYCLE_TRAIL_THICKNESS;

    let a_world = Vec3::new(a.0 * spacing, 0.0, a.1 * spacing);
    let b_world = Vec3::new(b.0 * spacing, 0.0, b.1 * spacing);
    let delta = b_world - a_world;
    let length = delta.length();

    let center = Vec3::new(
        (a_world.x + b_world.x) * 0.5,
        height * 0.5,
        (a_world.z + b_world.z) * 0.5,
    );
    let rotation = Quat::from_rotation_arc(Vec3::X, delta.normalize_or_zero());

    Mesh::from(Cuboid::default()).transformed_by(
        Transform::from_translation(center)
            .with_rotation(rotation)
            .with_scale(Vec3::new(length, height, thickness)),
    )
}

fn heading_rotation(heading: Heading) -> Quat {
    let (dx, dz) = heading.delta();
    let direction = Vec3::new(dx as f32, 0.0, dz as f32).normalize_or_zero();
    Quat::from_rotation_arc(Vec3::X, direction)
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn cycle_world_position(sim: &LightcycleSim) -> Vec3 {
    let height = config::LIGHTCYCLE_CYCLE_HEIGHT;
    let base = config::ground_position(sim.cell.0, sim.cell.1);

    if sim.phase != RunPhase::Running {
        return Vec3::new(base.x, height * 0.5, base.z);
    }

    let (x, z) = cycle_cell_position(sim);
    Vec3::new(
        x * config::GRID_SPACING,
        height * 0.5,
        z * config::GRID_SPACING,
    )
}

/// Continuous cell-space position for the rendered cycle.
///
/// Straight segments use the raw simulation position. Near queued/applied turns
/// the path follows a rounded 90-degree arc around the intersection so the
/// cycle visibly curves through corners instead of snapping an L-shape.
fn cycle_cell_position(sim: &LightcycleSim) -> (f32, f32) {
    if let Some((x, z)) = rounded_cell_position(sim) {
        return (x, z);
    }

    let (dx, dz) = sim.heading.delta();
    (
        sim.cell.0 as f32 + dx as f32 * sim.cell_t,
        sim.cell.1 as f32 + dz as f32 * sim.cell_t,
    )
}

fn rounded_cell_position(sim: &LightcycleSim) -> Option<(f32, f32)> {
    let radius = config::LIGHTCYCLE_TURN_RADIUS;

    // Approaching a queued turn: the first half of the arc happens just before
    // the cycle reaches the intersection cell.
    if let Some(turn) = sim.queued_turn
        && sim.cell_t >= 1.0 - radius
    {
        let incoming = sim.heading.delta();
        let outgoing = sim.heading.turn(turn).delta();
        let u = ((sim.cell_t - (1.0 - radius)) / radius) * 0.5;
        return Some(arc_cell_point(
            sim.next_cell(),
            incoming,
            outgoing,
            u,
            radius,
        ));
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
            return Some(arc_cell_point(sim.cell, incoming, outgoing, u, radius));
        }
    }

    None
}

fn arc_cell_point(
    corner: (i32, i32),
    incoming: (i32, i32),
    outgoing: (i32, i32),
    u: f32,
    radius: f32,
) -> (f32, f32) {
    let corner_x = corner.0 as f32;
    let corner_z = corner.1 as f32;
    let center_x = corner_x - incoming.0 as f32 * radius + outgoing.0 as f32 * radius;
    let center_z = corner_z - incoming.1 as f32 * radius + outgoing.1 as f32 * radius;

    let start_x = -outgoing.0 as f32;
    let start_z = -outgoing.1 as f32;
    let start_angle = start_z.atan2(start_x);
    let end_angle = (incoming.1 as f32).atan2(incoming.0 as f32);

    let mut delta = end_angle - start_angle;
    if delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    } else if delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }

    let theta = start_angle + delta * u.clamp(0.0, 1.0);
    (
        center_x + radius * theta.cos(),
        center_z + radius * theta.sin(),
    )
}

fn update_cycle_transform(
    time: Res<Time>,
    state: Res<LightcycleState>,
    mut cycle: Query<(&mut Transform, &mut CycleVisual), With<CycleEntity>>,
) {
    let Ok((mut transform, mut visual)) = cycle.single_mut() else {
        return;
    };
    let Some(run) = state.run.as_ref() else {
        return;
    };

    transform.translation = cycle_world_position(&run.sim);

    let target_rotation = heading_rotation(run.sim.heading);
    if visual.target_rotation != target_rotation {
        visual.from_rotation = transform.rotation;
        visual.target_rotation = target_rotation;
        visual.progress = 0.0;
    }

    if visual.progress < 1.0 {
        visual.progress += time.delta_secs() / config::LIGHTCYCLE_TURN_DURATION;
        if visual.progress >= 1.0 {
            visual.progress = 1.0;
        }
        transform.rotation = visual
            .from_rotation
            .slerp(visual.target_rotation, smoothstep(visual.progress));
    } else {
        transform.rotation = target_rotation;
    }
}

fn update_chase_camera(
    state: Res<LightcycleState>,
    time: Res<Time>,
    mut camera: Single<&mut Transform, (With<Camera3d>, Without<CycleEntity>)>,
    cycle: Query<&Transform, (With<CycleEntity>, Without<Camera3d>)>,
) {
    let Ok(cycle) = cycle.single() else {
        return;
    };
    if state.run.is_none() {
        return;
    }

    let cycle_pos = cycle.translation;
    let raw_forward = cycle.rotation * Vec3::X;
    let forward = Vec3::new(raw_forward.x, 0.0, raw_forward.z).normalize_or_zero();
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
