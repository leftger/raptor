use crate::config;
use crate::state::{DirectoryLoaded, LabelsRoot, NavigatorResource, SelectionState, UiSettings};
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::window::PrimaryWindow;

/// Fixed number of UI label entities. Each frame they are re-assigned to the closest
/// visible blocks, so a 30k-entry directory can still label the blocks around the
/// current view without spawning tens of thousands of UI nodes.
const LABEL_BUDGET: usize = 2_000;

#[derive(Component)]
struct ProjectedLabel;

pub struct LabelsPlugin;

impl Plugin for LabelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_label_root)
            .add_systems(Update, (spawn_label_pool, update_labels));
    }
}

fn setup_label_root(mut commands: Commands) {
    commands.spawn((
        LabelsRoot,
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: percent(100.0),
            height: percent(100.0),
            ..default()
        },
        ZIndex(-1),
        Pickable::IGNORE,
    ));
}

fn spawn_label_pool(
    mut commands: Commands,
    mut loaded: MessageReader<DirectoryLoaded>,
    root: Single<Entity, With<LabelsRoot>>,
    old_labels: Query<Entity, With<ProjectedLabel>>,
) {
    for _event in loaded.read() {
        for entity in &old_labels {
            commands.entity(entity).despawn();
        }

        commands.entity(*root).with_children(|parent| {
            for _ in 0..LABEL_BUDGET {
                parent.spawn((
                    ProjectedLabel,
                    Text::new(""),
                    TextFont {
                        font_size: FontSize::Px(config::LABEL_FONT_SIZE),
                        ..default()
                    },
                    TextColor(config::TEXT_SECONDARY),
                    TextBackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(-1000.0),
                        top: px(-1000.0),
                        ..default()
                    },
                    Visibility::Hidden,
                ));
            }
        });
    }
}

#[derive(Clone, Copy)]
struct LabelCandidate {
    index: usize,
    screen: Vec2,
    distance_sq: f32,
    priority: u8,
}

#[allow(clippy::type_complexity)]
fn update_labels(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    navigator: Res<NavigatorResource>,
    ui_settings: Res<UiSettings>,
    selection: Res<SelectionState>,
    mut labels: Query<
        (
            &mut Node,
            &mut Text,
            &mut TextColor,
            &mut TextBackgroundColor,
            &mut TextFont,
            &mut Visibility,
        ),
        With<ProjectedLabel>,
    >,
) {
    let (camera, camera_transform) = *camera;

    for (_, _, _, _, _, mut visibility) in &mut labels {
        *visibility = Visibility::Hidden;
    }

    if !ui_settings.show_labels {
        return;
    }

    let window_width = window.width();
    let window_height = window.height();
    let camera_pos = camera_transform.translation();

    let mut candidates: Vec<LabelCandidate> = Vec::new();
    for (index, entry) in navigator.0.entries.iter().enumerate() {
        let height = entry.calculate_height();
        let world_pos = config::block_top_position(entry.grid_pos.0, entry.grid_pos.1, height);

        let Ok(screen) = camera.world_to_viewport(camera_transform, world_pos) else {
            continue;
        };

        if screen.x < 0.0 || screen.y < 0.0 || screen.x > window_width || screen.y > window_height {
            continue;
        }

        // Keep labels from drawing over the header/breadcrumb and footer bars.
        if screen.y < config::HEADER_HEIGHT + config::BREADCRUMB_HEIGHT
            || screen.y > window_height - config::FOOTER_HEIGHT
        {
            continue;
        }

        let dx = world_pos.x - camera_pos.x;
        let dy = world_pos.y - camera_pos.y;
        let dz = world_pos.z - camera_pos.z;
        let priority = if selection.selected == Some(index) || selection.hovered == Some(index) {
            0
        } else {
            1
        };

        candidates.push(LabelCandidate {
            index,
            screen,
            distance_sq: dx * dx + dy * dy + dz * dz,
            priority,
        });
    }

    candidates.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| a.distance_sq.total_cmp(&b.distance_sq))
    });

    let mut candidate_iter = candidates.iter();
    for (mut node, mut text, mut text_color, mut background, mut font, mut visibility) in
        &mut labels
    {
        let Some(candidate) = candidate_iter.next() else {
            *visibility = Visibility::Hidden;
            continue;
        };

        let Some(entry) = navigator.0.entries.get(candidate.index) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        let is_selected = selection.selected == Some(candidate.index);
        let is_hovered = selection.hovered == Some(candidate.index);

        let display = entry.display_name(config::LABEL_MAX_LENGTH);
        if **text != display {
            **text = display;
        }

        text_color.0 = text_color_for_node(entry, is_selected);
        background.0 = if is_selected {
            Color::srgba(1.0, 0.0, 1.0, 0.7)
        } else if is_hovered {
            Color::srgba(0.0, 1.0, 0.25, 0.7)
        } else {
            Color::srgba(0.0, 0.0, 0.0, 0.6)
        };

        let font_size = if is_selected || is_hovered {
            config::LABEL_FOCUSED_FONT_SIZE
        } else {
            config::LABEL_FONT_SIZE
        };
        if font.font_size != FontSize::Px(font_size) {
            font.font_size = FontSize::Px(font_size);
        }

        node.left = px(candidate.screen.x - 40.0);
        node.top = px(candidate.screen.y - 24.0);
        *visibility = Visibility::Visible;
    }
}

fn text_color_for_node(node: &crate::filesystem::FileNode, is_selected: bool) -> Color {
    if is_selected {
        Color::WHITE
    } else if node.is_dir {
        config::TEXT_PRIMARY
    } else {
        config::TEXT_SECONDARY
    }
}
