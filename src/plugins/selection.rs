use crate::command::Command;
use crate::config;
use crate::platform;
use crate::state::{
    DirectoryRequested, NavigatorResource, OrbitCameraResource, ScanEffectResource, SelectionState,
    UiSettings,
};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

pub struct SelectionPlugin;

impl Plugin for SelectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Command>().add_systems(
            Update,
            (
                read_keyboard_commands,
                handle_commands.after(read_keyboard_commands),
                update_mouse_picking,
                handle_mouse_click.after(update_mouse_picking),
            ),
        );
    }
}

fn read_keyboard_commands(keys: Res<ButtonInput<KeyCode>>, mut commands: MessageWriter<Command>) {
    let mut send = |command: Command| commands.write(command);

    if keys.just_pressed(KeyCode::KeyH) || keys.just_pressed(KeyCode::ArrowLeft) {
        send(Command::MoveLeft);
    }
    if keys.just_pressed(KeyCode::KeyJ) || keys.just_pressed(KeyCode::ArrowDown) {
        send(Command::MoveDown);
    }
    if keys.just_pressed(KeyCode::KeyK) || keys.just_pressed(KeyCode::ArrowUp) {
        send(Command::MoveUp);
    }
    if keys.just_pressed(KeyCode::KeyL) || keys.just_pressed(KeyCode::ArrowRight) {
        send(Command::MoveRight);
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::KeyO) {
        send(Command::OpenSelected);
    }
    if keys.just_pressed(KeyCode::Backspace) || keys.just_pressed(KeyCode::Escape) {
        send(Command::GoBack);
    }
    if keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::KeyU) {
        send(Command::GoToParent);
    }
    if keys.just_pressed(KeyCode::Slash) {
        send(Command::GoToRoot);
    }
    if keys.just_pressed(KeyCode::Home) {
        send(Command::GoHome);
    }
    if keys.just_pressed(KeyCode::KeyR) {
        send(Command::ReloadDirectory);
    }
    if keys.just_pressed(KeyCode::Digit0) {
        send(Command::GoToFirst);
    }
    if keys.just_pressed(KeyCode::KeyG)
        && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight))
    {
        send(Command::GoToLast);
    }
    if keys.just_pressed(KeyCode::Period) {
        send(Command::ToggleHidden);
    }
    if keys.just_pressed(KeyCode::Tab) {
        send(Command::ToggleLabels);
    }
    if keys.just_pressed(KeyCode::KeyF) {
        send(Command::RevealInFileManager);
    }
}

fn handle_commands(
    mut command_events: MessageReader<Command>,
    mut navigator: ResMut<NavigatorResource>,
    mut selection: ResMut<SelectionState>,
    mut orbit: ResMut<OrbitCameraResource>,
    mut ui_settings: ResMut<UiSettings>,
    mut scan: ResMut<ScanEffectResource>,
    mut requests: MessageWriter<DirectoryRequested>,
) {
    for command in command_events.read() {
        execute_command(
            *command,
            &mut navigator.0,
            &mut selection,
            &mut orbit,
            &mut ui_settings,
            &mut scan,
            &mut requests,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_command(
    command: Command,
    navigator: &mut crate::filesystem::Navigator,
    selection: &mut SelectionState,
    orbit: &mut OrbitCameraResource,
    ui_settings: &mut UiSettings,
    scan: &mut ScanEffectResource,
    requests: &mut MessageWriter<DirectoryRequested>,
) {
    match command {
        Command::MoveLeft => move_selection(-1, 0, navigator, selection, orbit),
        Command::MoveRight => move_selection(1, 0, navigator, selection, orbit),
        Command::MoveUp => move_selection(0, -1, navigator, selection, orbit),
        Command::MoveDown => move_selection(0, 1, navigator, selection, orbit),

        Command::GoToFirst => {
            if !navigator.entries.is_empty() {
                selection.selected = Some(0);
                focus_camera_on_selection(navigator, selection, orbit);
            }
        }

        Command::GoToLast => {
            if !navigator.entries.is_empty() {
                selection.selected = Some(navigator.entries.len() - 1);
                focus_camera_on_selection(navigator, selection, orbit);
            }
        }

        Command::OpenSelected => {
            if let Some(index) = selection.selected {
                let Some(node) = navigator.entries.get(index) else {
                    return;
                };

                if node.is_dir {
                    let path = node.path.clone();
                    navigator.begin_navigate_to(&path);
                    requests.write(DirectoryRequested { path });
                } else {
                    platform::open_path(&node.path);
                }
            }
        }

        Command::GoBack => {
            if let Some(path) = navigator.begin_go_back() {
                requests.write(DirectoryRequested { path });
            }
        }

        Command::GoToParent => {
            if let Some(path) = navigator.begin_go_to_parent() {
                requests.write(DirectoryRequested { path });
            }
        }

        Command::GoToRoot => {
            let path = navigator.begin_go_to_root();
            requests.write(DirectoryRequested { path });
        }

        Command::GoHome => {
            let path = navigator.begin_go_home();
            requests.write(DirectoryRequested { path });
        }

        Command::ReloadDirectory => {
            let path = navigator.current_path.clone();
            requests.write(DirectoryRequested { path });
        }

        Command::ToggleHidden => {
            navigator.show_hidden = !navigator.show_hidden;
            let path = navigator.current_path.clone();
            requests.write(DirectoryRequested { path });
        }

        Command::ToggleLabels => {
            ui_settings.show_labels = !ui_settings.show_labels;
        }

        Command::RevealInFileManager => {
            let path = match selection
                .selected
                .and_then(|index| navigator.entries.get(index))
            {
                Some(node) => node.path.clone(),
                None => navigator.current_path.clone(),
            };
            platform::reveal_path(&path);
        }
    }

    // A navigation command starts a fresh scan effect; harmless to reset for open file etc.
    if matches!(
        command,
        Command::OpenSelected
            | Command::GoBack
            | Command::GoToParent
            | Command::GoToRoot
            | Command::GoHome
            | Command::ReloadDirectory
            | Command::ToggleHidden
    ) {
        scan.reset();
    }
}

fn move_selection(
    dx: i32,
    dz: i32,
    navigator: &crate::filesystem::Navigator,
    selection: &mut SelectionState,
    orbit: &mut OrbitCameraResource,
) {
    if navigator.entries.is_empty() {
        return;
    }

    if selection.selected.is_none() {
        selection.selected = Some(0);
        focus_camera_on_selection(navigator, selection, orbit);
        return;
    }

    let current_index = selection.selected.unwrap();
    let Some(current_node) = navigator.entries.get(current_index) else {
        return;
    };
    let (cx, cz) = current_node.grid_pos;

    // Camera-relative direction.
    let sin_yaw = orbit.yaw.sin();
    let cos_yaw = orbit.yaw.cos();
    let world_dx = dx as f32 * sin_yaw + dz as f32 * cos_yaw;
    let world_dz = -dx as f32 * cos_yaw + dz as f32 * sin_yaw;
    let dir_len = (world_dx * world_dx + world_dz * world_dz).sqrt();
    if dir_len < 0.001 {
        return;
    }
    let norm_dx = world_dx / dir_len;
    let norm_dz = world_dz / dir_len;

    let mut best_index: Option<usize> = None;
    let mut best_score = f32::MAX;

    for (i, node) in navigator.entries.iter().enumerate() {
        if i == current_index {
            continue;
        }
        let (nx, nz) = node.grid_pos;
        let delta_x = (nx - cx) as f32;
        let delta_z = (nz - cz) as f32;
        let dot = delta_x * norm_dx + delta_z * norm_dz;
        if dot <= 0.0 {
            continue;
        }

        let dist = (delta_x * delta_x + delta_z * delta_z).sqrt();
        let perp_dist = ((delta_x * norm_dz - delta_z * norm_dx).abs()).max(0.01);
        let score = dist + perp_dist * 2.0 - dot * 0.5;
        if score < best_score {
            best_score = score;
            best_index = Some(i);
        }
    }

    if let Some(new_index) = best_index {
        selection.selected = Some(new_index);
        focus_camera_on_selection(navigator, selection, orbit);
    }
}

fn focus_camera_on_selection(
    navigator: &crate::filesystem::Navigator,
    selection: &SelectionState,
    orbit: &mut OrbitCameraResource,
) {
    if let Some(index) = selection.selected
        && let Some(node) = navigator.entries.get(index)
    {
        orbit.set_target(Vec3::new(
            node.grid_pos.0 as f32 * config::GRID_SPACING,
            0.0,
            node.grid_pos.1 as f32 * config::GRID_SPACING,
        ));
    }
}

fn update_mouse_picking(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    navigator: Res<NavigatorResource>,
    mut selection: ResMut<SelectionState>,
) {
    selection.hovered = None;

    let Some(cursor) = window.cursor_position() else {
        return;
    };

    let (camera, camera_transform) = *camera;
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor) else {
        return;
    };

    let mut closest: Option<(usize, f32)> = None;
    for (index, node) in navigator.0.entries.iter().enumerate() {
        let height = node.calculate_height();
        let half_size = Vec3::new(
            config::BLOCK_WIDTH / 2.0,
            height / 2.0,
            config::BLOCK_DEPTH / 2.0,
        );
        let center = Vec3::new(
            node.grid_pos.0 as f32 * config::GRID_SPACING,
            height / 2.0,
            node.grid_pos.1 as f32 * config::GRID_SPACING,
        );

        if let Some(distance) = ray_box_intersection(&ray, center, half_size)
            && (closest.is_none() || distance < closest.unwrap().1)
        {
            closest = Some((index, distance));
        }
    }

    if let Some((index, _)) = closest {
        selection.hovered = Some(index);
    }
}

fn handle_mouse_click(
    window: Single<&Window, With<PrimaryWindow>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    navigator: Res<NavigatorResource>,
    mut selection: ResMut<SelectionState>,
    mut commands: MessageWriter<Command>,
    mut orbit: ResMut<OrbitCameraResource>,
) {
    if !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }

    // Keep UI bars from selecting blocks underneath them.
    if let Some(cursor) = window.cursor_position() {
        let in_ui_band = cursor.y < config::HEADER_HEIGHT + config::BREADCRUMB_HEIGHT
            || cursor.y > window.height() - config::FOOTER_HEIGHT;
        if in_ui_band {
            return;
        }
    }

    let Some(hovered) = selection.hovered else {
        return;
    };

    if selection.selected == Some(hovered) {
        // Clicking the already-selected block opens/enters it (macroquad parity).
        commands.write(Command::OpenSelected);
    } else {
        selection.selected = Some(hovered);
        focus_camera_on_selection(&navigator.0, &selection, &mut orbit);
    }
}

fn ray_box_intersection(ray: &bevy::math::Ray3d, center: Vec3, half_size: Vec3) -> Option<f32> {
    let min = center - half_size;
    let max = center + half_size;
    let origin = ray.origin;
    let direction = ray.direction.as_vec3();

    let mut tmin = (min.x - origin.x) / direction.x;
    let mut tmax = (max.x - origin.x) / direction.x;
    if tmin > tmax {
        std::mem::swap(&mut tmin, &mut tmax);
    }

    let mut tymin = (min.y - origin.y) / direction.y;
    let mut tymax = (max.y - origin.y) / direction.y;
    if tymin > tymax {
        std::mem::swap(&mut tymin, &mut tymax);
    }

    if tmin > tymax || tymin > tmax {
        return None;
    }

    tmin = tmin.max(tymin);
    tmax = tmax.min(tymax);

    let mut tzmin = (min.z - origin.z) / direction.z;
    let mut tzmax = (max.z - origin.z) / direction.z;
    if tzmin > tzmax {
        std::mem::swap(&mut tzmin, &mut tzmax);
    }

    if tmin > tzmax || tzmin > tmax {
        return None;
    }

    tmin = tmin.max(tzmin);
    (tmin > 0.0).then_some(tmin)
}
