use crate::config;
use crate::state::ScanEffectResource;
use bevy::asset::RenderAssetUsages;
use bevy::post_process::effect_stack::Vignette;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_scan_effect, setup_scanlines))
            .add_systems(Update, (update_scan_effect, insert_vignette));
    }
}

#[derive(Component)]
struct ScanPlane;

#[derive(Component)]
struct ScanlineRoot;

fn setup_scan_effect(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        ScanPlane,
        Mesh3d(meshes.add(Cuboid::new(120.0, 0.15, 120.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: config::SCAN_COLOR,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Visibility::Hidden,
    ));
}

fn setup_scanlines(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let alpha = (config::SCANLINE_ALPHA * 255.0) as u8;
    let texture = images.add(Image::new(
        Extent3d {
            width: 1,
            height: config::SCANLINE_STEP as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        (0..config::SCANLINE_STEP)
            .flat_map(|row| {
                let row_alpha = if row < config::SCANLINE_HEIGHT as usize {
                    alpha
                } else {
                    0
                };
                [0, 0, 0, row_alpha]
            })
            .collect(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ));

    commands.spawn((
        ScanlineRoot,
        ImageNode {
            image: texture,
            image_mode: NodeImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: 1.0,
            },
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: percent(100.0),
            height: percent(100.0),
            ..default()
        },
        ZIndex(10),
        Pickable::IGNORE,
    ));
}

fn insert_vignette(
    mut commands: Commands,
    camera: Query<Entity, (With<Camera3d>, Without<Vignette>)>,
) {
    for entity in &camera {
        commands.entity(entity).insert(Vignette {
            intensity: (config::VIGNETTE_ALPHA * 2.5).min(1.0),
            radius: 0.65,
            smoothness: 3.0,
            color: Color::BLACK,
            ..default()
        });
    }
}

fn update_scan_effect(
    time: Res<Time>,
    mut scan: ResMut<ScanEffectResource>,
    mut plane: Query<(&mut Transform, &mut Visibility), With<ScanPlane>>,
) {
    scan.update(time.delta_secs());

    let Ok((mut transform, mut visibility)) = plane.single_mut() else {
        return;
    };

    if scan.active {
        transform.translation.y = scan.y_position;
        *visibility = Visibility::Visible;
    } else {
        *visibility = Visibility::Hidden;
    }
}
