use crate::config;
use crate::state::OrbitCameraResource;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(config::BACKGROUND_COLOR))
            .add_systems(Startup, setup_camera)
            .add_systems(Update, update_camera);
    }
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
    if mouse_buttons.pressed(MouseButton::Right) {
        orbit.rotate(mouse_motion.delta.x, mouse_motion.delta.y);
    }

    if mouse_scroll.delta.y != 0.0 {
        orbit.zoom(mouse_scroll.delta.y);
    }

    orbit.update();
    camera.translation = orbit.position();
    camera.look_at(orbit.target, Vec3::Y);
}
