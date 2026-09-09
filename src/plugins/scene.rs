use crate::config;
use crate::filesystem::FileNode;
use crate::state::{
    DirectoryLoaded, DirectorySceneRoot, FileBlock, NavigatorResource, SelectionState,
};
use bevy::prelude::*;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_assets, spawn_highlight_shells).chain())
            .add_systems(Update, (spawn_scene, update_highlighting));
    }
}

#[derive(Resource)]
pub struct RaptorAssets {
    pub unit_cube: Handle<Mesh>,
    pub grid_line: Handle<Mesh>,
    pub grid_material: Handle<StandardMaterial>,
    pub dir_body: Handle<StandardMaterial>,
    pub file_body: Handle<StandardMaterial>,
    pub hover_dir_body: Handle<StandardMaterial>,
    pub hover_file_body: Handle<StandardMaterial>,
    pub selected_body: Handle<StandardMaterial>,
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
        grid_line: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        grid_material: materials.add(unlit_material(config::GRID_COLOR, Some(0.35))),
        dir_body: materials.add(unlit_material(config::DIR_COLOR, None)),
        file_body: materials.add(unlit_material(config::FILE_COLOR, None)),
        hover_dir_body: materials.add(unlit_material(config::HOVER_DIR_COLOR, None)),
        hover_file_body: materials.add(unlit_material(config::HOVER_FILE_COLOR, None)),
        selected_body: materials.add(unlit_material(config::SELECTED_COLOR, None)),
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
) {
    for event in loaded.read() {
        for entity in &old_scene {
            commands.entity(entity).despawn();
        }

        spawn_grid(&mut commands, &assets);
        spawn_blocks(&mut commands, &assets, &event.contents.nodes);
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
            Mesh3d(assets.grid_line.clone()),
            MeshMaterial3d(assets.grid_material.clone()),
            Transform::from_translation(Vec3::new(0.0, 0.0, pos)).with_scale(Vec3::new(
                extent * 2.0,
                line_width,
                line_width,
            )),
        ));

        // Along Z at fixed X.
        commands.spawn((
            DirectorySceneRoot,
            Mesh3d(assets.grid_line.clone()),
            MeshMaterial3d(assets.grid_material.clone()),
            Transform::from_translation(Vec3::new(pos, 0.0, 0.0)).with_scale(Vec3::new(
                line_width,
                line_width,
                extent * 2.0,
            )),
        ));
    }
}

fn spawn_blocks(commands: &mut Commands, assets: &RaptorAssets, nodes: &[FileNode]) {
    let batch: Vec<_> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let height = node.calculate_height();
            let material = if node.is_dir {
                assets.dir_body.clone()
            } else {
                assets.file_body.clone()
            };

            (
                DirectorySceneRoot,
                FileBlock { index },
                Mesh3d(assets.unit_cube.clone()),
                MeshMaterial3d(material),
                Transform::from_translation(config::world_position(
                    node.grid_pos.0,
                    node.grid_pos.1,
                    height,
                ))
                .with_scale(Vec3::new(
                    config::BLOCK_WIDTH,
                    height,
                    config::BLOCK_DEPTH,
                )),
            )
        })
        .collect();

    commands.spawn_batch(batch);
}

#[allow(clippy::type_complexity)]
fn update_highlighting(
    navigator: Res<NavigatorResource>,
    selection: Res<SelectionState>,
    assets: Res<RaptorAssets>,
    mut blocks: Query<
        (&FileBlock, &mut MeshMaterial3d<StandardMaterial>),
        (Without<HoverShell>, Without<SelectedShell>),
    >,
    mut hover_shell: Query<
        (
            &mut Transform,
            &mut MeshMaterial3d<StandardMaterial>,
            &mut Visibility,
        ),
        (With<HoverShell>, Without<SelectedShell>, Without<FileBlock>),
    >,
    mut selected_shell: Query<
        (
            &mut Transform,
            &mut MeshMaterial3d<StandardMaterial>,
            &mut Visibility,
        ),
        (With<SelectedShell>, Without<HoverShell>, Without<FileBlock>),
    >,
) {
    for (block, mut material) in &mut blocks {
        let Some(node) = navigator.0.entries.get(block.index) else {
            continue;
        };

        let target = if selection.selected == Some(block.index) {
            assets.selected_body.clone()
        } else if selection.hovered == Some(block.index) {
            if node.is_dir {
                assets.hover_dir_body.clone()
            } else {
                assets.hover_file_body.clone()
            }
        } else if node.is_dir {
            assets.dir_body.clone()
        } else {
            assets.file_body.clone()
        };

        if material.0 != target {
            material.0 = target;
        }
    }

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
