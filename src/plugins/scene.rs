use crate::config;
use crate::filesystem::FileNode;
use crate::load::DirectoryLoaded;
use crate::plugins::transition::{transition_active, transition_inactive};
use crate::state::{DirectorySceneRoot, InteractionMode, NavigatorResource, SelectionState};
use bevy::prelude::*;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_assets, spawn_highlight_shells).chain())
            .add_systems(
                Update,
                (
                    spawn_scene,
                    // The shells wrap a block at its full height, so they have
                    // to go away while a mode transition folds the scene flat.
                    update_highlighting.run_if(in_explorer_mode.and_then(transition_inactive)),
                    hide_highlighting.run_if(in_lightcycle_mode.or_else(transition_active)),
                ),
            );
    }
}

fn in_explorer_mode(mode: Res<InteractionMode>) -> bool {
    *mode == InteractionMode::Explorer
}

fn in_lightcycle_mode(mode: Res<InteractionMode>) -> bool {
    *mode == InteractionMode::Lightcycle
}

fn hide_highlighting(
    mut hover: Query<&mut Visibility, (With<HoverShell>, Without<SelectedShell>)>,
    mut selected: Query<&mut Visibility, (With<SelectedShell>, Without<HoverShell>)>,
) {
    for mut visibility in &mut hover {
        *visibility = Visibility::Hidden;
    }
    for mut visibility in &mut selected {
        *visibility = Visibility::Hidden;
    }
}

#[derive(Resource)]
pub struct RaptorAssets {
    pub unit_cube: Handle<Mesh>,
    pub grid_material: Handle<StandardMaterial>,
    pub dir_body: Handle<StandardMaterial>,
    pub file_body: Handle<StandardMaterial>,
    pub source_body: Handle<StandardMaterial>,
    pub hover_glow: Handle<StandardMaterial>,
    pub hover_file_glow: Handle<StandardMaterial>,
    pub selected_glow: Handle<StandardMaterial>,
}

#[derive(Component)]
struct HoverShell;

#[derive(Component)]
struct SelectedShell;

fn unlit_material(color: Color, alpha: Option<f32>) -> StandardMaterial {
    StandardMaterial {
        base_color: if let Some(alpha) = alpha {
            color.with_alpha(alpha)
        } else {
            color
        },
        unlit: true,
        alpha_mode: if alpha.is_some() {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        ..default()
    }
}

fn setup_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let unit_cube = meshes.add(Cuboid::default());

    commands.insert_resource(RaptorAssets {
        unit_cube,
        grid_material: materials.add(unlit_material(config::GRID_COLOR, Some(0.35))),
        dir_body: materials.add(unlit_material(config::DIR_COLOR, None)),
        file_body: materials.add(unlit_material(config::FILE_COLOR, None)),
        source_body: materials.add(unlit_material(config::SOURCE_TOWER_COLOR, None)),
        hover_glow: materials.add(unlit_material(config::HOVER_DIR_COLOR, Some(0.30))),
        hover_file_glow: materials.add(unlit_material(config::HOVER_FILE_COLOR, Some(0.30))),
        selected_glow: materials.add(unlit_material(config::SELECTED_COLOR, Some(0.30))),
    });
}

fn spawn_highlight_shells(mut commands: Commands, assets: Res<RaptorAssets>) {
    commands.spawn((
        HoverShell,
        Mesh3d(assets.unit_cube.clone()),
        MeshMaterial3d(assets.hover_glow.clone()),
        Transform::IDENTITY,
        Visibility::Hidden,
    ));
    commands.spawn((
        SelectedShell,
        Mesh3d(assets.unit_cube.clone()),
        MeshMaterial3d(assets.selected_glow.clone()),
        Transform::IDENTITY,
        Visibility::Hidden,
    ));
}

fn spawn_scene(
    mut commands: Commands,
    mut loaded: MessageReader<DirectoryLoaded>,
    old_scene: Query<Entity, With<DirectorySceneRoot>>,
    assets: Res<RaptorAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for event in loaded.read() {
        for entity in &old_scene {
            commands.entity(entity).despawn();
        }

        spawn_grid(&mut commands, &assets);
        spawn_blocks(&mut commands, &assets, &mut meshes, &event.contents.nodes);
    }
}

fn spawn_grid(commands: &mut Commands, assets: &RaptorAssets) {
    let extent = config::GRID_SIZE as f32 * config::GRID_SPACING;
    let line_width = 0.03;

    for i in -config::GRID_SIZE..=config::GRID_SIZE {
        let pos = i as f32 * config::GRID_SPACING;

        // Along X at fixed Z.
        commands.spawn((
            DirectorySceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.grid_material.clone()),
            Pickable::IGNORE,
            Transform::from_translation(Vec3::new(0.0, 0.0, pos)).with_scale(Vec3::new(
                extent * 2.0,
                line_width,
                line_width,
            )),
        ));

        // Along Z at fixed X.
        commands.spawn((
            DirectorySceneRoot,
            Mesh3d(assets.unit_cube.clone()),
            MeshMaterial3d(assets.grid_material.clone()),
            Pickable::IGNORE,
            Transform::from_translation(Vec3::new(pos, 0.0, 0.0)).with_scale(Vec3::new(
                line_width,
                line_width,
                extent * 2.0,
            )),
        ));
    }
}

/// One material plus the predicate selecting the nodes that wear it.
type BlockGroup = (Handle<StandardMaterial>, fn(&FileNode) -> bool);

fn spawn_blocks(
    commands: &mut Commands,
    assets: &RaptorAssets,
    meshes: &mut Assets<Mesh>,
    nodes: &[FileNode],
) {
    // Directories, source files, and everything else each get their own body,
    // so a rideable `.rs` tower reads differently from a plain file.
    let groups: [BlockGroup; 3] = [
        (assets.dir_body.clone(), |node| node.is_dir),
        (assets.source_body.clone(), |node| {
            !node.is_dir && node.is_source()
        }),
        (assets.file_body.clone(), |node| {
            !node.is_dir && !node.is_source()
        }),
    ];
    for (material, matches) in groups {
        let matching: Vec<_> = nodes.iter().filter(|node| matches(node)).collect();
        spawn_block_chunks(commands, meshes, &matching, material);
    }
}

fn spawn_block_chunks(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    matching: &[&FileNode],
    material: Handle<StandardMaterial>,
) {
    for chunk in matching.chunks(config::MESH_CHUNK_SIZE) {
        let Some(mesh) = build_chunk_mesh(chunk) else {
            continue;
        };
        commands.spawn((
            DirectorySceneRoot,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Pickable::IGNORE,
        ));
    }
}

fn build_chunk_mesh(nodes: &[&FileNode]) -> Option<Mesh> {
    let mut nodes = nodes.iter();
    let first = transformed_cube(nodes.next()?);
    let mut mesh = first;
    for node in nodes {
        // Every source is the same cuboid topology and attributes, so merging cannot fail.
        mesh.merge(&transformed_cube(node))
            .expect("cuboid meshes must be merge-compatible");
    }
    Some(mesh)
}

fn transformed_cube(node: &FileNode) -> Mesh {
    let height = node.calculate_height();
    Mesh::from(Cuboid::default()).transformed_by(
        Transform::from_translation(config::world_position(
            node.grid_pos.0,
            node.grid_pos.1,
            height,
        ))
        .with_scale(Vec3::new(config::BLOCK_WIDTH, height, config::BLOCK_DEPTH)),
    )
}

#[allow(clippy::type_complexity)]
fn update_highlighting(
    navigator: Res<NavigatorResource>,
    selection: Res<SelectionState>,
    assets: Res<RaptorAssets>,
    mut hover_shell: Query<
        (
            &mut Transform,
            &mut MeshMaterial3d<StandardMaterial>,
            &mut Visibility,
        ),
        (With<HoverShell>, Without<SelectedShell>),
    >,
    mut selected_shell: Query<
        (
            &mut Transform,
            &mut MeshMaterial3d<StandardMaterial>,
            &mut Visibility,
        ),
        (With<SelectedShell>, Without<HoverShell>),
    >,
) {
    let hover_index = selection
        .hovered
        .filter(|hovered| selection.selected != Some(*hovered));
    if let Ok((mut transform, mut material, mut visibility)) = hover_shell.single_mut() {
        set_shell_state(
            &mut transform,
            &mut material,
            &mut visibility,
            hover_index,
            &navigator,
            &assets.hover_glow,
            &assets.hover_file_glow,
        );
    }

    if let Ok((mut transform, mut material, mut visibility)) = selected_shell.single_mut() {
        set_shell_state(
            &mut transform,
            &mut material,
            &mut visibility,
            selection.selected,
            &navigator,
            &assets.selected_glow,
            &assets.selected_glow,
        );
    }
}

fn set_shell_state(
    transform: &mut Transform,
    material: &mut MeshMaterial3d<StandardMaterial>,
    visibility: &mut Visibility,
    index: Option<usize>,
    navigator: &NavigatorResource,
    dir_material: &Handle<StandardMaterial>,
    file_material: &Handle<StandardMaterial>,
) {
    let Some(index) = index else {
        *visibility = Visibility::Hidden;
        return;
    };
    let Some(node) = navigator.0.entries.get(index) else {
        *visibility = Visibility::Hidden;
        return;
    };

    let height = node.calculate_height();
    *transform = Transform::from_translation(config::world_position(
        node.grid_pos.0,
        node.grid_pos.1,
        height,
    ))
    .with_scale(Vec3::new(
        config::BLOCK_WIDTH + 0.25,
        height + 0.25,
        config::BLOCK_DEPTH + 0.25,
    ));

    let target = if node.is_dir {
        dir_material
    } else {
        file_material
    };
    if material.0 != *target {
        material.0 = target.clone();
    }
    *visibility = Visibility::Visible;
}

#[cfg(test)]
mod tests {
    use super::build_chunk_mesh;
    use crate::filesystem::FileNode;
    use bevy::prelude::{Cuboid, Mesh};
    use std::path::PathBuf;

    #[test]
    fn chunk_mesh_contains_every_source_cube() {
        let mut first = FileNode::new("a".into(), PathBuf::from("a"), false, 1, 0);
        first.grid_pos = (0, 0);
        let mut second = FileNode::new("b".into(), PathBuf::from("b"), false, 1, 0);
        second.grid_pos = (1, 0);

        let mesh = build_chunk_mesh(&[&first, &second]).unwrap();
        assert_eq!(
            mesh.count_vertices(),
            Mesh::from(Cuboid::default()).count_vertices() * 2
        );
    }
}
