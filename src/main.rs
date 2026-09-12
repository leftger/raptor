use bevy::prelude::*;

mod asteroids;
mod bomberman;
mod breaker;
mod cli;
mod columns;
mod command;
mod config;
mod disc;
mod document;
mod filesystem;
mod frogger;
mod galaga;
mod lightcycle;
mod load;
mod music;
mod pacman;
mod platform;
mod platformer;
mod plinko;
mod plugins;
mod qbert;
mod scheduler;
mod snake;
mod state;
mod stealth;
mod surfer;
mod tetris;

use cli::Options;
use config::{WINDOW_HEIGHT, WINDOW_TITLE, WINDOW_WIDTH};
use disc::SourceLoadState;
use document::DocumentLoadState;
use load::DirectoryLoadState;
use plugins::RaptorPlugins;
use plugins::music::MusicState;
use state::{
    NavigatorResource, OrbitCameraResource, RenderSettings, ScanEffectResource, SelectionState,
    UiNotice, UiSettings,
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
        .insert_resource(SourceLoadState::default())
        .insert_resource(OrbitCameraResource::default())
        .insert_resource(SelectionState::default())
        .insert_resource(ScanEffectResource::default())
        .insert_resource(RenderSettings {
            msaa: options.render.msaa,
            bloom: options.render.bloom,
            scanlines: options.render.scanlines,
            vignette: options.render.vignette,
        })
        .insert_resource(MusicState::with_settings(
            options.music,
            options.music_volume,
        ))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: WINDOW_TITLE.into(),
                        resolution: options
                            .window
                            .unwrap_or((WINDOW_WIDTH, WINDOW_HEIGHT))
                            .into(),
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
        .add_plugins(plugins::bench::BenchmarkPlugin {
            config: plugins::bench::BenchConfig {
                seconds: options.bench_seconds,
                ride: options.bench_ride,
            },
        })
        .run();
}
