use crate::config;
use crate::state::{
    BlockLabel, DirectoryLoaded, LabelsRoot, NavigatorResource, SelectionState, UiSettings,
};
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::window::PrimaryWindow;

const MAX_PROJECTED_LABELS: usize = 4_000;

pub struct LabelsPlugin;

impl Plugin for LabelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_label_root)
            .add_systems(Update, (spawn_labels, update_labels));
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

fn spawn_labels(
    mut commands: Commands,
    mut loaded: MessageReader<DirectoryLoaded>,
    root: Single<Entity, With<LabelsRoot>>,
    old_labels: Query<Entity, With<BlockLabel>>,
) {
    for event in loaded.read() {
        for entity in &old_labels {
            commands.entity(entity).despawn();
        }

        // Keep label spawning from turning a huge directory into an unusable UI tree.
        let node_count = event.contents.nodes.len().min(MAX_PROJECTED_LABELS);
        let nodes = &event.contents.nodes[..node_count];

        commands.entity(*root).with_children(|parent| {
            for (index, node) in nodes.iter().enumerate() {
                parent.spawn((
                    BlockLabel { index },
                    Text::new(node.display_name(config::LABEL_MAX_LENGTH)),
                    TextFont {
                        font_size: FontSize::Px(config::LABEL_FONT_SIZE),
                        ..default()
                    },
                    TextColor(text_color_for_node(node, false, false)),
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

fn update_labels(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    navigator: Res<NavigatorResource>,
    ui_settings: Res<UiSettings>,
    selection: Res<SelectionState>,
    mut labels: Query<(
        &BlockLabel,
        &mut Node,
        &mut Text,
        &mut TextColor,
        &mut TextBackgroundColor,
        &mut TextFont,
        &mut Visibility,
    )>,
) {
    let (camera, camera_transform) = *camera;

    for (label, mut node, mut text, mut text_color, mut background, mut font, mut visibility) in
        &mut labels
    {
        let Some(entry) = navigator.0.entries.get(label.index) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        if !ui_settings.show_labels {
            *visibility = Visibility::Hidden;
            continue;
        }

        let is_selected = selection.selected == Some(label.index);
        let is_hovered = selection.hovered == Some(label.index);
        let height = entry.calculate_height();
        let world_pos = config::block_top_position(entry.grid_pos.0, entry.grid_pos.1, height);

        let Ok(screen) = camera.world_to_viewport(camera_transform, world_pos) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        if screen.x < 0.0
            || screen.y < 0.0
            || screen.x > window.width()
            || screen.y > window.height()
        {
            *visibility = Visibility::Hidden;
            continue;
        }

        // Keep labels from drawing over the header/breadcrumb and footer bars.
        if screen.y < config::HEADER_HEIGHT + config::BREADCRUMB_HEIGHT
            || screen.y > window.height() - config::FOOTER_HEIGHT
        {
            *visibility = Visibility::Hidden;
            continue;
        }

        let display = entry.display_name(config::LABEL_MAX_LENGTH);
        if **text != display {
            **text = display;
        }

        let color = text_color_for_node(entry, is_selected, is_hovered);
        text_color.0 = color;

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

        node.left = px(screen.x - 40.0);
        node.top = px(screen.y - 24.0);
        *visibility = Visibility::Visible;
    }
}

fn text_color_for_node(
    node: &crate::filesystem::FileNode,
    is_selected: bool,
    _is_hovered: bool,
) -> Color {
    if is_selected {
        Color::WHITE
    } else if node.is_dir {
        config::TEXT_PRIMARY
    } else {
        config::TEXT_SECONDARY
    }
}
