use crate::command::Command;
use crate::config;
use crate::platform;
use crate::state::{
    DirectoryLoaded, DirectoryRequested, NavigatorResource, OrbitCameraResource,
    ScanEffectResource, SelectionState, UiNotice, UiSettings,
};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::HashMap;

pub struct SelectionPlugin;

impl Plugin for SelectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Command>()
            .init_resource::<SpatialIndex>()
            .add_systems(
                Update,
                (
                    read_keyboard_commands,
                    handle_commands.after(read_keyboard_commands),
                    rebuild_spatial_index,
                    update_mouse_picking.after(rebuild_spatial_index),
                    handle_mouse_click.after(update_mouse_picking),
                ),
            );
    }
}

#[derive(Resource, Default)]
struct SpatialIndex(HashMap<(i32, i32), usize>);

#[derive(Default)]
struct PickingCache {
    cursor: Option<Vec2>,
    camera: Option<Mat4>,
}

fn rebuild_spatial_index(
    mut loaded: MessageReader<DirectoryLoaded>,
    mut index: ResMut<SpatialIndex>,
) {
    for event in loaded.read() {
        index.0.clear();
        index.0.extend(
            event
                .contents
                .nodes
                .iter()
                .enumerate()
                .map(|(entry_index, node)| (node.grid_pos, entry_index)),
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

#[allow(clippy::too_many_arguments)]
fn handle_commands(
    mut command_events: MessageReader<Command>,
    mut navigator: ResMut<NavigatorResource>,
    mut selection: ResMut<SelectionState>,
    mut orbit: ResMut<OrbitCameraResource>,
    mut ui_settings: ResMut<UiSettings>,
    mut ui_notice: ResMut<UiNotice>,
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
            &mut ui_notice,
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
    ui_notice: &mut UiNotice,
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
                } else if let Err(error) = platform::open_path(&node.path) {
                    ui_notice.message =
                        Some(format!("Could not open {}: {error}", node.path.display()));
                } else {
                    ui_notice.message = None;
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
            if let Err(error) = platform::reveal_path(&path) {
                ui_notice.message = Some(format!("Could not reveal {}: {error}", path.display()));
            } else {
                ui_notice.message = None;
            }
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
    spatial_index: Res<SpatialIndex>,
    mut selection: ResMut<SelectionState>,
    mut cache: Local<PickingCache>,
) {
    let Some(cursor) = window.cursor_position() else {
        if selection.hovered.is_some() {
            selection.hovered = None;
        }
        return;
    };

    let (camera, camera_transform) = *camera;
    let camera_matrix = camera_transform.to_matrix();
    if cache.cursor == Some(cursor) && cache.camera == Some(camera_matrix) {
        return;
    }
    cache.cursor = Some(cursor);
    cache.camera = Some(camera_matrix);

    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor) else {
        if selection.hovered.is_some() {
            selection.hovered = None;
        }
        return;
    };

    let mut closest: Option<(usize, f32)> = None;
    for grid_pos in ray_grid_cells(&ray) {
        let Some(&index) = spatial_index.0.get(&grid_pos) else {
            continue;
        };
        let Some(node) = navigator.0.entries.get(index) else {
            continue;
        };
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

        if let Some(distance) = ray_box_intersection(&ray, center, half_size) {
            match closest {
                Some((_, closest_distance)) if closest_distance <= distance => {}
                _ => closest = Some((index, distance)),
            }
        }
    }

    let hovered = closest.map(|(index, _)| index);
    if selection.hovered != hovered {
        selection.hovered = hovered;
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

    let mut near: f32 = 0.0;
    let mut far = f32::INFINITY;
    for (origin, direction, min, max) in [
        (origin.x, direction.x, min.x, max.x),
        (origin.y, direction.y, min.y, max.y),
        (origin.z, direction.z, min.z, max.z),
    ] {
        if direction.abs() < f32::EPSILON {
            if origin < min || origin > max {
                return None;
            }
            continue;
        }

        let mut axis_near = (min - origin) / direction;
        let mut axis_far = (max - origin) / direction;
        if axis_near > axis_far {
            std::mem::swap(&mut axis_near, &mut axis_far);
        }
        near = near.max(axis_near);
        far = far.min(axis_far);
        if near > far {
            return None;
        }
    }
    (far >= 0.0).then_some(near.max(0.0))
}

fn ray_grid_cells(ray: &bevy::math::Ray3d) -> Vec<(i32, i32)> {
    let direction = ray.direction.as_vec3();
    if direction.y >= -f32::EPSILON {
        return Vec::new();
    }

    let max_height = config::MAX_BLOCK_HEIGHT.max(
        (config::DIR_CHILD_COUNT_CAP as f32).sqrt() * config::DIR_HEIGHT_MULTIPLIER
            + config::DIR_HEIGHT_BASE,
    );
    let start_t = ((max_height - ray.origin.y) / direction.y).max(0.0);
    let end_t = (0.0 - ray.origin.y) / direction.y;
    if end_t < start_t {
        return Vec::new();
    }

    let start = ray.origin + direction * start_t;
    let end = ray.origin + direction * end_t;
    grid_cells_along_segment(start.xz(), end.xz())
}

fn grid_cells_along_segment(start: Vec2, end: Vec2) -> Vec<(i32, i32)> {
    let spacing = config::GRID_SPACING;
    let to_cell = |value: f32| (value / spacing + 0.5).floor() as i32;
    let mut cell = IVec2::new(to_cell(start.x), to_cell(start.y));
    let end_cell = IVec2::new(to_cell(end.x), to_cell(end.y));
    let delta = end - start;
    let step = IVec2::new(delta.x.signum() as i32, delta.y.signum() as i32);

    let axis_values = |coordinate: f32, cell: i32, delta: f32, step: i32| {
        if step == 0 {
            (f32::INFINITY, f32::INFINITY)
        } else {
            let boundary = (cell as f32 + if step > 0 { 0.5 } else { -0.5 }) * spacing;
            ((boundary - coordinate) / delta, spacing / delta.abs())
        }
    };
    let (mut t_max_x, t_delta_x) = axis_values(start.x, cell.x, delta.x, step.x);
    let (mut t_max_y, t_delta_y) = axis_values(start.y, cell.y, delta.y, step.y);

    let mut cells = Vec::new();
    for _ in 0..=config::MAX_DIRECTORY_ENTRIES {
        cells.push((cell.x, cell.y));
        if cell == end_cell {
            break;
        }
        if t_max_x < t_max_y {
            cell.x += step.x;
            t_max_x += t_delta_x;
        } else {
            cell.y += step.y;
            t_max_y += t_delta_y;
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::{grid_cells_along_segment, move_selection, ray_box_intersection};
    use crate::filesystem::{FileNode, Navigator};
    use crate::state::{OrbitCameraResource, SelectionState};
    use bevy::prelude::{Dir3, Ray3d, Vec2, Vec3};
    use std::path::PathBuf;

    #[test]
    fn axis_parallel_ray_hits_without_dividing_by_zero() {
        let ray = Ray3d::new(Vec3::new(0.0, 5.0, 0.0), Dir3::new(Vec3::NEG_Y).unwrap());
        assert_eq!(
            ray_box_intersection(&ray, Vec3::ZERO, Vec3::splat(1.0)),
            Some(4.0)
        );
    }

    #[test]
    fn axis_parallel_ray_outside_box_misses() {
        let ray = Ray3d::new(Vec3::new(2.0, 5.0, 0.0), Dir3::new(Vec3::NEG_Y).unwrap());
        assert_eq!(
            ray_box_intersection(&ray, Vec3::ZERO, Vec3::splat(1.0)),
            None
        );
    }

    #[test]
    fn grid_traversal_visits_each_crossed_cell() {
        let cells = grid_cells_along_segment(Vec2::new(0.0, 0.0), Vec2::new(7.0, 0.0));
        assert_eq!(cells, vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
    }

    #[test]
    fn selection_moves_in_camera_relative_direction() {
        let mut left = FileNode::new("left".into(), PathBuf::from("left"), false, 1, 0);
        left.grid_pos = (0, 0);
        let mut right = FileNode::new("right".into(), PathBuf::from("right"), false, 1, 0);
        right.grid_pos = (1, 0);
        let mut navigator = Navigator::empty(PathBuf::from("/"), false);
        navigator.entries = vec![left, right];
        let mut selection = SelectionState {
            selected: Some(0),
            hovered: None,
        };
        let mut orbit = OrbitCameraResource {
            yaw: std::f32::consts::FRAC_PI_2,
            ..Default::default()
        };

        move_selection(1, 0, &navigator, &mut selection, &mut orbit);
        assert_eq!(selection.selected, Some(1));
    }
}
