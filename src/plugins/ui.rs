use crate::config;
use crate::state::{
    DirectoryLoadState, DirectoryLoaded, NavigatorResource, SelectionState, UiNotice, UiSettings,
};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use std::path::PathBuf;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .add_systems(Startup, setup_ui)
            .add_systems(
                Update,
                (
                    spawn_breadcrumbs,
                    update_header_text,
                    update_breadcrumb_styling,
                    update_status_text,
                    update_selection_info,
                ),
            );
    }
}

#[derive(Component)]
struct HeaderPathText;

#[derive(Component)]
struct HeaderStatsText;

#[derive(Component)]
struct BreadcrumbButton {
    path: PathBuf,
}

#[derive(Component)]
struct BreadcrumbContainer;

#[derive(Component)]
struct ChildrenStatsText;

#[derive(Component)]
struct StatusLineText;

#[derive(Component)]
struct SelectionInfoText;

#[derive(Component)]
struct UiChrome;

fn setup_ui(mut commands: Commands) {
    commands
        .spawn((
            UiChrome,
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|parent| {
            // Header
            parent
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        top: px(0.0),
                        left: px(0.0),
                        width: percent(100.0),
                        height: px(config::HEADER_HEIGHT),
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(px(20.0)),
                        ..default()
                    },
                    BackgroundColor(config::UI_PANEL_COLOR),
                ))
                .with_children(|header| {
                    header.spawn((
                        Text::new("LOCATION: "),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(config::HEADER_FONT_SIZE),
                            ..default()
                        },
                        TextColor(config::TEXT_SECONDARY),
                    ));
                    header.spawn((
                        HeaderPathText,
                        Text::new("/"),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(config::HEADER_FONT_SIZE),
                            ..default()
                        },
                        TextColor(config::TEXT_PRIMARY),
                    ));
                    header.spawn((
                        HeaderStatsText,
                        Text::new(""),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(config::HEADER_FONT_SIZE),
                            ..default()
                        },
                        TextColor(config::TEXT_SECONDARY),
                    ));
                });

            // Breadcrumb bar
            parent
                .spawn((
                    BreadcrumbContainer,
                    Node {
                        position_type: PositionType::Absolute,
                        top: px(config::HEADER_HEIGHT),
                        left: px(0.0),
                        width: percent(100.0),
                        height: px(config::BREADCRUMB_HEIGHT),
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(px(8.0)),
                        ..default()
                    },
                    BackgroundColor(config::UI_BREADCRUMB_COLOR),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        ChildrenStatsText,
                        Text::new(""),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(config::LABEL_FONT_SIZE),
                            ..default()
                        },
                        TextColor(config::TEXT_PRIMARY),
                        Node {
                            margin: UiRect::left(px(8.0)),
                            ..default()
                        },
                    ));
                });

            // Footer/status
            parent
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: px(0.0),
                        left: px(0.0),
                        width: percent(100.0),
                        height: px(config::FOOTER_HEIGHT),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::SpaceEvenly,
                        padding: UiRect::axes(px(20.0), px(4.0)),
                        ..default()
                    },
                    BackgroundColor(config::UI_PANEL_COLOR),
                ))
                .with_children(|footer| {
                    footer.spawn((
                        Text::new(
                            "NAV: h j k l  |  o/ENTER: Open  |  r: Reload  |  f: Reveal  |  .: Hidden  |  u/-: Parent  |  /: Root  |  TAB: Labels",
                        ),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(config::LABEL_FONT_SIZE),
                            ..default()
                        },
                        TextColor(config::TEXT_WARNING),
                    ));
                    footer.spawn((
                        Text::new(
                            "MOUSE: Right-drag rotate | Scroll zoom | Click select | Click again enter | Breadcrumb: jump to folder",
                        ),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(config::LABEL_FONT_SIZE),
                            ..default()
                        },
                        TextColor(config::TEXT_SECONDARY),
                    ));
                    footer.spawn((
                        StatusLineText,
                        Text::new(""),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(config::INFO_FONT_SIZE),
                            ..default()
                        },
                        TextColor(config::TEXT_PRIMARY),
                    ));
                });

            // Selection info panel
            parent
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(20.0),
                        bottom: px(config::FOOTER_HEIGHT + 8.0),
                        max_width: px(560.0),
                        padding: UiRect::all(px(6.0)),
                        ..default()
                    },
                    BackgroundColor(config::UI_PANEL_COLOR),
                ))
                .with_child((
                    SelectionInfoText,
                    Text::new(""),
                    TextFont {
                        font_size: bevy::text::FontSize::Px(config::INFO_FONT_SIZE),
                        ..default()
                    },
                    TextColor(config::TEXT_HIGHLIGHT),
                    Node {
                        max_width: px(540.0),
                        ..default()
                    },
                ));
        });
}

fn spawn_breadcrumbs(
    mut commands: Commands,
    mut loaded: MessageReader<DirectoryLoaded>,
    container: Single<Entity, With<BreadcrumbContainer>>,
    old_buttons: Query<Entity, With<BreadcrumbButton>>,
) {
    for event in loaded.read() {
        for entity in &old_buttons {
            commands.entity(entity).despawn();
        }

        let components = crate::filesystem::loader::get_path_components(&event.path);
        let container = *container;
        commands.entity(container).with_children(|bar| {
            for (name, path) in components {
                let current = path == event.path;
                let color = if current {
                    config::TEXT_HIGHLIGHT
                } else {
                    config::TEXT_SECONDARY
                };
                let display = if name == "/" {
                    name.clone()
                } else {
                    format!("{name}/")
                };
                let observer_path = path.clone();
                bar.spawn((
                    BreadcrumbButton { path },
                    Button,
                    Node {
                        padding: UiRect::axes(px(4.0), px(2.0)),
                        margin: UiRect::axes(px(2.0), px(0.0)),
                        ..default()
                    },
                    Text::new(display),
                    TextFont {
                        font_size: bevy::text::FontSize::Px(config::INFO_FONT_SIZE),
                        ..default()
                    },
                    TextColor(color),
                    BackgroundColor(Color::NONE),
                ))
                .observe(
                    move |_click: On<Pointer<Click>>,
                          mut navigator: ResMut<NavigatorResource>,
                          mut requests: MessageWriter<crate::state::DirectoryRequested>| {
                        let path = observer_path.clone();
                        if path != navigator.0.current_path {
                            navigator.0.begin_navigate_to(&path);
                            requests.write(crate::state::DirectoryRequested { path });
                        }
                    },
                );
            }
        });
    }
}

fn update_header_text(
    navigator: Res<NavigatorResource>,
    mut path_text: Query<&mut Text, (With<HeaderPathText>, Without<HeaderStatsText>)>,
    mut stats_text: Query<&mut Text, With<HeaderStatsText>>,
) {
    let nav = &navigator.0;
    let Ok(mut path_text) = path_text.single_mut() else {
        return;
    };
    let truncated = truncate_path_prefix(&nav.current_path.display().to_string(), 60);
    if **path_text != truncated {
        **path_text = truncated;
    }

    let Ok(mut stats_text) = stats_text.single_mut() else {
        return;
    };
    let stats = format!(
        "NODES: {}  |  GRID: {}x{}",
        nav.entries.len(),
        nav.grid_width,
        nav.grid_height()
    );
    if **stats_text != stats {
        **stats_text = stats;
    }
}

fn update_breadcrumb_styling(
    navigator: Res<NavigatorResource>,
    mut buttons: Query<(
        &BreadcrumbButton,
        &mut BackgroundColor,
        &mut TextColor,
        &mut Text,
    )>,
) {
    let current = &navigator.0.current_path;
    for (button, mut background, mut text_color, mut text) in &mut buttons {
        let is_current = button.path == *current;
        let color = if is_current {
            config::TEXT_HIGHLIGHT
        } else {
            config::TEXT_SECONDARY
        };
        if text_color.0 != color {
            text_color.0 = color;
        }
        let bg = if is_current {
            config::UI_PANEL_COLOR
        } else {
            Color::NONE
        };
        if background.0 != bg {
            background.0 = bg;
        }

        // Keep separators readable; full path hierarchy is already in the button text.
        let name = current_path_name(&button.path);
        let display = if name == "/" {
            name
        } else {
            format!("{name}/")
        };
        if **text != display {
            **text = display;
        }
    }
}

fn update_status_text(
    ui_settings: Res<UiSettings>,
    navigator: Res<NavigatorResource>,
    load_state: Res<DirectoryLoadState>,
    ui_notice: Res<UiNotice>,
    diagnostics: Res<DiagnosticsStore>,
    mut status_text: Query<&mut Text, (With<StatusLineText>, Without<ChildrenStatsText>)>,
    mut children_stats: Query<&mut Text, (With<ChildrenStatsText>, Without<StatusLineText>)>,
) {
    let Ok(mut status_text) = status_text.single_mut() else {
        return;
    };

    let mut status = format!(
        "Labels: {} | Hidden: {}",
        if ui_settings.show_labels { "ON" } else { "OFF" },
        if navigator.0.show_hidden { "ON" } else { "OFF" },
    );

    if navigator.0.entries_truncated {
        status = format!(
            "SHOWING FIRST {} ENTRIES | {status}",
            crate::config::MAX_DIRECTORY_ENTRIES
        );
    }

    if ui_settings.show_fps
        && let Some(fps) = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|fps| fps.smoothed())
    {
        status = format!("FPS: {fps:>3.0} | {status}");
    }

    if load_state.loading {
        status = format!("LOADING... | {status}");
    }
    if let Some(error) = &load_state.last_error {
        status = format!("ERROR: {error} | {status}");
    }
    if let Some(message) = &ui_notice.message {
        status = format!("ERROR: {message} | {status}");
    }

    if **status_text != status {
        **status_text = status;
    }

    let Ok(mut children_stats) = children_stats.single_mut() else {
        return;
    };
    let (dirs, files) = navigator.0.count_by_type();
    let stats = format!("{} dirs | {} files", dirs, files);
    if **children_stats != stats {
        **children_stats = stats;
    }
}

fn update_selection_info(
    navigator: Res<NavigatorResource>,
    selection: Res<SelectionState>,
    mut query: Query<(&mut Text, &mut Visibility), With<SelectionInfoText>>,
) {
    let Ok((mut text, mut visibility)) = query.single_mut() else {
        return;
    };

    let Some(index) = selection.selected else {
        *visibility = Visibility::Hidden;
        return;
    };
    let Some(node) = navigator.0.entries.get(index) else {
        *visibility = Visibility::Hidden;
        return;
    };

    let line1 = format!("SELECTED: {} | TYPE: {}", node.name, node.type_display());
    let line2 = if node.is_dir {
        format!(
            "CONTENTS: {} | POS: ({}, {})",
            node.size_display(),
            node.grid_pos.0,
            node.grid_pos.1
        )
    } else {
        format!(
            "SIZE: {} | POS: ({}, {})",
            node.size_display(),
            node.grid_pos.0,
            node.grid_pos.1
        )
    };
    let line3 = format!("PATH: {}", node.path.to_string_lossy());
    let line4 = format!("BLOCK HEIGHT: {:.2}", node.calculate_height());
    let content = format!("{line1}\n{line2}\n{line3}\n{line4}");

    if **text != content {
        **text = content;
    }
    *visibility = Visibility::Visible;
}

fn truncate_path_prefix(path: &str, max_length: usize) -> String {
    if path.len() <= max_length {
        return path.to_string();
    }

    let keep = max_length.saturating_sub(3);
    let mut start = path.len() - keep;
    while start < path.len() && !path.is_char_boundary(start) {
        start += 1;
    }
    format!("...{}", &path[start..])
}

fn current_path_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".to_string())
}

#[cfg(test)]
mod tests {
    use super::truncate_path_prefix;

    #[test]
    fn short_paths_are_unchanged() {
        assert_eq!(truncate_path_prefix("/home/user", 60), "/home/user");
    }

    #[test]
    fn long_paths_keep_the_suffix() {
        let path = "a/very/long/path/that/exceeds/the/display/limit/for/sure/here.txt";
        let truncated = truncate_path_prefix(path, 40);
        assert!(truncated.starts_with("..."));
        assert!(truncated.ends_with("here.txt"));
        assert!(truncated.len() <= 40);
    }

    #[test]
    fn multibyte_paths_do_not_panic_or_split_characters() {
        let path = "/tmp/文档/文件夹/这是一个非常长的中文路径名称用于测试/文件.txt";
        let truncated = truncate_path_prefix(path, 40);
        assert!(truncated.starts_with("..."));
        assert!(truncated.ends_with("文件.txt"));
        assert!(!truncated.contains('\u{FFFD}'));
        assert!(truncated.len() <= 40);
    }
}
