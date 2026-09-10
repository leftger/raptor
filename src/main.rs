use bevy::prelude::*;

mod cli;
mod command;
mod config;
mod document;
mod filesystem;
mod lightcycle;
mod load;
mod platform;
mod plugins;
mod state;

use cli::Options;
use config::{WINDOW_HEIGHT, WINDOW_TITLE, WINDOW_WIDTH};
use document::DocumentLoadState;
use load::DirectoryLoadState;
use plugins::RaptorPlugins;
use state::{
    NavigatorResource, OrbitCameraResource, ScanEffectResource, SelectionState, UiNotice,
    UiSettings,
};

fn main() {
    let options = Options::parse();
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"));
    let initial_path = match options.initial_path {
        Some(path) if path.is_dir() => path.canonicalize().unwrap_or(path),
        Some(path) => {
            eprintln!(
                "raptor: '{}' is not a readable directory; opening '{}' instead",
                path.display(),
                home.display()
            );
            home
        }
        None => home,
    };

    App::new()
        .insert_resource(NavigatorResource::new(initial_path, options.show_hidden))
        .insert_resource(UiSettings {
            show_labels: options.show_labels,
            show_fps: options.show_fps,
        })
        .insert_resource(UiNotice::default())
        .insert_resource(DirectoryLoadState::default())
        .insert_resource(DocumentLoadState::default())
        .insert_resource(OrbitCameraResource::default())
        .insert_resource(SelectionState::default())
        .insert_resource(ScanEffectResource::default())
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: WINDOW_TITLE.into(),
                        resolution: (WINDOW_WIDTH, WINDOW_HEIGHT).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: platform::asset_root().to_string_lossy().into_owned(),
                    ..default()
                }),
        )
        .add_plugins(RaptorPlugins)
        .run();
}
