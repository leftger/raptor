use crate::config;
use crate::lightcycle::LightcycleState;
use crate::state::{InteractionMode, LabelsRoot, NavigatorResource, SelectionState, UiSettings};
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::window::PrimaryWindow;
use std::collections::BinaryHeap;

/// Fixed number of UI label entities. Each frame they are re-assigned to the closest
/// visible blocks, so a 30k-entry directory can still label the blocks around the
/// current view without spawning tens of thousands of UI nodes.
#[derive(Component)]
struct ProjectedLabel;

pub struct LabelsPlugin;

impl Plugin for LabelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_label_root)
            .add_systems(Update, (sync_label_visibility, update_labels));
    }
}

fn sync_label_visibility(mut labels_root: Single<&mut Visibility, With<LabelsRoot>>) {
    // Assigning unconditionally would mark the node changed every frame.
    if **labels_root != Visibility::Visible {
        **labels_root = Visibility::Visible;
    }
}

fn setup_label_root(mut commands: Commands) {
    commands
        .spawn((
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
        ))
        .with_children(|parent| {
            for _ in 0..config::LABEL_BUDGET {
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

#[derive(Clone, Copy)]
struct LabelCandidate {
    index: usize,
    screen: Vec2,
    distance_sq: f32,
    priority: u8,
}

/// A pooled entry: raw distance bits first, so the heap's maximum is the
/// farthest candidate and the pool keeps the nearest ones. Distances are
/// non-negative, and `f32::to_bits` preserves that ordering.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PooledCandidate {
    distance_bits: u32,
    index: usize,
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn update_labels(
    mode: Res<InteractionMode>,
    window: Single<Ref<Window>, With<PrimaryWindow>>,
    camera: Single<(&Camera, Ref<GlobalTransform>), With<Camera3d>>,
    navigator: Res<NavigatorResource>,
    ui_settings: Res<UiSettings>,
    selection: Res<SelectionState>,
    lightcycle: Res<LightcycleState>,
    mut pool: Local<BinaryHeap<PooledCandidate>>,
    mut flagged: Local<Vec<usize>>,
    mut candidates: Local<Vec<LabelCandidate>>,
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
    let (camera, camera_transform) = &*camera;
    if !window.is_changed()
        && !camera_transform.is_changed()
        && !navigator.is_changed()
        && !ui_settings.is_changed()
        && !selection.is_changed()
        && !mode.is_changed()
    {
        return;
    }

    for (_, _, _, _, _, mut visibility) in &mut labels {
        *visibility = Visibility::Hidden;
    }

    if !ui_settings.show_labels {
        return;
    }
    if lightcycle
        .run
        .as_ref()
        .is_some_and(crate::lightcycle::ActiveRun::is_document)
    {
        return;
    }

    let window_width = window.width();
    let window_height = window.height();
    let camera_pos = camera_transform.translation();
    let forward = camera_transform.forward();
    let lightcycle_mode = *mode == InteractionMode::Lightcycle;

    // Pass one: a cheap cull and a bounded pool, so only the closest blocks ever
    // reach the projection. Projecting every entry in a large directory *and*
    // sorting the result was the heaviest thing the frame did; the pool keeps
    // the nearest few thousand, which is far more than the label budget, so the
    // labels chosen are the ones a full pass would have chosen.
    //
    // The selected and hovered blocks are tracked separately: they earn a label
    // however far away they are.
    pool.clear();
    flagged.clear();
    for (index, entry) in navigator.0.entries.iter().enumerate() {
        let height = entry.calculate_height();
        let (grid_x, grid_z) = if lightcycle_mode {
            (
                entry.grid_pos.0 * config::LIGHTCYCLE_TOWER_STRIDE,
                entry.grid_pos.1 * config::LIGHTCYCLE_TOWER_STRIDE,
            )
        } else {
            entry.grid_pos
        };
        let offset = config::block_top_position(grid_x, grid_z, height) - camera_pos;
        // Anything behind the camera cannot be on screen.
        if forward.dot(offset) <= 0.0 {
            continue;
        }

        if selection.selected == Some(index) || selection.hovered == Some(index) {
            flagged.push(index);
            continue;
        }

        let candidate = PooledCandidate {
            distance_bits: offset.length_squared().to_bits(),
            index,
        };
        if pool.len() < config::LABEL_CANDIDATE_POOL {
            pool.push(candidate);
        } else if let Some(farthest) = pool.peek()
            && candidate < *farthest
        {
            pool.pop();
            pool.push(candidate);
        }
    }

    // Pass two: project only the pooled blocks, keeping the ones on screen.
    candidates.clear();
    for index in pool
        .iter()
        .map(|pooled| pooled.index)
        .chain(flagged.iter().copied())
    {
        let Some(entry) = navigator.0.entries.get(index) else {
            continue;
        };
        let height = entry.calculate_height();
        let (grid_x, grid_z) = if lightcycle_mode {
            (
                entry.grid_pos.0 * config::LIGHTCYCLE_TOWER_STRIDE,
                entry.grid_pos.1 * config::LIGHTCYCLE_TOWER_STRIDE,
            )
        } else {
            entry.grid_pos
        };
        let world_pos = config::block_top_position(grid_x, grid_z, height);

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

        let priority =
            u8::from(!(selection.selected == Some(index) || selection.hovered == Some(index)));
        candidates.push(LabelCandidate {
            index,
            screen,
            distance_sq: world_pos.distance_squared(camera_pos),
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

        // Only write what actually changed: every write marks the node dirty and
        // costs UI layout work, so stable labels must stay untouched.
        let color = text_color_for_node(entry, is_selected);
        if text_color.0 != color {
            text_color.0 = color;
        }

        let shade = if is_selected {
            Color::srgba(1.0, 0.0, 1.0, 0.7)
        } else if is_hovered {
            Color::srgba(0.0, 1.0, 0.25, 0.7)
        } else {
            Color::srgba(0.0, 0.0, 0.0, 0.6)
        };
        if background.0 != shade {
            background.0 = shade;
        }

        let font_size = if is_selected || is_hovered {
            config::LABEL_FOCUSED_FONT_SIZE
        } else {
            config::LABEL_FONT_SIZE
        };
        if font.font_size != FontSize::Px(font_size) {
            font.font_size = FontSize::Px(font_size);
        }

        let left = px(candidate.screen.x - 40.0);
        let top = px(candidate.screen.y - 24.0);
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
        if *visibility != Visibility::Visible {
            *visibility = Visibility::Visible;
        }
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
