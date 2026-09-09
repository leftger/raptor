use crate::state::{
    DirectoryLoadFailed, DirectoryLoadState, DirectoryLoaded, DirectoryRequested,
    NavigatorResource, OrbitCameraResource, ScanEffectResource, SelectionState,
};
use bevy::prelude::*;

pub struct FilesystemPlugin;

impl Plugin for FilesystemPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DirectoryRequested>()
            .add_message::<DirectoryLoaded>()
            .add_message::<DirectoryLoadFailed>()
            .add_systems(Startup, request_initial_directory)
            .add_systems(
                Update,
                (start_loads, poll_loads, apply_loaded, apply_failure).chain(),
            );
    }
}

fn request_initial_directory(
    navigator: Res<NavigatorResource>,
    mut events: MessageWriter<DirectoryRequested>,
) {
    events.write(DirectoryRequested {
        path: navigator.0.current_path.clone(),
    });
}

fn start_loads(
    mut requests: MessageReader<DirectoryRequested>,
    mut state: ResMut<DirectoryLoadState>,
    navigator: Res<NavigatorResource>,
) {
    for request in requests.read() {
        let generation = state.next_generation();
        let show_hidden = navigator.0.show_hidden;
        state.begin_scan(generation, request.path.clone(), show_hidden);
    }
}

fn poll_loads(
    mut state: ResMut<DirectoryLoadState>,
    mut loaded: MessageWriter<DirectoryLoaded>,
    mut failed: MessageWriter<DirectoryLoadFailed>,
) {
    while let Some(result) = state.poll() {
        if result.generation != state.generation {
            continue;
        }

        match result.result {
            Ok(contents) => {
                loaded.write(DirectoryLoaded {
                    path: result.path,
                    contents,
                });
            }
            Err(message) => {
                state.last_error = Some(message.clone());
                failed.write(DirectoryLoadFailed {
                    path: result.path,
                    message,
                });
            }
        }
    }
}

fn apply_loaded(
    mut events: MessageReader<DirectoryLoaded>,
    mut navigator: ResMut<NavigatorResource>,
    mut selection: ResMut<SelectionState>,
    mut scan: ResMut<ScanEffectResource>,
    mut camera: ResMut<OrbitCameraResource>,
) {
    for event in events.read() {
        let navigator = &mut navigator.0;
        navigator.current_path = event.path.clone();
        navigator.entries = event.contents.nodes.clone();
        navigator.grid_width = event.contents.grid_width;

        selection.selected = None;
        selection.hovered = None;
        scan.reset();
        camera.reset_target();
    }
}

fn apply_failure(
    mut events: MessageReader<DirectoryLoadFailed>,
    mut state: ResMut<DirectoryLoadState>,
) {
    for event in events.read() {
        state.last_error = Some(format!("{}: {}", event.path.display(), event.message));
    }
}
