use bevy::prelude::*;

mod cli;
mod command;
mod config;
mod filesystem;
mod platform;
mod plugins;
mod state;

use cli::Options;
use config::{WINDOW_HEIGHT, WINDOW_TITLE, WINDOW_WIDTH};
use plugins::RaptorPlugins;
use state::{
    DirectoryLoadState, NavigatorResource, OrbitCameraResource, ScanEffectResource, SelectionState,
    UiSettings,
};

fn main() {
    let options = Options::parse();
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"));
    let initial_path = options
        .initial_path
        .clone()
        .filter(|path| path.is_dir())
        .unwrap_or(home);

    App::new()
        .insert_resource(NavigatorResource::new(initial_path, options.show_hidden))
        .insert_resource(UiSettings {
            show_labels: options.show_labels,
            show_fps: options.show_fps,
        })
        .insert_resource(DirectoryLoadState::default())
        .insert_resource(OrbitCameraResource::default())
        .insert_resource(SelectionState::default())
        .insert_resource(ScanEffectResource::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: WINDOW_TITLE.into(),
                resolution: (WINDOW_WIDTH, WINDOW_HEIGHT).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(RaptorPlugins)
        .run();
}
