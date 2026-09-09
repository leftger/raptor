use crate::config;
use crate::filesystem::FileNode;
use crate::lightcycle::logic::{
    Arena, CrashReason, Heading, LightcycleSim, RunPhase, StepOutcome, classify_next_content,
};
use crate::lightcycle::{ActiveRun, LightcycleState};
use crate::load::{DirectoryLoadFailed, DirectoryLoaded, DirectoryRequested};
use crate::state::{
    InteractionMode, LightcycleSceneRoot, NavigatorResource, OrbitCameraResource, TrailSceneRoot,
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
                    read_lightcycle_input.run_if(in_lightcycle_mode),
                    step_lightcycle.run_if(in_lightcycle_mode),
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
}

#[derive(Component)]
struct CycleEntity;

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
    });
}

fn in_lightcycle_mode(mode: Res<InteractionMode>) -> bool {
    *mode == InteractionMode::Lightcycle
}

fn build_active_run(path: &Path, nodes: Vec<FileNode>) -> ActiveRun {
    let arena = Arena::from_nodes(
        nodes.iter().map(|node| node.grid_pos),
        path.parent().is_some(),
        config::LIGHTCYCLE_ARENA_PADDING,
        config::LIGHTCYCLE_EMPTY_ARENA_HALF,
    );

    let cells: HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.grid_pos, index))
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
    mut commands: Commands,
    old_lightcycle_entities: Query<Entity, Or<(With<LightcycleSceneRoot>, With<TrailSceneRoot>)>>,
) {
    if !keys.just_pressed(KeyCode::KeyM) {
        return;
    }

    despawn_lightcycle_entities(&mut commands, &old_lightcycle_entities);
    state.clock = 0.0;
    state.run = None;

    if *mode == InteractionMode::Lightcycle {
        *mode = InteractionMode::Explorer;
        orbit.reset_target();
        return;
    }

    let run = build_active_run(&navigator.0.current_path, navigator.0.entries.clone());
    spawn_run_entities(&mut commands, &assets, &run);
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

fn spawn_run_entities(commands: &mut Commands, assets: &LightcycleAssets, run: &ActiveRun) {
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
        Pickable::IGNORE,
    ));

    spawn_arena_walls(commands, assets, &run.arena);
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
    mut commands: Commands,
    old_lightcycle_entities: Query<Entity, Or<(With<LightcycleSceneRoot>, With<TrailSceneRoot>)>>,
) {
    if *mode != InteractionMode::Lightcycle {
        return;
    }

    for event in loaded.read() {
        despawn_lightcycle_entities(&mut commands, &old_lightcycle_entities);

        let run = build_active_run(&event.path, event.contents.nodes.clone());
        spawn_run_entities(&mut commands, &assets, &run);

        state.clock = 0.0;
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

        if run.sim.phase != RunPhase::Running {
            break;
        }
    }

    state.run = Some(run);
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

    for chunk in run.sim.trail.chunks(config::MESH_CHUNK_SIZE) {
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

fn build_trail_chunk_mesh(cells: &[(i32, i32)]) -> Option<Mesh> {
    let mut cells = cells.iter();
    let first = trail_cube_mesh(cells.next()?);
    let mut mesh = first;
    for cell in cells {
        mesh.merge(&trail_cube_mesh(cell))
            .expect("trail cuboid meshes must be merge-compatible");
    }
    Some(mesh)
}

fn trail_cube_mesh(cell: &(i32, i32)) -> Mesh {
    let height = config::LIGHTCYCLE_TRAIL_HEIGHT;
    Mesh::from(Cuboid::default()).transformed_by(
        Transform::from_translation(config::world_position(cell.0, cell.1, height)).with_scale(
            Vec3::new(
                config::LIGHTCYCLE_TRAIL_SIZE,
                height,
                config::LIGHTCYCLE_TRAIL_SIZE,
            ),
        ),
    )
}

fn cycle_world_position(sim: &LightcycleSim) -> Vec3 {
    let base = config::ground_position(sim.cell.0, sim.cell.1);
    let (dx, dz) = sim.heading.delta();

    let t = if sim.phase == RunPhase::Running {
        sim.cell_t
    } else {
        0.0
    };

    Vec3::new(
        base.x + dx as f32 * t * config::GRID_SPACING,
        config::LIGHTCYCLE_CYCLE_HEIGHT * 0.5,
        base.z + dz as f32 * t * config::GRID_SPACING,
    )
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
    transform.translation = cycle_world_position(&run.sim);
}

fn update_chase_camera(
    state: Res<LightcycleState>,
    mut camera: Single<&mut Transform, With<Camera3d>>,
    cycle: Query<&Transform, With<CycleEntity>>,
) {
    let Ok(cycle) = cycle.single() else {
        return;
    };
    let Some(run) = state.run.as_ref() else {
        return;
    };

    let cycle_pos = cycle.translation;
    let (dx, dz) = run.sim.heading.delta();
    let forward = Vec3::new(dx as f32, 0.0, dz as f32);
    let look_target = cycle_pos + forward * config::LIGHTCYCLE_CAMERA_LOOKAHEAD;
    let camera_position = cycle_pos - forward * config::LIGHTCYCLE_CAMERA_DISTANCE
        + Vec3::Y * config::LIGHTCYCLE_CAMERA_HEIGHT;

    camera.translation = camera_position;
    camera.look_at(look_target, Vec3::Y);
}
