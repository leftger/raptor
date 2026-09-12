use crate::asteroids::AsteroidsPhase;
use crate::config;
use crate::disc::{DiscPhase, SourceGame};
use crate::document::DocumentLoadState;
use crate::filesystem::loader::{breadcrumb_label, get_path_components, path_component_name};
use crate::lightcycle::{LightcycleState, RunEnvironment, SourceSim};
use crate::load::{DirectoryLoadState, DirectoryLoaded, DirectoryRequested};
use crate::plugins::music::MusicState;
use crate::state::{
    FloodState, HistoryState, InteractionMode, MachineState, NavigatorResource, PauseState,
    SchedulerRace, SelectionState, UiNotice, UiSettings,
};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use std::path::PathBuf;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .init_resource::<MachineState>()
            .add_systems(Startup, setup_ui)
            .add_systems(
                Update,
                (
                    spawn_breadcrumbs,
                    update_header_text,
                    update_breadcrumb_styling,
                    update_footer_text,
                    update_status_text,
                    update_selection_info,
                    update_folio_panel,
                    sync_radar,
                    update_radar,
                    sync_pause_menu,
                    update_machine_stats,
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
struct SelectionInfoPanel;

#[derive(Component)]
struct FooterPrimaryText;

#[derive(Component)]
struct FooterSecondaryText;

#[derive(Component)]
struct UiChrome;

#[derive(Component)]
struct FolioPanel;

#[derive(Component)]
struct FolioPanelText;

/// The pause/warp menu overlay, present only while a lightcycle run is paused.
#[derive(Component)]
struct PauseMenuPanel;

/// The fake machine telemetry line in the header.
#[derive(Component)]
struct MachineStatsText;

/// The radar panel, while a stealth run is on and gone when it is not.
#[derive(Component)]
struct RadarPanel;

/// One room cell of the radar, so a shade can be set without rebuilding the grid.
#[derive(Component)]
struct RadarCell {
    cell: (i32, i32),
}

/// Builds the radar for a stealth run and tears it down when the run ends.
fn sync_radar(
    mut commands: Commands,
    state: Res<LightcycleState>,
    panels: Query<Entity, With<RadarPanel>>,
) {
    if state
        .run
        .as_ref()
        .and_then(|run| run.source_stealth())
        .is_none()
    {
        for panel in &panels {
            commands.entity(panel).despawn();
        }
        return;
    }
    if !panels.is_empty() {
        return;
    }

    // The grid is fixed for the run, so it is built once and only recoloured.
    let half_x = config::STEALTH_WIDTH / 2;
    let half_z = config::STEALTH_HEIGHT / 2;
    let cell = config::RADAR_CELL_SIZE;
    commands
        .spawn((
            RadarPanel,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(config::RADAR_MARGIN),
                right: Val::Px(config::RADAR_MARGIN),
                width: Val::Px((half_x * 2 - 1) as f32 * cell),
                height: Val::Px((half_z * 2 - 1) as f32 * cell),
                ..default()
            },
            BackgroundColor(config::RADAR_PANEL_COLOR),
            Pickable::IGNORE,
        ))
        .with_children(|panel| {
            for z in -half_z + 1..half_z {
                for x in -half_x + 1..half_x {
                    // A map, so room +Z runs up the panel: north on the radar is
                    // north in the room, whichever way the camera happens to face.
                    let across = (x + half_x - 1) as f32 * cell;
                    let down = (half_z - 1 - z) as f32 * cell;
                    panel.spawn((
                        RadarCell { cell: (x, z) },
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(across),
                            top: Val::Px(down),
                            width: Val::Px(cell - config::RADAR_CELL_GAP),
                            height: Val::Px(cell - config::RADAR_CELL_GAP),
                            ..default()
                        },
                        BackgroundColor(config::RADAR_FLOOR_COLOR),
                        Pickable::IGNORE,
                    ));
                }
            }
        });
}

/// A fake syscall trace the header ticker scrolls through.
const SYSCALL_TRACE: [&str; 8] = [
    "mov rax, [rdi]",
    "call read",
    "test rax, rax",
    "jz .retry",
    "push rbp",
    "add rsp, 0x18",
    "int 0x80",
    "ret",
];

/// Drives the machine telemetry: a syscall trace cursor plus PC/SP, clock, and
/// temperature readouts that scale with how much of the directory is loaded.
fn update_machine_stats(
    time: Res<Time>,
    mode: Res<InteractionMode>,
    lightcycle: Res<LightcycleState>,
    navigator: Res<NavigatorResource>,
    mut last_step: Local<usize>,
    mut machine: ResMut<MachineState>,
    mut text: Query<&mut Text, With<MachineStatsText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };

    if *mode != InteractionMode::Lightcycle {
        if **text != "CPU IDLE · BUS 0x00" {
            **text = "CPU IDLE · BUS 0x00".to_string();
        }
        return;
    }

    let steps = (time.elapsed_secs() / 0.35) as usize;
    // The ticker advances about three times a second: rebuilding the line and
    // counting the directory every frame would be pure waste.
    if steps == *last_step
        && !navigator.is_changed()
        && !lightcycle.is_changed()
        && !mode.is_changed()
    {
        return;
    }
    *last_step = steps;

    let (dirs, files) = navigator.0.count_by_type();
    let load = files as f32 + dirs as f32 * 0.5;
    let boost = if lightcycle.cache_boost > 0.0 {
        900.0
    } else {
        0.0
    };
    machine.index = steps % SYSCALL_TRACE.len();
    machine.pc = (0x1000 + steps as u32 * 4) & 0xFFFF;
    machine.sp = 0xFFF0_u32.wrapping_sub(steps as u32 * 4) & 0xFFFF;
    machine.clock_mhz = 3200.0 + (load * 40.0).min(2600.0) + boost;
    machine.temperature = 38.0 + (load * 0.35).min(28.0);

    let trace = SYSCALL_TRACE[machine.index];
    let line = format!(
        "{trace} · PC 0x{:04X} · SP 0x{:04X} · {:.2} GHz · {:.0}°C",
        machine.pc,
        machine.sp,
        machine.clock_mhz / 1000.0,
        machine.temperature,
    );
    if **text != line {
        **text = line;
    }
}

/// Builds, refreshes and tears down the pause/warp menu overlay.
fn sync_pause_menu(
    mut commands: Commands,
    mode: Res<InteractionMode>,
    pause: Res<PauseState>,
    panels: Query<Entity, With<PauseMenuPanel>>,
    mut labels: Query<&mut Text, With<PauseMenuPanel>>,
) {
    if *mode != InteractionMode::Lightcycle || !pause.paused {
        for panel in &panels {
            commands.entity(panel).despawn();
        }
        return;
    }

    let mut text = String::from("PAUSED\n\n");
    for (index, game) in SourceGame::ALL.iter().enumerate() {
        let cursor = if index == pause.warp_index {
            "> "
        } else {
            "  "
        };
        text.push_str(&format!("{cursor}{:>2}. {}\n", index + 1, game.label()));
    }
    text.push_str("\nW/S select · ENTER/SPACE warp · ESC/P resume");

    if let Ok(mut label) = labels.single_mut() {
        if **label != text {
            **label = text;
        }
        return;
    }

    commands
        .spawn((
            PauseMenuPanel,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50.0),
                top: percent(50.0),
                width: px(360.0),
                padding: UiRect::all(px(20.0)),
                ..default()
            },
            BackgroundColor(config::UI_PANEL_COLOR),
            Pickable::IGNORE,
        ))
        .with_child((
            Text::new(text),
            TextFont {
                font_size: bevy::text::FontSize::Px(20.0),
                ..default()
            },
            TextColor(config::TEXT_PRIMARY),
        ));
}

/// Shades the radar: cover, the figure, the patrols, and everything a patrol can
/// see.
///
/// The sight test is the sim's own, so the radar cannot disagree with the guards
/// about where is safe to stand — which is the one thing a stealth map must get
/// right.
fn update_radar(state: Res<LightcycleState>, mut cells: Query<(&RadarCell, &mut BackgroundColor)>) {
    let Some(room) = state.run.as_ref().and_then(|run| run.source_stealth()) else {
        return;
    };
    for (cell, mut shade) in &mut cells {
        let at = cell.cell;
        let guard_here = room.guards.iter().any(|guard| guard.cell() == at);
        let seen = room.guards.iter().any(|guard| room.guard_sees(guard, at));
        shade.0 = if at == room.character {
            config::RADAR_PLAYER_COLOR
        } else if guard_here {
            config::RADAR_GUARD_COLOR
        } else if seen {
            config::RADAR_CONE_COLOR
        } else if room.is_solid(at) {
            config::RADAR_SOLID_COLOR
        } else {
            config::RADAR_FLOOR_COLOR
        };
    }
}

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
                    // Machine telemetry sits at the far right of the header.
                    header.spawn((
                        MachineStatsText,
                        Node {
                            margin: UiRect::left(Val::Auto),
                            ..default()
                        },
                        Text::new(""),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(config::LABEL_FONT_SIZE),
                            ..default()
                        },
                        TextColor(config::TEXT_PRIMARY),
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
                        FooterPrimaryText,
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
                        FooterSecondaryText,
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
                    SelectionInfoPanel,
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

            parent
                .spawn((
                    FolioPanel,
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(20.0),
                        top: px(config::HEADER_HEIGHT + 20.0),
                        width: px(420.0),
                        max_height: px(360.0),
                        padding: UiRect::all(px(10.0)),
                        overflow: Overflow::scroll_y(),
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(config::UI_PANEL_COLOR),
                ))
                .with_child((
                    FolioPanelText,
                    Text::new(""),
                    TextFont {
                        font_size: bevy::text::FontSize::Px(config::INFO_FONT_SIZE),
                        ..default()
                    },
                    TextColor(config::TEXT_PRIMARY),
                    Node {
                        max_width: px(400.0),
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

        let components = get_path_components(&event.path);
        let container = *container;
        commands.entity(container).with_children(|bar| {
            for (name, path) in components {
                let current = path == event.path;
                let color = if current {
                    config::TEXT_HIGHLIGHT
                } else {
                    config::TEXT_SECONDARY
                };
                let display = breadcrumb_label(&name);
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
                          mut requests: MessageWriter<DirectoryRequested>| {
                        let path = observer_path.clone();
                        if path != navigator.0.current_path {
                            navigator.0.begin_navigate_to(&path);
                            requests.write(DirectoryRequested { path });
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

        let display = breadcrumb_label(&path_component_name(&button.path));
        if **text != display {
            **text = display;
        }
    }
}

fn update_footer_text(
    mode: Res<InteractionMode>,
    mut primary: Query<&mut Text, (With<FooterPrimaryText>, Without<FooterSecondaryText>)>,
    mut secondary: Query<&mut Text, (With<FooterSecondaryText>, Without<FooterPrimaryText>)>,
) {
    let (primary_text, secondary_text) = if *mode == InteractionMode::Explorer {
        (
            "NAV: h j k l  |  o/ENTER: Open  |  r: Reload  |  f: Reveal  |  .: Hidden  |  u/-: Parent  |  /: Root  |  TAB: Labels",
            "MOUSE: Right-drag rotate | Scroll zoom | Click select | Click again enter | Breadcrumb: jump to folder",
        )
    } else {
        (
            "LIGHTCYCLE  |  A/D: Turn, Pivot, Walk or Slide  |  WASD: walk in stealth  |  R: Restart  |  M: Explorer  |  Folders: enter  |  .md: read  |  source: .rs/.cpp fight, .c rocks, .py snake, .slint platformer, .lua breaker, .sh stealth (Space: throw/fire/jump/serve)  |  Shift: bullet time  |  Q: recall  |  Gate: parent/close",
            "MOUSE: Hold right-drag to look around  |  u/-: Parent directory or close  |  Breadcrumb: jump to folder  |  Approach text for the folio panel",
        )
    };

    if let Ok(mut text) = primary.single_mut()
        && **text != primary_text
    {
        **text = primary_text.to_string();
    }

    if let Ok(mut text) = secondary.single_mut()
        && **text != secondary_text
    {
        **text = secondary_text.to_string();
    }
}

#[allow(clippy::too_many_arguments)]
fn update_status_text(
    mode: Res<InteractionMode>,
    ui_settings: Res<UiSettings>,
    navigator: Res<NavigatorResource>,
    load_state: Res<DirectoryLoadState>,
    ui_notice: Res<UiNotice>,
    lightcycle: Res<LightcycleState>,
    flood: Res<FloodState>,
    race: Res<SchedulerRace>,
    history: Res<HistoryState>,
    document_load: Res<DocumentLoadState>,
    music: Res<MusicState>,
    diagnostics: Res<DiagnosticsStore>,
    mut status_text: Query<&mut Text, (With<StatusLineText>, Without<ChildrenStatsText>)>,
    mut children_stats: Query<&mut Text, (With<ChildrenStatsText>, Without<StatusLineText>)>,
) {
    let Ok(mut status_text) = status_text.single_mut() else {
        return;
    };

    let mut status = if *mode == InteractionMode::Lightcycle {
        match &lightcycle.run {
            Some(run) => {
                let mut status = format!("MODE: LIGHTCYCLE | BUS TRACE: {}", run.sim.trail.len());
                if run.sim.phase == crate::lightcycle::logic::RunPhase::Ready {
                    status = format!("{status} | READY: no empty spawn cell");
                }
                if let Some(crash) = &run.crash_label {
                    status = format!("{status} | CRASHED: {crash}");
                }
                if let Some(entering) = &run.entering_label {
                    status = format!("{status} | ENTERING: {entering}");
                }
                if let RunEnvironment::Document { name, layout, .. } = &run.environment {
                    status = format!("DOCUMENT: {name} | {status}");
                    if layout.truncated {
                        status = format!("{status} | truncated");
                    }
                    if let Some(heading) = layout.current_heading(run.sim.cell) {
                        status = format!("{status} | {heading}");
                    }
                    if layout.lossy_utf8 {
                        status = format!("{status} | lossy utf-8");
                    }
                }
                if let RunEnvironment::Source {
                    name,
                    layout,
                    sim,
                    language,
                    ..
                } = &run.environment
                {
                    match sim {
                        SourceSim::Asteroids(asteroids) => {
                            status = format!(
                                "ASTEROIDS {} | LIVES {} | RING: {name} | {} | {status}",
                                asteroids.score,
                                asteroids.lives,
                                language.name(),
                            );
                            if asteroids.phase == AsteroidsPhase::Flying {
                                status = format!("{status} | rocks: {}", asteroids.rocks.len());
                            } else {
                                status = format!(
                                    "{status} | ASTEROIDS: {} | drive out the gate",
                                    asteroids.phase.label()
                                );
                            }
                        }
                        SourceSim::Snake(snake) => {
                            status = format!(
                                "SNAKE {} | LEFT {} | TAIL {} | RING: {name} | {} | {status}",
                                snake.eaten,
                                snake.remaining(),
                                snake.max_tail,
                                language.name(),
                            );
                            status = format!(
                                "{status} | EXIT: {}",
                                if snake.exit_open { "OPEN" } else { "LOCKED" }
                            );
                        }
                        SourceSim::Platformer(level) => {
                            status = format!(
                                "PLATFORMER {}% | RING: {name} | {} | {status}",
                                (level.progress() * 100.0).round() as u32,
                                language.name(),
                            );
                            status = format!("{status} | {}", level.phase.label());
                        }
                        SourceSim::Breaker(level) => {
                            status = format!(
                                "BREAKER {} bricks | RING: {name} | {} | {status}",
                                level.remaining(),
                                language.name(),
                            );
                            status = format!("{status} | {}", level.phase.label());
                        }
                        SourceSim::Stealth(room) => {
                            status = format!(
                                "STEALTH {} | DETECT {}% | RING: {name} | {} | {status}",
                                room.phase.label(),
                                room.detection_percent(),
                                language.name(),
                            );
                        }
                        SourceSim::Surfer(surfer) => {
                            status = format!(
                                "SURFER {}% | RIVER: {name} | {} | {status}",
                                (surfer.progress() * 100.0).round() as u32,
                                language.name(),
                            );
                            status = format!("{status} | {}", surfer.phase.label());
                        }
                        SourceSim::Galaga(sim) => {
                            status = format!(
                                "GALAGA {} | LIVES {} | RING: {name} | {} | {status}",
                                sim.score,
                                sim.lives,
                                language.name(),
                            );
                            status = format!("{status} | {}", sim.phase.label());
                        }
                        SourceSim::PacMan(sim) => {
                            status = format!(
                                "PAC-MAN {} | LIVES {} | RING: {name} | {} | {status}",
                                sim.score,
                                sim.lives,
                                language.name(),
                            );
                            status = format!("{status} | {}", sim.phase.label());
                        }
                        SourceSim::Columns(sim) => {
                            status = format!(
                                "COLUMNS {} | RING: {name} | {} | {status}",
                                sim.score,
                                language.name(),
                            );
                            status = format!("{status} | {}", sim.phase.label());
                        }
                        SourceSim::Tetris(sim) => {
                            status = format!(
                                "TETRIS {} / {} LINES | RING: {name} | {} | {status}",
                                sim.lines,
                                config::TETRIS_TARGET_LINES,
                                language.name(),
                            );
                            status = format!("{status} | {}", sim.phase.label());
                        }
                        SourceSim::Frogger(sim) => {
                            status = format!(
                                "FROGGER LIVES {} | RING: {name} | {} | {status}",
                                sim.lives,
                                language.name(),
                            );
                            status = format!("{status} | {}", sim.phase.label());
                        }
                        SourceSim::Qbert(sim) => {
                            status = format!(
                                "QBERT LIVES {} | RING: {name} | {} | {status}",
                                sim.lives,
                                language.name(),
                            );
                            status = format!("{status} | {}", sim.phase.label());
                        }
                        SourceSim::Bomberman(sim) => {
                            status = format!(
                                "BOMBERMAN CRATES {} | LIVES {} | RING: {name} | {} | {status}",
                                sim.crates.len(),
                                sim.lives,
                                language.name(),
                            );
                            status = format!("{status} | {}", sim.phase.label());
                        }
                        SourceSim::Plinko(sim) => {
                            status = format!(
                                "PLINKO {} / {} | BALLS {} | RING: {name} | {} | {status}",
                                sim.score,
                                sim.target,
                                sim.balls_left,
                                language.name(),
                            );
                            status = format!("{status} | {}", sim.phase.label());
                        }
                        SourceSim::DiscWars(disc) => {
                            status = format!(
                                "DISC {}-{} | RING: {name} | {} | {status}",
                                disc.player_score,
                                disc.opponent_score,
                                language.compiler(),
                            );
                            if disc.phase != DiscPhase::Fighting {
                                status = format!("{status} | DISC: {}", disc.phase.label());
                            }
                            if let Some(pickup) = disc.last_pickup {
                                status = format!("{status} | pickup: {}", pickup.label());
                            }
                            // LOCK means a throw would currently line up a clear shot.
                            if disc.has_clear_shot(run.sim.cell, &run.arena) {
                                status = format!("{status} | LOCK");
                            }
                        }
                    }
                    if lightcycle.slow_motion {
                        status = format!("{status} | BULLET TIME");
                    }
                    if layout.truncated {
                        status = format!("{status} | truncated");
                    }
                    if layout.lossy_utf8 {
                        status = format!("{status} | lossy utf-8");
                    }
                }
                status
            }
            None => "MODE: LIGHTCYCLE | STARTING...".to_string(),
        }
    } else {
        format!(
            "Labels: {} | Hidden: {}",
            if ui_settings.show_labels { "ON" } else { "OFF" },
            if navigator.0.show_hidden { "ON" } else { "OFF" },
        )
    };

    let music_label = if music.enabled {
        let (bpm, scale, family) = music.theme.as_ref().map_or_else(
            || (0.0, "-", "-"),
            |theme| {
                (
                    theme.bpm(music.profile),
                    theme.scale.name(),
                    theme.family.name(),
                )
            },
        );
        format!(
            "MUSIC: ON {bpm:.0}bpm {} {scale}/{family} | near: {}",
            music.profile.label(),
            music.mixer.active_slots()
        )
    } else {
        "MUSIC: OFF".to_string()
    };
    status = format!("{status} | {music_label}");

    if *mode == InteractionMode::Explorer && navigator.0.entries_truncated {
        status = format!(
            "SHOWING FIRST {} ENTRIES | {status}",
            crate::config::MAX_DIRECTORY_ENTRIES
        );
    }

    if *mode == InteractionMode::Lightcycle {
        if lightcycle.quarantined {
            status = format!("{status} | QUARANTINE");
        }
        if flood.active && flood.timer > flood.delay {
            status = format!("{status} | MEM OVERFLOW");
        }
        if race.sim.is_some() {
            status = format!("{status} | SCHEDULER RACE");
        }
        if !race.notice.is_empty() {
            status = format!("{status} | {}", race.notice);
        }
        if !history.notice.is_empty() {
            status = format!("{status} | {}", history.notice);
        } else if history.depth() > 1 {
            let redo = if history.can_fast_forward() {
                " ↻"
            } else {
                ""
            };
            status = format!("{status} | HISTORY {}{redo}", history.depth());
        }
        if lightcycle.cache_boost > 0.0 {
            status = format!("{status} | CACHE HIT");
        }
        // The stall itself is under a third of a second, so the label follows
        // the sweep instead: it is up for as long as the wave is on the arena.
        if lightcycle.gc_pause > 0.0 {
            status = format!("{status} | **GC PAUSE**");
        } else if lightcycle.gc_sweep > 0.0 {
            status = format!("{status} | GC SWEEP");
        }
    }

    if ui_settings.show_fps
        && let Some(fps) = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|fps| fps.smoothed())
    {
        status = format!("FPS: {fps:>3.0} | {status}");
    }

    if load_state.loading || document_load.loading {
        status = format!("LOADING... | {status}");
    }
    if let Some(error) = &load_state.last_error {
        status = format!("ERROR: {error} | {status}");
    }
    if let Some(error) = &document_load.last_error {
        status = format!("DOCUMENT ERROR: {error} | {status}");
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

#[allow(clippy::type_complexity)]
fn update_selection_info(
    mode: Res<InteractionMode>,
    navigator: Res<NavigatorResource>,
    selection: Res<SelectionState>,
    mut panel: Single<&mut Visibility, (With<SelectionInfoPanel>, Without<SelectionInfoText>)>,
    mut query: Query<
        (&mut Text, &mut Visibility),
        (With<SelectionInfoText>, Without<SelectionInfoPanel>),
    >,
) {
    let Ok((mut text, mut visibility)) = query.single_mut() else {
        return;
    };

    if *mode == InteractionMode::Lightcycle {
        **panel = Visibility::Hidden;
        *visibility = Visibility::Hidden;
        return;
    }
    **panel = Visibility::Visible;

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

fn update_folio_panel(
    mode: Res<InteractionMode>,
    lightcycle: Res<LightcycleState>,
    mut panel: Query<&mut Node, With<FolioPanel>>,
    mut text: Query<&mut Text, With<FolioPanelText>>,
) {
    let Ok(mut panel) = panel.single_mut() else {
        return;
    };
    let Ok(mut text) = text.single_mut() else {
        return;
    };

    let content = (*mode == InteractionMode::Lightcycle)
        .then(|| lightcycle.run.as_ref())
        .flatten()
        .and_then(|run| match &run.environment {
            RunEnvironment::Document {
                layout,
                focused_block,
                name,
                path,
            } => focused_block.and_then(|index| {
                layout.blocks.get(index).map(|block| {
                    let kind = match block.kind {
                        crate::document::DocBlockKind::Heading(level) => {
                            format!("HEADING {level}")
                        }
                        crate::document::DocBlockKind::Paragraph => "PARAGRAPH".to_string(),
                    };
                    format!("{name}\n{}\n{kind}\n\n{}", path.display(), block.text)
                })
            }),
            RunEnvironment::Directory { .. } => None,
            RunEnvironment::Source {
                name,
                path,
                layout,
                focused_block,
                ..
            } => focused_block.and_then(|index| {
                layout
                    .blocks
                    .get(index)
                    .map(|block| format!("{name}\n{}\nSIGNATURE\n\n{}", path.display(), block.text))
            }),
        });

    if let Some(content) = content {
        panel.display = Display::Flex;
        if **text != content {
            **text = content;
        }
    } else {
        panel.display = Display::None;
        if !text.is_empty() {
            **text = String::new();
        }
    }
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
