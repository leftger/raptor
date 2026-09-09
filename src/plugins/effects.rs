use crate::config;
use crate::state::ScanEffectResource;
use bevy::post_process::effect_stack::Vignette;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_scan_effect, setup_scanlines))
            .add_systems(Update, (update_scan_effect, insert_vignette));
    }
}

#[derive(Component)]
struct ScanPlane;

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

fn setup_scanlines(mut commands: Commands, window: Single<&Window, With<PrimaryWindow>>) {
    let height = window.height() as i32;
    commands
        .spawn((
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
        ))
        .with_children(|parent| {
            for y in (0..height).step_by(config::SCANLINE_STEP) {
                parent.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        top: px(y as f32),
                        left: px(0.0),
                        width: percent(100.0),
                        height: px(config::SCANLINE_HEIGHT),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, config::SCANLINE_ALPHA)),
                    Pickable::IGNORE,
                ));
            }
        });
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
