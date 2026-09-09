use crate::config;
use crate::state::{InteractionMode, OrbitCameraResource};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::post_process::effect_stack::Vignette;
use bevy::prelude::*;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(config::BACKGROUND_COLOR))
            .add_systems(Startup, setup_camera)
            .add_systems(Update, update_camera.run_if(in_explorer_mode));
    }
}

fn in_explorer_mode(mode: Res<InteractionMode>) -> bool {
    *mode == InteractionMode::Explorer
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(
            config::DEFAULT_CAMERA_DISTANCE,
            config::DEFAULT_CAMERA_DISTANCE,
            config::DEFAULT_CAMERA_DISTANCE,
        )
        .looking_at(Vec3::ZERO, Vec3::Y),
        Vignette {
            intensity: (config::VIGNETTE_ALPHA * 2.5).min(1.0),
            radius: 0.65,
            smoothness: 3.0,
            color: Color::BLACK,
            ..default()
        },
    ));

    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.4, 0.6, 0.0)),
    ));
}

fn update_camera(
    mut orbit: ResMut<OrbitCameraResource>,
    mut camera: Single<&mut Transform, With<Camera3d>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
) {
    if mouse_buttons.pressed(MouseButton::Right) && mouse_motion.delta != Vec2::ZERO {
        orbit.rotate(mouse_motion.delta.x, mouse_motion.delta.y);
    }

    if mouse_scroll.delta.y != 0.0 {
        orbit.zoom(mouse_scroll.delta.y);
    }

    if orbit.target.distance_squared(orbit.target_destination) > 0.0001 {
        orbit.update();
    }

    camera.translation = orbit.position();
    camera.look_at(orbit.target, Vec3::Y);
}
